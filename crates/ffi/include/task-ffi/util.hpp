#pragma once

#include "generated.h"

#include <cstdlib>
#include <functional>
#include <iterator>
#include <optional>
#include <string>
#include <string_view>
#include <type_traits>

namespace rust_util {

// To rust Str

template <typename T>
inline rust::Ref<rust::Str> to_rust_str(T input);

template <>
inline rust::Ref<rust::Str> to_rust_str(const char* input)
{
    return rust::std::ffi::CStr::from_ptr(reinterpret_cast<const int8_t*>(input)).to_str().expect("invalid_utf8"_rs);
}
template <>
inline rust::Ref<rust::Str> to_rust_str(std::string input)
{
    return rust::std::ffi::CStr::from_ptr(reinterpret_cast<const int8_t*>(input.c_str())).to_str().expect("invalid_utf8"_rs);
}

template <>
inline rust::Ref<rust::Str> to_rust_str(const std::string& input)
{
    return rust::std::ffi::CStr::from_ptr(reinterpret_cast<const int8_t*>(input.c_str())).to_str().expect("invalid_utf8"_rs);
}

template <>
inline rust::Ref<rust::Str> to_rust_str(std::string_view input)
{
    return rust::std::ffi::CStr::from_ptr(reinterpret_cast<const int8_t*>(input.data())).to_str().expect("invalid_utf8"_rs);
}

// To rust String

template <typename T>
inline rust::std::string::String to_rust_string(T input)
{
    return to_rust_str(input).to_string();
}

// To std::string

/**
 * @brief Convert argument to [std::string_view]
 */
template <typename T>
inline std::string_view to_string_view(T);

/**
 * @brief Convert argument to [std::string]
 */
template <typename T>
inline std::string to_std_string(T str)
{
    return std::string(to_string_view(str));
}

template <>
inline std::string_view to_string_view(rust::Ref<rust::Str> str)
{
    return std::string_view(reinterpret_cast<const char*>(str.as_ptr()), str.len());
}

template <>
inline std::string_view to_string_view(rust::Ref<rust::std::string::String> str)
{
    return std::string_view(reinterpret_cast<const char*>(str.as_str().as_ptr()), str.len());
}

template <>
inline std::string_view to_string_view(rust::std::string::String str)
{
    return std::string_view(reinterpret_cast<const char*>(str.as_str().as_ptr()), str.len());
}

namespace collection {

template <typename T,
          typename Iter,
          typename std::enable_if<std::is_same<typename std::iterator_traits<Iter>::value_type, T>::value, bool>::type = false>
class Iterator : public rust::std::iter::Iterator<T> {
    Iter m_iter;
    Iter m_end;

   public:
    Iterator(Iter begin, Iter end) : m_iter(begin), m_end(end) {}

    rust::std::option::Option<T> next() override
    {
        if (m_iter >= m_end) {
            return rust::std::option::Option<T>::None();
        }
        T value = *m_iter++;
        // You can construct Rust enum with fields in C++
        return rust::std::option::Option<T>::Some(value);
    }
};

template <typename T,
          typename U,
          typename Iter,
          typename std::enable_if<std::is_same<typename std::iterator_traits<Iter>::value_type, T>::value, bool>::type = false>
class MappingIterator : public rust::std::iter::Iterator<U> {
    Iter m_iter;
    Iter m_end;
    std::function<U(T)> m_map;

   public:
    template <typename Map>
    MappingIterator(Iter begin, Iter end, Map&& map) : m_iter(begin), m_end(end), m_map(std::move(map))
    {}

    rust::std::option::Option<U> next() override
    {
        if (m_iter >= m_end) {
            return rust::std::option::Option<U>::None();
        }
        T value = *m_iter++;
        // You can construct Rust enum with fields in C++
        return rust::std::option::Option<U>::Some(m_map(value));
    }
};

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

// useful types

template <typename T>
using rust_iter_t = decltype(std::declval<T>().iter());

template <typename T>
constexpr bool has_rust_iter = is_detected<rust_iter_t, T>;

template <typename T>
using rust_iter_next_t = decltype(std::declval<T>().next());

template <typename T>
constexpr bool has_rust_iter_next = is_detected<rust_iter_next_t, T>;

template <typename T>
using rust_unwrap_t = decltype(std::declval<T>().unwrap());

template <typename T>
using rust_iter_next_unwrap_t = decltype(std::declval<rust_iter_next_t<T>>().unwrap());

struct end_tag {};
}  // namespace detail

/**
 * @brief A c++ iterator over a type implimenting rust's std::iter::Iter trait.
 * Incrementing the iterator calls next and stores the Option<T> value.
 * Derefrencing the iterator calls unwrap on the stored value and thus can only ever be done *ONCE*
 * per index/state.
 *
 * iterators are equal *ONLY* if they are both ended
 *
 * @tparam Iter the rust std::iter::Iter type
 * @param iter the rust std::iter::Iter
 * @return a c++ iterator
 */
template <typename Iter, typename std::enable_if<detail::has_rust_iter_next<Iter>, int>::type = 0>
struct CxxIterator {
    using OptionT = detail::rust_iter_next_t<Iter>;
    using T = detail::rust_iter_next_unwrap_t<Iter>;

    using value_type = T;
    using difference_type = void;
    using pointer = T;
    using reference = T;
    using iterator_category = std::input_iterator_tag;

   private:
    Iter m_iter;
    std::optional<T> m_current;
    bool m_ended;

   public:
    CxxIterator(Iter&& iter) : m_iter(std::move(iter)), m_current(std::nullopt), m_ended(false) {
        ++(*this);
    }
    CxxIterator(detail::end_tag end) : m_ended(true) {}
    CxxIterator(const CxxIterator<Iter>& other) = delete;

    CxxIterator<Iter>& operator++()
    {
        auto current = m_iter.next();
        m_ended = current.matches_None();
        if (!m_ended) {
            m_current = current.unwrap();
        } else {
            m_current = std::nullopt;
        }
        return *this;
    }

    T operator*() { return *m_current; }

    bool operator==(const CxxIterator<Iter>& other) { return m_ended == other.m_ended; }
    bool operator!=(const CxxIterator<Iter>& other) { return m_ended != other.m_ended; }
};

/**
 * @brief A wrapper to impliment a C++ iterator for a type with a `.iter()` function
 * which returns a rust std::iter::Iter
 *
 * Incrementing the iterator calls next and stores the Option<T> value.
 * Derefrencing the iterator calls unwrap on the stored value and thus can only ever be done *ONCE*
 * per index/state.
 *
 * iterators are equal *ONLY* if they are both ended
 *
 * @tparam Iterable Iterable type
 * @param source the iterable
 * @return a wrapped interable
 */
template <typename Iterable, typename std::enable_if<detail::has_rust_iter<Iterable>, int>::type = 0>
struct CxxIterable {
    using Iter = detail::rust_iter_t<Iterable>;

    using iterator = CxxIterator<Iter>;

   private:
    Iterable& m_source;

   public:
    CxxIterable(Iterable& source) : m_source(source) {}

    iterator begin() { return CxxIterator<Iter>(m_source.iter()); }
    iterator end() { return CxxIterator<Iter>(detail::end_tag()); }
};

}  // namespace collection

}  // namespace rust_util
