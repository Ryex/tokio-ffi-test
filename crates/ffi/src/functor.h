#pragma once

#include <functional>

namespace task {
namespace ffi {

template <class R, class... Args>
class Functor : public std::function<R(Args...)> {
public:
  R call(Args... args) const;
};

} // namespace ffi
} // namespace task
