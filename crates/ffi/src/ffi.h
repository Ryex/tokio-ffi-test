#pragma once

#include <atomic>
#include <cstdint>
#include <functional>
#include <memory>
#include <mutex>
#include <optional>
#include <string>
#include <vector>

#include "ffi/functor.h"
#include "ffi/sync.h"
#include "ffi/tasks.h"
#include "ffi/types.h"

#include "rust/cxx.h"
#include "rust/cxx_enumext.h"
#include "rust/cxx_enumext_macros.h"

#include "tokio-ffi-test/src/ffi/types.rs.h"
#include "tokio-ffi-test/src/ffi/tasks.rs.h"
#include "tokio-ffi-test/src/ffi/functor.rs.h"
#include "tokio-ffi-test/src/ffi/error.rs.h"

namespace task {


struct TaskOptions {
  std::optional<std::string> name;
  std::vector<std::string> tags;
};

class VoidTask;

template <typename T> class Task {
private:
  rust::Box<ffi::TaskCxxAny> m_task;

public:
  Task(rust::Box<ffi::TaskCxxAny> &&task);

public:
  // no value
  template <typename T2>
  Task<T2> then(std::function<T2()>, std::function<T2(const ffi::FfiError &)>);
  // no value, with options
  template <typename T2>
  Task<T2> then(std::function<T2()>, std::function<T2(const ffi::FfiError &)>,
                TaskOptions);
  // with value
  template <typename T2>
  Task<T2> then(std::function<T2(T)>, std::function<T2(const ffi::FfiError &)>);
  // with value, with options
  template <typename T2>
  Task<T2> then(std::function<T2(T)>, std::function<T2(const ffi::FfiError &)>,
                TaskOptions);
  // with value, with context
  template <typename T2>
  Task<T2>
      then(std::function<T2(T, const ffi::TaskContext &)>,
           std::function<T2(const ffi::FfiError &, const ffi::TaskContext &)>);
  // with value, with context, with options
  template <typename T2>
  Task<T2> then(std::function<T2(T, const ffi::TaskContext &)>,
                std::function<T2(const ffi::FfiError &, const ffi::TaskContext &)>,
                TaskOptions);

  // no value
  VoidTask then(std::function<void()>, std::function<void(const ffi::FfiError &)>);
  // no value, with options
  VoidTask then(std::function<void()>, std::function<void(const ffi::FfiError &)>,
                TaskOptions);
  // no vlaue, with context
  VoidTask
      then(std::function<void(const ffi::TaskContext &)>,
           std::function<void(const ffi::FfiError &, const ffi::TaskContext &)>);
  // no vlaue, with context, with options
  VoidTask
      then(std::function<void(const ffi::TaskContext &)>,
           std::function<void(const ffi::FfiError &, const ffi::TaskContext &)>,
           TaskOptions);

  // with value
  VoidTask then(std::function<void(T)>,
                std::function<void(const ffi::FfiError &)>);
  // with value, with options
  VoidTask then(std::function<void(T)>, std::function<void(const ffi::FfiError &)>,
                TaskOptions);
  // with value, with context
  VoidTask
      then(std::function<void(T, const ffi::TaskContext &)>,
           std::function<void(const ffi::FfiError &, const ffi::TaskContext &)>);
  // with value, with context, with options
  VoidTask
      then(std::function<void(T, const ffi::TaskContext &)>,
           std::function<void(const ffi::FfiError &, const ffi::TaskContext &)>,
           TaskOptions);
};

class VoidTask {
private:
  rust::Box<ffi::TaskVoid> m_task;

public:
  VoidTask(rust::Box<ffi::TaskVoid> &&task);

public:
  // no value
  template <typename T2>
  Task<T2> then(std::function<T2()>, std::function<T2(const ffi::FfiError &)>);
  // no value, with options
  template <typename T2>
  Task<T2> then(std::function<T2()>, std::function<T2(const ffi::FfiError &)>,
                TaskOptions);
  // no vlaue, with context
  template <typename T2>
  Task<T2> then(std::function<T2(const ffi::TaskContext &)>,
                std::function<T2(const ffi::FfiError &)>);
  // no vlaue, with context, with options
  template <typename T2>
  Task<T2> then(std::function<T2(const ffi::TaskProgress &)>,
                std::function<T2(const ffi::FfiError &)>, TaskOptions);

  // no value
  VoidTask then(std::function<void()>, std::function<void(const ffi::FfiError &)>);
  // no value, with options
  VoidTask then(std::function<void()>, std::function<void(const ffi::FfiError &)>,
                TaskOptions);
  // no vlaue, with context
  VoidTask then(std::function<void(const ffi::TaskContext &)>,
                std::function<void(const ffi::FfiError &)>);
  // no vlaue, with context, with options
  VoidTask then(std::function<void(const ffi::TaskProgress &)>,
                std::function<void(const ffi::FfiError &)>, TaskOptions);
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
