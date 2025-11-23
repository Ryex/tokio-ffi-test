#pragma once

#include <exception>
#include <functional>
#include <memory>
#include <sstream>
#include <string>
#include <string_view>
#include <type_traits>

namespace task {
namespace ffi {

namespace detail {
// Implimentaiton detection
template <typename...>
using try_to_instantiate = void;

using disregard_this = void;

template <template <typename...> class Expression, typename Attempt, typename... Ts>
struct is_detected_impl : std::false_type {};

template <template <typename...> class Expression, typename... Ts>
struct is_detected_impl<Expression, try_to_instantiate<Expression<Ts...>>, Ts...> : std::true_type {};

template <template <typename...> class Expression, typename... Ts>
constexpr bool is_detected = is_detected_impl<Expression, disregard_this, Ts...>::value;

template <typename T>
using std_to_string_expression = decltype(std::to_string(std::declval<T>()));

template <typename T>
constexpr bool has_std_to_string = is_detected<std_to_string_expression, T>;

template <typename T>
using to_string_expression = decltype(to_string(std::declval<T>()));

template <typename T>
constexpr bool has_to_string = is_detected<to_string_expression, T>;

template <typename T>
using ostringstream_expression = decltype(std::declval<std::ostringstream&>() << std::declval<T>());

template <typename T>
constexpr bool has_ostringstream = is_detected<ostringstream_expression, T>;

// ----------------------
// Debug format utils
// ----------------------

template <typename T, typename std::enable_if<has_std_to_string<T>, int>::type = 0>
std::string debugFormat(T const& t)
{
    return std::to_string(t);
}

template <typename T, typename std::enable_if<!has_std_to_string<T> && has_to_string<T>, int>::type = 0>
std::string debugFormat(T const& t)
{
    return to_string(t);
}

template <typename T, typename std::enable_if<!has_std_to_string<T> && !has_to_string<T> && has_ostringstream<T>, int>::type = 0>
std::string debugFormat(T const& t)
{
    std::ostringstream out;
    out << t;
    return out.str();
}

template <>
std::string debugFormat(std::string const& s);
template <>
std::string debugFormat(std::string_view const& s);
std::string debugFormat(const char* s);

template <typename T>
using debugFormat_expression = decltype(::task::ffi::detail::debugFormat(std::declval<T>()));

template <typename T>
constexpr bool has_debugFormat = is_detected<debugFormat_expression, T>;

}  // namespace detail

/**
 * @class CxxException
 * @brief Holds an [std::exception_ptr] refrencing an exception that was passed over the Rust boundry.
 *
 * use [std::rethrow_exception] inside a try_catch block to properly handle the Exception
 *
 */
struct CxxException {
   private:
    std::string m_msg;
    std::exception_ptr m_err;

   public:
    CxxException(std::exception_ptr eptr) : m_msg("Unknown Error"), m_err(eptr) {}
    CxxException(std::exception_ptr eptr, const char* msg) : m_msg(msg), m_err(eptr) {}
    CxxException(const std::exception& err) : m_msg(err.what()), m_err(std::make_exception_ptr(err)) {}

    std::exception_ptr exception() const { return m_err; }
    std::string_view what() const { return m_msg; }
};

/**
 * @class CxxAny
 * @brief Holds an arbitrary value to pass through the Rust/C++ boundry.
 *
 * use [CxxAny::cast] or [CxxAny::rcast] to retrieve the value.
 *
 */
struct CxxAny {
   public:
    CxxAny(const CxxAny& trait) = default;
    CxxAny(CxxAny&& trait) = default;
    CxxAny& operator=(const CxxAny& trait) = default;
    CxxAny& operator=(CxxAny&& trait) = default;
    virtual ~CxxAny() = default;

    template <typename T>
    explicit CxxAny(T t)
        : container(std::make_shared<Model<T>>(std::move(t))), debug_formater([this]() -> std::string {
            if constexpr (detail::has_debugFormat<T>) {
            return ::task::ffi::detail::debugFormat(this->rcast<T>());
            } else {
                return "Un-formatable C++ Value";
            }
        })
    {}

    template <typename T>
    T cast() const
    {
        auto typed_container = std::static_pointer_cast<const Model<T>>(container);
        return typed_container->m_data;
    }

    /**
     * @brief return a const T& to the inner value
     *
     * @tparam T type to fetch
     * @return const T&
     */
    template <typename T>
    T const& rcast() const&
    {
        auto typed_container = std::static_pointer_cast<const Model<T>>(container);
        return typed_container->m_data;
    }
    template <typename T>
    T&& take() &&
    {
        auto typed_container = std::static_pointer_cast<Model<T>>(container);
        return std::move(typed_container->m_data);
    }

    std::string debug() { return debug_formater(); }

   private:
    struct Concept {
        // All need to be explicitly defined just to make the destructor virtual
        Concept() = default;
        Concept(const Concept& cncept) = default;
        Concept(Concept&& cncept) = default;
        Concept& operator=(const Concept& cncept) = default;
        Concept& operator=(Concept&& cncept) = default;
        virtual ~Concept() = default;
    };

    template <typename T>
    struct Model : public Concept {
        Model(T x) : m_data(std::move(x)) {}
        T m_data;
    };

    std::shared_ptr<Concept> container;
    std::function<std::string()> debug_formater;
};

}  // namespace ffi
}  // namespace task
