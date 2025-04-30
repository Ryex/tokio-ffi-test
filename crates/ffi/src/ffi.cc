#include "ffi.h"
#include "functor.h"
#include "tokio-ffi-test/src/ffi.rs.h"
#include <cstddef>
#include <memory>

namespace task {

TaskManager::TaskManager() : m_inner(ffi::new_task_manager()) {}

template <typename T>
Task<T>::Task(rust::Box<ffi::TaskCxxAny> &&task) : m_task(std::move(task)) {}
template <typename T>
Task<T>::Task(ffi::TaskCxxAnyWProgress &&task_with_progress)
    : m_task(std::move(task_with_progress.task)),
      m_progress(std::move(task_with_progress.progress)) {}

VoidTask::VoidTask(rust::Box<ffi::TaskVoid> &&task) : m_task(std::move(task)) {}
VoidTask::VoidTask(ffi::TaskVoidWProgress &&task_with_progress)
    : m_task(std::move(task_with_progress.task)),
      m_progress(std::move(task_with_progress.progress)) {}

ffi::TaskOptions transform_options(TaskOptions &&options) {
  return ffi::TaskOptions{
      options.name.has_value()
          ? std::make_unique<std::string>(options.name.value())
          : nullptr,
      std::make_unique<std::vector<std::string>>(std::move(options.tags))};
}

//
// Task Manager
//

template <typename T> Task<T> TaskManager::newTask(std::function<T()> f) {
  return m_inner->new_task_any_noopts([f]() { return ffi::CxxAny(f()); });
}
// no value, with options
template <typename T>
Task<T> TaskManager::newTask(std::function<T()> f, TaskOptions options) {
  return m_inner->new_task_any([f]() { return ffi::CxxAny(f()); },
                               transform_options(std::move(options)));
}

// no value, with context
template <typename T>
Task<T> TaskManager::newTask(std::function<T(const ffi::TaskContext &)> f) {
  return m_inner->new_task_any_ctx(
      [f](const ffi::TaskContext &ctx) { return ffi::CxxAny(f(ctx)); },
      ffi::TaskOptions());
}

// no vlaue, with context, with options
template <typename T>
Task<T> TaskManager::newTask(std::function<T(const ffi::TaskProgress &)> f,
                             TaskOptions options) {
  return m_inner->new_task_any_ctx(
      [f](const ffi::TaskContext &ctx) { return ffi::CxxAny(f(ctx)); },
      transform_options(std::move(options)));
}

//
// Task
//

// no value
template <typename T>
template <typename T2>
Task<T2> Task<T>::then(std::function<T2()> f1,
                       std::function<T2(const ffi::Error &)> f2) {
  return m_task->then_any(
      [f1]() { return ffi::CxxAny(f1()); },
      [f2](const ffi::Error &err) { return ffi::CxxAny(f2(err)); },
      ffi::TaskOptions());
}
// no value, with options

template <typename T>
template <typename T2>
Task<T2> Task<T>::then(std::function<T2()> f1,
                       std::function<T2(const ffi::Error &)> f2,
                       TaskOptions options) {
  return m_task->then_any(
      [f1]() { return ffi::CxxAny(f1()); },
      [f2](const ffi::Error &err) { return ffi::CxxAny(f2(err)); },
      transform_options(std::move(options)));
}

// with value
template <typename T>
template <typename T2>
Task<T2> Task<T>::then(std::function<T2(T)> f1,
                       std::function<T2(const ffi::Error &)> f2) {
  return m_task->then_any_any(
      [f1]() { return ffi::CxxAny(f1()); },
      [f2](const ffi::Error &err) { return ffi::CxxAny(f2(err)); },
      ffi::TaskOptions());
}
// with value, with options
template <typename T>
template <typename T2>
Task<T2> Task<T>::then(std::function<T2(T)> f1,
                       std::function<T2(const ffi::Error &)> f2,
                       TaskOptions options) {
  return m_task->then_any_any(
      [f1](T v) { return ffi::CxxAny(f1(v)); },
      [f2](const ffi::Error &err) { return ffi::CxxAny(f2(err)); },
      transform_options(std::move(options)));
}
// with value, with context
template <typename T>
template <typename T2>
Task<T2> Task<T>::then(
    std::function<T2(T, const ffi::TaskContext &)> f1,
    std::function<T2(const ffi::Error &, const ffi::TaskContext &)> f2) {
  return m_task->then_any_any_ctx(
      [f1](T v, const ffi::TaskContext &ctx) {
        return ffi::CxxAny(f1(v, ctx));
      },
      [f2](const ffi::Error &err, const ffi::TaskContext &ctx) {
        return ffi::CxxAny(f2(err, ctx));
      },
      ffi::TaskOptions());
}
// with value, with context, with options
template <typename T>
template <typename T2>
Task<T2> Task<T>::then(
    std::function<T2(T, const ffi::TaskContext &)> f1,
    std::function<T2(const ffi::Error &, const ffi::TaskContext &)> f2,
    TaskOptions options) {
  return m_task->then_any_any_ctx(
      [f1](T v, const ffi::TaskContext &ctx) {
        return ffi::CxxAny(f1(v, ctx));
      },
      [f2](const ffi::Error &err, const ffi::TaskContext &ctx) {
        return ffi::CxxAny(f2(err, ctx));
      },
      transform_options(std::move(options)));
}

// no value
template <typename T>
VoidTask Task<T>::then(std::function<void()> f1,
                       std::function<void(const ffi::Error &)> f2) {
  return m_task->then_void(std::make_unique<ffi::VoidFunctor>(f1),
                           std::make_unique<ffi::VoidFunctorError>(f2),
                           ffi::TaskOptions());
}
// no value, with options
template <typename T>
VoidTask Task<T>::then(std::function<void()> f1,
                       std::function<void(const ffi::Error &)> f2,
                       TaskOptions options) {
  return m_task->then_void(std::make_unique<ffi::VoidFunctor>(f1),
                           std::make_unique<ffi::VoidFunctorError>(f2),
                           transform_options(std::move(options)));
}
// no vlaue, with context
template <typename T>
VoidTask Task<T>::then(
    std::function<void(const ffi::TaskContext &)> f1,
    std::function<void(const ffi::Error &, const ffi::TaskContext &)> f2) {
  return m_task->then_void_ctx(std::make_unique<ffi::VoidFunctorCtx>(f1),
                               std::make_unique<ffi::VoidFunctorErrorCtx>(f2),
                               ffi::TaskOptions());
}
// no vlaue, with context, with options
template <typename T>
VoidTask Task<T>::then(
    std::function<void(const ffi::TaskContext &)> f1,
    std::function<void(const ffi::Error &, const ffi::TaskContext &)> f2,
    TaskOptions options) {
  return m_task->then_void_ctx(std::make_unique<ffi::VoidFunctorCtx>(f1),
                               std::make_unique<ffi::VoidFunctorErrorCtx>(f2),
                               transform_options(std::move(options)));
}

// with value
template <typename T>
VoidTask Task<T>::then(std::function<void(T)> f1,
                       std::function<void(const ffi::Error &)> f2) {
  return m_task->then_void_any(std::make_unique<ffi::VoidFunctorAny>(f1),
                               std::make_unique<ffi::VoidFunctorError>(f2),
                               ffi::TaskOptions());
}
// with value, with options
template <typename T>
VoidTask Task<T>::then(std::function<void(T)> f1,
                       std::function<void(const ffi::Error &)> f2,
                       TaskOptions options) {
  return m_task->then_void_any(std::make_unique<ffi::VoidFunctorAny>(f1),
                               std::make_unique<ffi::VoidFunctorError>(f2),
                               transform_options(std::move(options)));
}
// with value, with context
template <typename T>
VoidTask Task<T>::then(
    std::function<void(T, const ffi::TaskContext &)> f1,
    std::function<void(const ffi::Error &, const ffi::TaskContext &)> f2) {
  return m_task->then_void_any_ctx(
      std::make_unique<ffi::VoidFunctorAnyCtx>(f1),
      std::make_unique<ffi::VoidFunctorErrorCtx>(f2), ffi::TaskOptions());
}

// with value, with context, with options
template <typename T>
VoidTask Task<T>::then(
    std::function<void(T, const ffi::TaskContext &)> f1,
    std::function<void(const ffi::Error &, const ffi::TaskContext &)> f2,
    TaskOptions options) {
  return m_task->then_void_any_ctx(
      std::make_unique<ffi::VoidFunctorAnyCtx>(f1),
      std::make_unique<ffi::VoidFunctorErrorCtx>(f2),
      transform_options(std::move(options)));
}

//
// VoidTask
//

} // namespace task
