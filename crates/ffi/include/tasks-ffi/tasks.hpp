#pragma once

#include "generated.h"
#include "types.hpp"

#include <cstdlib>
#include <functional>
#include <optional>
#include <stdexcept>
#include <string>
#include <vector>

namespace task {

struct TaskOptions {
    std::optional<std::string> name;
    std::vector<std::string> tags;
};

using rust::Bool;
using rust::Send;
using rust::Unit;
template <typename T>
using Option = rust::std::option::Option<T>;
template <typename T, typename E>
using Result = rust::std::result::Result<T, E>;

using String = rust::std::string::String;
using Str = rust::Str;

template <typename T>
using Ref = rust::Ref<T>;

template <typename T>
using Box = rust::Box<T>;
template <typename... T>
using Dyn = rust::Dyn<T...>;
template <typename... T>
using Fn = rust::Fn<T...>;
template <typename... T>
using BoxDyn = Box<Dyn<T...>>;

using RustCxxAny = rust::crate::ffi::CxxAny;

using TaskError = rust::crate::tasks::TaskError;
using RefTaskError = Ref<TaskError>;

using TaskSpawnError = rust::crate::tasks::TaskSpawnError;

using FfiTaskAny = rust::Box<rust::crate::tasks::Task<rust::crate::ffi::CxxAny>>;
using FfiTaskVoid = rust::Box<rust::crate::tasks::Task<rust::Unit>>;

using TaskContext = rust::crate::tasks::TaskContext;
using RefTaskContext = Ref<TaskContext>;
using TaskProgress = rust::crate::tasks::TaskProgress;
using RefTaskProgress = Ref<TaskProgress>;

using RustTaskOptions = rust::crate::tasks::TaskOptions;
using FfiError = rust::crate::tasks::FfiError;
using ExternalFfiError = rust::crate::tasks::ExternalFfiError;
using FfiAbortHandle = rust::tokio::task::AbortHandle;
using TokioId = rust::tokio::task::Id;

inline rust::Ref<rust::Str> rust_str_from_c_str(const char* input)
{
    return rust::std::ffi::CStr::from_ptr(reinterpret_cast<const int8_t*>(input)).to_str().expect("invalid_utf8"_rs);
}

struct task_spawn_error : public std::runtime_error {
   public:
    task_spawn_error(TaskSpawnError&& err);

   private:
    task_spawn_error(String&& err);
};

class AbortHandle {
   public:
    AbortHandle(Option<FfiAbortHandle>&& handle);

   public:
    void abort();

   private:
    Option<FfiAbortHandle> m_handle;
};

template <typename T>
class Task {
   private:
    FfiTaskAny m_task;

   public:
    Task(FfiTaskAny&& task);

   public:
    template <typename T2>
    Task<T2> then(std::function<T2(T)> func, std::function<T2(RefTaskError)> fail, std::optional<TaskOptions> options = std::nullopt);
    template <typename T2>
    Task<T2> then(std::function<T2(T, RefTaskContext)> func,
                  std::function<T2(RefTaskError, RefTaskContext)> fail,
                  std::optional<TaskOptions> options = std::nullopt);

    Task<void> then(std::function<void(T)> func, std::function<void(RefTaskError)> fail, std::optional<TaskOptions> options = std::nullopt);
    Task<void> then(std::function<void(T, RefTaskContext)> func,
                    std::function<void(RefTaskError, RefTaskContext)> fail,
                    std::optional<TaskOptions> options = std::nullopt);

    AbortHandle on_progress(std::function<void(Ref<TaskProgress>)>) const;

   public:
    TokioId id() const;
    bool is_finished() const;
    bool is_cancelled() const;
    void cancel();

};

template <typename T>
Task<T>::Task(FfiTaskAny&& task) : m_task(std::move(task))
{}

template <>
class Task<void> {
   private:
    FfiTaskVoid m_task;

   public:
    Task(FfiTaskVoid&& task);

   public:
    template <typename T2>
    Task<T2> then(std::function<T2()> func, std::function<T2(RefTaskError)> fail, std::optional<TaskOptions> options = std::nullopt);
    template <typename T2>
    Task<T2> then(std::function<T2(RefTaskContext)> func,
                  std::function<T2(RefTaskError, RefTaskContext)> fail,
                  std::optional<TaskOptions> options = std::nullopt);

    Task<void> then(std::function<void()> func, std::function<void(RefTaskError)> fail, std::optional<TaskOptions> options = std::nullopt);
    Task<void> then(std::function<void(RefTaskContext)> func,
                    std::function<void(RefTaskError, RefTaskContext)> fail,
                    std::optional<TaskOptions> options = std::nullopt);

    AbortHandle on_progress(std::function<void(Ref<TaskProgress>)>) const;

   public:
    TokioId id() const;
    bool is_finished() const;
    bool is_cancelled() const;
    void cancel();
};

class TaskManager {
   private:
    rust::crate::tasks::TaskManager m_manager;

   public:
    TaskManager();

   public:
    template <typename T>
    Task<T> newTask(std::function<T()>, std::optional<TaskOptions> options = std::nullopt);
    template <typename T>
    Task<T> newTask(std::function<T(RefTaskContext)>, std::optional<TaskOptions> options = std::nullopt);
};

inline Option<RustTaskOptions> transform_options_ffi(std::optional<task::TaskOptions>& options)
{
    Option<RustTaskOptions> rust_opts = Option<RustTaskOptions>::None();
    if (options.has_value()) {
        auto opts = options.value();
        auto ropts = RustTaskOptions::new_();
        if (opts.name.has_value()) {
            ropts.set_name(Option<String>::Some(rust_str_from_c_str(opts.name.value().c_str()).to_owned()));
        }
        auto tags = ropts.tags_mut();
        for (auto& tag : opts.tags) {
            tags.insert(rust_str_from_c_str(tag.c_str()).to_owned());
        }
    }
    return rust_opts;
}

template <typename Try, typename... Args>
inline Result<RustCxxAny, TaskError> trycatch_any(Try&& func, Args&&... args)
{
    try {
        return Result<RustCxxAny, TaskError>::Ok(
            RustCxxAny(rust::ZngurCppOpaqueOwnedObject::build<task::ffi::CxxAny>(func(std::forward<Args>(args)...))));
    } catch (const ::std::exception& e) {
        return Result<RustCxxAny, TaskError>::Err(
            TaskError::Error(FfiError::External(ExternalFfiError::new_(2, rust_str_from_c_str(e.what()).to_string()))));
    }
}

template <typename Try, typename... Args>
inline Result<Unit, TaskError> trycatch_unit(Try&& func, Args&&... args)
{
    try {
        func(std::forward<Args>(args)...);
        return Result<Unit, TaskError>::Ok(Unit());
    } catch (const ::std::exception& e) {
        return Result<Unit, TaskError>::Err(
            TaskError::Error(FfiError::External(ExternalFfiError::new_(2, rust_str_from_c_str(e.what()).to_string()))));
    }
}

template <typename T>
Task<T> TaskManager::newTask(std::function<T()> f, std::optional<TaskOptions> options)
{
    return Task<T>(
        m_manager.new_blocking(BoxDyn<Fn<Result<RustCxxAny, TaskError>>, Send>::make_box([f = std::move(f)]() { return trycatch_any(f); }),
                               transform_options_ffi(options)));
}
template <typename T>
Task<T> TaskManager::newTask(std::function<T(Ref<TaskContext>)> f, std::optional<TaskOptions> options)
{
    return Task<T>(m_manager.new_blocking_with_ctx(BoxDyn<Fn<Ref<TaskContext>, Result<RustCxxAny, TaskError>>, Send>::make_box(
                                                       [f = std::move(f)](Ref<TaskContext> ctx) { return trycatch_any(f, ctx); }),
                                                   transform_options_ffi(options)));
}

template <>
Task<void> TaskManager::newTask(std::function<void()> f, std::optional<TaskOptions> options);
template <>
Task<void> TaskManager::newTask(std::function<void(Ref<TaskContext>)> f, std::optional<TaskOptions> options);

template <typename T>
template <typename T2>
Task<T2> Task<T>::then(std::function<T2(T)> func, std::function<T2(Ref<TaskError>)> fail, std::optional<TaskOptions> options)
{
    auto result = m_task.as_ref().then(BoxDyn<Fn<Result<RustCxxAny, TaskError>, Result<RustCxxAny, TaskError>>, Send>::make_box(
                                           [func = std::move(func), fail = std::move(fail)](Result<RustCxxAny, TaskError> result) {
                                               if (result.is_ok()) {
                                                   return trycatch_any(func, result.unwrap().cpp().take<T>());
                                               } else {
                                                   return trycatch_any(fail, result.err().unwrap());
                                               }
                                           }),
                                       transform_options_ffi(options));

    if (result.is_err()) {
        throw task_spawn_error(result.err().unwrap());
    }
    return result.unwrap();
}
template <typename T>
template <typename T2>
Task<T2> Task<T>::then(std::function<T2(T, Ref<TaskContext>)> func,
                       std::function<T2(Ref<TaskError>, Ref<TaskContext>)> fail,
                       std::optional<TaskOptions> options)
{
    auto result = m_task.as_ref().then_with_ctx(
        BoxDyn<Fn<Result<RustCxxAny, TaskError>, Ref<TaskContext>, Result<RustCxxAny, TaskError>>, Send>::make_box(
            [func = std::move(func), fail = std::move(fail)](Result<RustCxxAny, TaskError> result, Ref<TaskContext> ctx) {
                if (result.is_ok()) {
                    return trycatch_any(func, result.unwrap().cpp().take<T>(), ctx);
                } else {
                    return trycatch_any(fail, result.err().unwrap(), ctx);
                }
            }),
        transform_options_ffi(options));
    if (result.is_err()) {
        throw task_spawn_error(result.err().unwrap());
    }
    return result.unwrap();
}

template <typename T>
Task<void> Task<T>::then(std::function<void(T)> func, std::function<void(Ref<TaskError>)> fail, std::optional<TaskOptions> options)
{
    auto result = m_task.as_ref().then(BoxDyn<Fn<Result<RustCxxAny, TaskError>, Result<Unit, TaskError>>, Send>::make_box(
                                           [func = std::move(func), fail = std::move(fail)](Result<RustCxxAny, TaskError> result) {
                                               if (result.is_ok()) {
                                                   task::ffi::CxxAny ret = result.unwrap().cpp();
                                                   return trycatch_unit(func, ret.cast<T>());
                                               } else {
                                                   return trycatch_unit(fail, result.err().unwrap());
                                               }
                                           }),
                                       transform_options_ffi(options));
    if (result.is_err()) {
        throw task_spawn_error(result.err().unwrap());
    }
    return result.unwrap();
}
template <typename T>
Task<void> Task<T>::then(std::function<void(T, Ref<TaskContext>)> func,
                         std::function<void(Ref<TaskError>, Ref<TaskContext>)> fail,
                         std::optional<TaskOptions> options)
{
    auto result = m_task.as_ref().then_with_ctx(
        BoxDyn<Fn<Result<RustCxxAny, TaskError>, Ref<TaskContext>, Result<Unit, TaskError>>, Send>::make_box(
            [func = std::move(func), fail = std::move(fail)](Result<RustCxxAny, TaskError> result, Ref<TaskContext> ctx) {
                if (result.is_ok()) {
                    return trycatch_unit(func, result.unwrap().cpp().take<T>(), ctx);
                } else {
                    return trycatch_unit(fail, result.err().unwrap(), ctx);
                }
            }),
        transform_options_ffi(options));
    if (result.is_err()) {
        throw task_spawn_error(result.err().unwrap());
    }
    return result.unwrap();
}

template <typename T2>
Task<T2> Task<void>::then(std::function<T2()> func, std::function<T2(Ref<TaskError>)> fail, std::optional<TaskOptions> options)
{
    auto result = m_task.as_ref().then(BoxDyn<Fn<Result<Unit, TaskError>, Result<RustCxxAny, TaskError>>, Send>::make_box(
                                           [func = std::move(func), fail = std::move(fail)](Result<Unit, TaskError> result) {
                                               if (result.is_ok()) {
                                                   return trycatch_any(func);
                                               } else {
                                                   return trycatch_any(fail, result.err().unwrap());
                                               }
                                           }),
                                       transform_options_ffi(options));
    if (result.is_err()) {
        throw task_spawn_error(result.err().unwrap());
    }
    return result.unwrap();
}

template <typename T2>
Task<T2> Task<void>::then(std::function<T2(Ref<TaskContext>)> func,
                          std::function<T2(Ref<TaskError>, Ref<TaskContext>)> fail,
                          std::optional<TaskOptions> options)
{
    auto result = m_task.as_ref().then_with_ctx(
        BoxDyn<Fn<Result<Unit, TaskError>, Ref<TaskContext>, Result<RustCxxAny, TaskError>>, Send>::make_box(
            [func = std::move(func), fail = std::move(fail)](Result<Unit, TaskError> result, Ref<TaskContext> ctx) {
                if (result.is_ok()) {
                    return trycatch_any(func, ctx);
                } else {
                    return trycatch_any(fail, result.err().unwrap(), ctx);
                }
            }),
        transform_options_ffi(options));
    if (result.is_err()) {
        throw task_spawn_error(result.err().unwrap());
    }
    return result.unwrap();
}

template <>
Task<void> Task<void>::then(std::function<void()> func, std::function<void(Ref<TaskError>)> fail, std::optional<TaskOptions> options);

template <>
Task<void> Task<void>::then(std::function<void(Ref<TaskContext>)> func,
                            std::function<void(Ref<TaskError>, Ref<TaskContext>)> fail,
                            std::optional<TaskOptions> options);

template <typename T>
AbortHandle Task<T>::on_progress(std::function<void(Ref<TaskProgress>)> func) const
{
    return AbortHandle(m_task.as_ref().on_progress(
        Box<Dyn<Fn<Ref<TaskProgress>, Unit>, Send>>::make_box([func = std::move(func)](Ref<TaskProgress> progress) {
            func(progress);
            return Unit{};
        })));
}

template <typename T>
TokioId Task<T>::id() const
{
    return m_task.as_ref().id();
}

template <typename T>
bool Task<T>::is_finished() const
{
    return m_task.as_ref().is_finished();
}

template <typename T>
bool Task<T>::is_cancelled() const
{
    return m_task.as_ref().is_cancelled();
}

template <typename T>
void Task<T>::cancel()
{
    m_task.as_ref().cancel();
}

}  // namespace task
