#pragma once

#include "generated.h"

#include <cstdlib>
#include <functional>
#include <iterator>
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

}  // namespace collection

}  // namespace rust_util
