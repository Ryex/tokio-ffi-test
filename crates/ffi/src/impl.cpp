#include <iomanip>

#include "task-ffi/generated.h"
#include "task-ffi/util.hpp"

rust::Ref<rust::Str> rust::Impl<rust::crate::ffi::CxxException>::what(Ref<rust::crate::ffi::CxxException> self)
{
    return rust_util::to_rust_str(self.cpp().what());
}

rust::std::result::Result<rust::Unit, rust::std::fmt::Error> rust::Impl<rust::crate::ffi::CxxAny>::fmt_debug(
    rust::Ref<rust::crate::ffi::CxxAny> self,
    rust::RefMut<rust::std::fmt::Formatter> f)
{
    return f.write_str(rust_util::to_rust_str(self.cpp().debug()));
}

namespace task {
namespace ffi {
namespace detail {

template <>
std::string debugFormat(std::string const& s)
{
    std::ostringstream out;
    out << std::quoted(s);
    return out.str();
}

template <>
std::string debugFormat(std::string_view const& s)
{
    std::ostringstream out;
    out << std::quoted(s);
    return out.str();
}

std::string debugFormat(const char* s)
{
    std::ostringstream out;
    out << std::quoted(s);
    return out.str();
}

}  // namespace detail
}  // namespace ffi
}  // namespace task
