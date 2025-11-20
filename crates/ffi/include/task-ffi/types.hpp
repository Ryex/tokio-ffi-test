#pragma once

#include <exception>
#include <memory>
#include <string>

namespace task {
namespace ffi {

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
    explicit CxxAny(T t) : container(std::make_shared<Model<T>>(std::move(t)))
    {}

    template <typename T>
    T cast() const
    {
        auto typed_container = std::static_pointer_cast<const Model<T>>(container);
        return typed_container->m_data;
    }

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

    std::shared_ptr<const Concept> container;
};

}  // namespace ffi
}  // namespace task
