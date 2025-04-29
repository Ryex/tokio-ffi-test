#pragma once

#include <atomic>
#include <cstdint>
#include <memory>
#include <mutex>
#include <optional>
#include <vector>

#include "functor.h"
#include "sync.h"

namespace task {

namespace ffi {

struct TaskContext;
struct TaskProgress;

struct CxxAny {
public:
  CxxAny(const CxxAny &trait) = default;
  CxxAny(CxxAny &&trait) = default;
  CxxAny &operator=(const CxxAny &trait) = default;
  CxxAny &operator=(CxxAny &&trait) = default;
  virtual ~CxxAny() = default;

  template <typename T>
  explicit CxxAny(T t) : container(std::make_shared<Model<T>>(std::move(t))) {}

  template <typename T> T cast() const {
    auto typed_container = std::static_pointer_cast<const Model<T>>(container);
    return typed_container->m_data;
  }
  template <typename T> T const &rcast() const & {
    auto typed_container = std::static_pointer_cast<const Model<T>>(container);
    return typed_container->m_data;
  }
  template <typename T> T &&take() && {
    auto typed_container = std::static_pointer_cast<const Model<T>>(container);
    return std::move(typed_container->m_data);
  }

private:
  struct Concept {
    // All need to be explicitly defined just to make the destructor virtual
    Concept() = default;
    Concept(const Concept &cncept) = default;
    Concept(Concept &&cncept) = default;
    Concept &operator=(const Concept &cncept) = default;
    Concept &operator=(Concept &&cncept) = default;
    virtual ~Concept() = default;
  };

  template <typename T> struct Model : public Concept {
    Model(T x) : m_data(std::move(x)) {}
    T m_data;
  };

  std::shared_ptr<const Concept> container;
};

using VoidFunctor = Functor<void>;
using TaskFunctor = Functor<void, const TaskContext &>;

} // namespace ffi

template<typename T>
class Task {


private:

};

} // namespace task
