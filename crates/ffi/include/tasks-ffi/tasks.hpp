#pragma once

#include "generated.h"
#include "types.hpp"

#include <functional>
#include <optional>
#include <string>
#include <vector>

namespace task {

struct TaskOptions {
  std::optional<std::string> name;
  std::vector<std::string> tags;
};

using RustCxxAny = rust::crate::ffi::CxxAny;

using RefTaskError = rust::Ref<rust::crate::tasks::TaskError>;
using RefTaskContext = rust::Ref<rust::crate::tasks::TaskContext>;

using FfiTaskAny = rust::crate::tasks::Task<rust::crate::ffi::CxxAny>;
using FfiTaskVoid = rust::crate::tasks::Task<rust::Unit>;

template <typename T> class Task {
private:
  FfiTaskAny m_task;

public:
  Task(FfiTaskAny&& task);

public:
  template <typename T2>
  Task<T2> then(std::function<T2(T)>, std::function<T2(RefTaskError)>,
                std::optional<TaskOptions> options = std::nullopt);
  template <typename T2>
  Task<T2> then(std::function<T2(T, RefTaskContext)>,
                std::function<T2(RefTaskError, RefTaskContext)>,
                std::optional<TaskOptions> options = std::nullopt);
};

template <> class Task<void> {
private:
  FfiTaskVoid m_task;

public:
  Task(FfiTaskVoid&& task);

public:
  template <typename T2>
  Task<T2> then(std::function<T2()>, std::function<T2(RefTaskError)>,
                std::optional<TaskOptions> options = std::nullopt);
  template <typename T2>
  Task<T2> then(std::function<T2(RefTaskContext)>,
                std::function<T2(RefTaskError, RefTaskContext)>,
                std::optional<TaskOptions> options = std::nullopt);
};


class TaskManager {
private:
  rust::crate::tasks::TaskManager m_manager;
public:
  TaskManager();
  template <typename T> Task<T> newTask(std::function<T()>, std::optional<TaskOptions> options = std::nullopt);
  template <typename T> Task<T> newTask(std::function<T(RefTaskContext)>, std::optional<TaskOptions> options = std::nullopt);

};

} // namespace task
