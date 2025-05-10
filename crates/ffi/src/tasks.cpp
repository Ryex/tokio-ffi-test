#include "tasks-ffi/tasks.hpp"
#include "tasks-ffi/generated.h"
#include <optional>

using rust::Bool;
using rust::Send;
using rust::Unit;
template <typename T> using Option = rust::std::option::Option<T>;
template <typename T, typename E>
using Result = rust::std::result::Result<T, E>;

using String = rust::std::string::String;
using Str = rust::Str;

template <typename T> using Ref = rust::Ref<T>;

template <typename T> using Box = rust::Box<T>;
template <typename... T> using Dyn = rust::Dyn<T...>;
template <typename... T> using Fn = rust::Fn<T...>;
template <typename... T> using BoxDyn = Box<Dyn<T...>>;

using RustTaskOptions = rust::crate::tasks::TaskOptions;
using TaskError = rust::crate::tasks::TaskError;
using TaskContext = rust::crate::tasks::TaskContext;

Option<RustTaskOptions>
transform_options(std::optional<task::TaskOptions> &options) {
  Option<RustTaskOptions> rust_opts = Option<RustTaskOptions>::None();
  if (options.has_value()) {
    auto opts = options.value();
    auto ropts = RustTaskOptions::new_();
    if (opts.name.has_value()) {
      ropts.set_name(Option<String>::Some(
          Str::from_char_star(opts.name.value().c_str()).to_owned()));
    }
    auto tags = ropts.tags_mut();
    for (auto &tag : opts.tags) {
      tags.insert(Str::from_char_star(tag.c_str()).to_owned());
    }
  }
  return rust_opts;
}

namespace task {

TaskManager::TaskManager()
    : m_manager(rust::crate::tasks::TaskManager::new_()) {}

template <typename T>
Task<T> TaskManager::newTask(std::function<T()> f,
                             std::optional<TaskOptions> options) {
  return Task<T>(m_manager.new_blocking(
      BoxDyn<Fn<Result<RustCxxAny, TaskError>>, Send>::make_box(
          [f = std::move(f)]() {
            return Result<RustCxxAny, TaskError>::Ok(
                rust::ZngurCppOpaqueOwnedObject::build<task::ffi::CxxAny>(f()));
          }),
      transform_options(options)));
}
template <typename T>
Task<T> TaskManager::newTask(std::function<T(Ref<TaskContext>)> f,
                             std::optional<TaskOptions> options) {
  return Task<T>(m_manager.new_blocking_with_ctx(
      BoxDyn<Fn<Ref<TaskContext>, Result<RustCxxAny, TaskError>>, Send>::make_box(
          [f = std::move(f)](Ref<TaskContext> ctx) {
            return Result<RustCxxAny, TaskError>::Ok(
                rust::ZngurCppOpaqueOwnedObject::build<task::ffi::CxxAny>(f(ctx)));
          }),
      transform_options(options)));
}

template <typename T>
Task<T>::Task(FfiTaskAny &&task) : m_task(std::move(task)) {}

Task<void>::Task(FfiTaskVoid &&task) : m_task(std::move(task)) {}

template <typename T>
template <typename T2>
Task<T2> Task<T>::then(std::function<T2(T)>, std::function<T2(Ref<TaskError>)>,
                       std::optional<TaskOptions> options) {}
template <typename T>
template <typename T2>
Task<T2> Task<T>::then(std::function<T2(T, Ref<TaskContext>)>,
                       std::function<T2(Ref<TaskError>, Ref<TaskContext>)>,
                       std::optional<TaskOptions> options) {}

template <typename T2>
Task<T2> Task<void>::then(std::function<T2()>,
                          std::function<T2(Ref<TaskError>)>,
                          std::optional<TaskOptions> options) {}
template <typename T2>
Task<T2> Task<void>::then(std::function<T2(Ref<TaskContext>)>,
                          std::function<T2(Ref<TaskError>, Ref<TaskContext>)>,
                          std::optional<TaskOptions> options) {}

} // namespace task
