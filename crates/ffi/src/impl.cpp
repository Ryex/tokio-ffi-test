#include "task-ffi/zngur_generated.h"
#include "task-ffi/util.hpp"

rust::Ref<rust::Str> rust::Impl<rust::crate::ffi::CxxException>::what(Ref<rust::crate::ffi::CxxException> self)
{
    return rust_util::to_rust_str(self.cpp().what());
}
