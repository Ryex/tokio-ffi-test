#include "functor.h"

namespace task {

namespace ffi {

template <class R, class... Args>
R Functor<R, Args...>::call(Args... args) const {
  return (*this)(args...);
}

}; // namespace ffi
} // namespace task
