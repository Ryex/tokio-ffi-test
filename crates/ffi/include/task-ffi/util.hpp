#pragma once

#include "zngur_generated.h"

#include <cstdlib>
#include <string>
#include <string_view>

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
inline rust::Ref<rust::Str> to_rust_str(std::string& input)
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
inline rust::std::string::String to_rust_string(T input) {
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

// template <>
// inline std::string_view to_string_view(rust::std::string::String& str)
// {
//     return std::string_view(reinterpret_cast<const char*>(str.as_str().as_ptr()), str.len());
// }

template <>
inline std::string_view to_string_view(rust::std::string::String str)
{
    return std::string_view(reinterpret_cast<const char*>(str.as_str().as_ptr()), str.len());
}

}  // namespace rust_util
