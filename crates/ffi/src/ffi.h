#pragma once

#include <atomic>
#include <cstdint>
#include <functional>
#include <memory>
#include <mutex>
#include <optional>
#include <string>
#include <vector>

#include "functor.h"
#include "sync.h"

#include "rust/cxx.h"

namespace task {

namespace ffi {

struct Error;

struct TaskContext;
struct TaskProgress;
struct TaskOptions;

struct TaskCxxAny;
struct TaskVoid;

struct TaskManager;

struct ProgressReceiver;

struct TaskVoidWProgress;
struct TaskCxxAnyWProgress;

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
using VoidFunctorCtx = Functor<void, const TaskContext &>;
using VoidFunctorAny = Functor<void, std::unique_ptr<CxxAny>>;
using VoidFunctorAnyCtx =
    Functor<void, std::unique_ptr<CxxAny>, const TaskContext &>;

using VoidFunctorError = Functor<void, const ffi::Error &>;
using VoidFunctorErrorCtx =
    Functor<void, const ffi::Error &, const TaskContext &>;

using AnyFunctor = Functor<std::unique_ptr<CxxAny>>;
using AnyFunctorCtx = Functor<std::unique_ptr<CxxAny>, const TaskContext &>;
using AnyFunctorAny = Functor<std::unique_ptr<CxxAny>, std::unique_ptr<CxxAny>>;
using AnyFunctorAnyCtx = Functor<std::unique_ptr<CxxAny>,
                                 std::unique_ptr<CxxAny>, const TaskContext &>;

using AnyFunctorError = Functor<std::unique_ptr<CxxAny>, const ffi::Error &>;
using AnyFunctorErrorCtx =
    Functor<std::unique_ptr<CxxAny>, const ffi::Error &, const TaskContext &>;

using VoidFunctorProgress = Functor<void, const TaskProgress &>;

} // namespace ffi

struct TaskOptions {
  std::optional<std::string> name;
  std::vector<std::string> tags;
};

class VoidTask;

template <typename T> class Task {
private:
  rust::Box<ffi::TaskCxxAny> m_task;
  std::optional<rust::Box<ffi::ProgressReceiver>> m_progress;

public:
  Task(rust::Box<ffi::TaskCxxAny> &&task);
  Task(ffi::TaskCxxAnyWProgress &&task_with_progress);

public:
  // no value
  template <typename T2>
  Task<T2> then(std::function<T2()>, std::function<T2(const ffi::Error &)>);
  // no value, with options
  template <typename T2>
  Task<T2> then(std::function<T2()>, std::function<T2(const ffi::Error &)>,
                TaskOptions);
  // with value
  template <typename T2>
  Task<T2> then(std::function<T2(T)>, std::function<T2(const ffi::Error &)>);
  // with value, with options
  template <typename T2>
  Task<T2> then(std::function<T2(T)>, std::function<T2(const ffi::Error &)>,
                TaskOptions);
  // with value, with context
  template <typename T2>
  Task<T2>
      then(std::function<T2(T, const ffi::TaskContext &)>,
           std::function<T2(const ffi::Error &, const ffi::TaskContext &)>);
  // with value, with context, with options
  template <typename T2>
  Task<T2> then(std::function<T2(T, const ffi::TaskContext &)>,
                std::function<T2(const ffi::Error &, const ffi::TaskContext &)>,
                TaskOptions);

  // no value
  VoidTask then(std::function<void()>, std::function<void(const ffi::Error &)>);
  // no value, with options
  VoidTask then(std::function<void()>, std::function<void(const ffi::Error &)>,
                TaskOptions);
  // no vlaue, with context
  VoidTask
      then(std::function<void(const ffi::TaskContext &)>,
           std::function<void(const ffi::Error &, const ffi::TaskContext &)>);
  // no vlaue, with context, with options
  VoidTask
      then(std::function<void(const ffi::TaskContext &)>,
           std::function<void(const ffi::Error &, const ffi::TaskContext &)>,
           TaskOptions);

  // with value
  VoidTask then(std::function<void(T)>,
                std::function<void(const ffi::Error &)>);
  // with value, with options
  VoidTask then(std::function<void(T)>, std::function<void(const ffi::Error &)>,
                TaskOptions);
  // with value, with context
  VoidTask
      then(std::function<void(T, const ffi::TaskContext &)>,
           std::function<void(const ffi::Error &, const ffi::TaskContext &)>);
  // with value, with context, with options
  VoidTask
      then(std::function<void(T, const ffi::TaskContext &)>,
           std::function<void(const ffi::Error &, const ffi::TaskContext &)>,
           TaskOptions);
};

class VoidTask {
private:
  rust::Box<ffi::TaskVoid> m_task;
  std::optional<rust::Box<ffi::ProgressReceiver>> m_progress;

public:
  VoidTask(rust::Box<ffi::TaskVoid> &&task);
  VoidTask(ffi::TaskVoidWProgress &&task_with_progress);

public:
  // no value
  template <typename T2>
  Task<T2> then(std::function<T2()>, std::function<T2(const ffi::Error &)>);
  // no value, with options
  template <typename T2>
  Task<T2> then(std::function<T2()>, std::function<T2(const ffi::Error &)>,
                TaskOptions);
  // no vlaue, with context
  template <typename T2>
  Task<T2> then(std::function<T2(const ffi::TaskContext &)>,
                std::function<T2(const ffi::Error &)>);
  // no vlaue, with context, with options
  template <typename T2>
  Task<T2> then(std::function<T2(const ffi::TaskProgress &)>,
                std::function<T2(const ffi::Error &)>, TaskOptions);

  // no value
  VoidTask then(std::function<void()>, std::function<void(const ffi::Error &)>);
  // no value, with options
  VoidTask then(std::function<void()>, std::function<void(const ffi::Error &)>,
                TaskOptions);
  // no vlaue, with context
  VoidTask then(std::function<void(const ffi::TaskContext &)>,
                std::function<void(const ffi::Error &)>);
  // no vlaue, with context, with options
  VoidTask then(std::function<void(const ffi::TaskProgress &)>,
                std::function<void(const ffi::Error &)>, TaskOptions);
};

class TaskManager {
private:
  rust::Box<ffi::TaskManager> m_inner;

public:
  TaskManager();

public:
  // no value
  template <typename T> Task<T> newTask(std::function<T()>);
  // no value, with options
  template <typename T> Task<T> newTask(std::function<T()>, TaskOptions);
  // no value, with context
  template <typename T>
  Task<T> newTask(std::function<T(const ffi::TaskContext &)>);
  // no vlaue, with context, with options
  template <typename T>
  Task<T> newTask(std::function<T(const ffi::TaskProgress &)>, TaskOptions);
};

} // namespace task
