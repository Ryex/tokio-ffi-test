
#pragma once

#include "generated.h"
#include "types.hpp"
#include "util.hpp"

#include <cstdlib>
#include <exception>
#include <functional>
#include <optional>
#include <stdexcept>
#include <type_traits>

namespace task {

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

template <typename T>
using Arc = rust::std::sync::Arc<T>;

using RustCxxAny = rust::crate::ffi::CxxAny;

using TaskError = rust::crate::task::TaskError;

using TaskSpawnError = rust::crate::task::TaskSpawnError;

using FfiTaskAny = rust::Box<rust::crate::task::Task<rust::crate::ffi::CxxAny>>;
using FfiTaskVoid = rust::Box<rust::crate::task::Task<rust::Unit>>;

using TaskContext = rust::crate::task::TaskContext;
using RefTaskContext = Ref<TaskContext>;
using TaskProgress = rust::crate::task::TaskProgress;
using RefTaskProgress = Ref<TaskProgress>;

using TaskSpawnOptions = rust::crate::task::TaskSpawnOptions;
using RustTaskOptions = rust::crate::task::TaskOptions;
using TaskMetadata = rust::crate::task::TaskMetadata;
using SubscriptionHandle = rust::crate::task::SubscriptionHandle;
using FfiError = rust::crate::task::FfiError;
using FfiAbortHandle = rust::tokio::task::AbortHandle;
using TokioId = rust::tokio::task::Id;
using TaskId = rust::crate::task::TaskId;
using EventType = rust::crate::task::EventType;

template <typename T, typename U>
using TaskContFn = BoxDyn<Fn<Result<T, TaskError>, Result<U, TaskError>>, Send>;

template <typename T, typename U>
using TaskContCtxFn = BoxDyn<Fn<Result<T, TaskError>, Ref<TaskContext>, Result<U, TaskError>>, Send>;

template <typename T>
using InspectFn = BoxDyn<Fn<Ref<T>, Unit>>;

using EventFilterFn = BoxDyn<Fn<TaskId, Ref<RustTaskOptions>, Bool>, Send>;
using EventCallbackFn = BoxDyn<Fn<Ref<TaskMetadata>, Unit>, Send>;

namespace current {

inline TaskId id()
{
    return rust::crate::task::current::id();
}
inline Option<TaskId> try_id()
{
    return rust::crate::task::current::try_id();
}

inline Ref<TaskContext> context()
{
    return rust::crate::task::current::context().deref();
}

inline Option<Ref<TaskContext>> try_context()
{
    return rust::crate::task::current::try_context().as_deref();
}

}  // namespace current

/**
 * @class TaskOptions
 * @brief Options to swpan a task with (name and tag list)
 *
 */
class TaskOptions {
   private:
    TaskSpawnOptions m_opts;

   public:
    TaskOptions() : m_opts(TaskSpawnOptions::new_()) {}
    template <typename T>
    TaskOptions(T name) : m_opts(TaskSpawnOptions::named(rust_util::to_rust_string(name)))
    {}

    /**
     * @brief Add a name to the task
     *
     * @tparam T A type convertable to a rust string via ['rust_util::to_rust_string']
     * @param name The name to add to the task
     * @return The moved TaskOptions
     */
    template <typename T>
    TaskOptions&& withName(T name) &&
    {
        m_opts.with_name(rust_util::to_rust_string(name));
        return std::move(*this);
    }

    /**
     * @brief Add tags to the task
     *
     * @tparam T A type with a `const_iterator` over a type convertable to a rust string via
     * ['rust_util::to_rust_string']
     * @param tags A collection of tags to add to the task
     * @return The moved TaskOptions
     */
    template <typename T>
    TaskOptions&& withTags(T tags) &&
    {
        using IterT = typename std::iterator_traits<typename T::const_iterator>::value_type;
        m_opts.set_tags(BoxDyn<rust::std::iter::Iterator<String>>::make_box<
                        rust_util::collection::MappingIterator<IterT, String, typename T::const_iterator>>(
            tags.cbegin(), tags.cend(), [](IterT str) { return rust_util::to_rust_string(str); }));
        return std::move(*this);
    }

    /**
     * @brief Take the contained TaskSpawnOptions moving it out of the TaskOptions
     * object is unusable after
     *
     * @return The inner TaskSpawnOptions
     */
    TaskSpawnOptions&& take() && { return std::move(m_opts); }
};

/**
 * @brief The return result of a Task. Wrapps an innner rust Result<T, TaskError>
 *
 * @tparam T Task return type
 */
template <typename T>
class TaskResult {
   private:
    Result<RustCxxAny, TaskError> m_result;

    TaskResult(T&& value)
        : m_result(
              Result<RustCxxAny, TaskError>::Ok(RustCxxAny(rust::ZngurCppOpaqueOwnedObject::build<task::ffi::CxxAny>(std::move(value)))))
    {}
    TaskResult(TaskError&& err) : m_result(Result<RustCxxAny, TaskError>::Err(std::move(err))) {}

   public:
    /**
     * @brief Wrap a Rust Task Result
     *
     * @param result Result<T, TaskError> to wrap
     */
    TaskResult(Result<RustCxxAny, TaskError>&& result) : m_result(std::move(result)) {}
    /**
     * @brief Construct a Ok Task Result
     *
     * @param value Ok value
     * @return TaskResult
     */
    static inline TaskResult<T> Ok(T&& value) { return TaskResult(std::move(value)); }
    /**
     * @brief Construct an Err Task Result
     *
     * @param error Err value
     * @return Task Result
     */
    static inline TaskResult<T> Err(TaskError&& error) { return TaskResult(std::move(error)); }

    /**
     * @brief Returns true if the result is Ok
     *
     * @return result is Ok variant
     */
    inline bool isOk() { return m_result.is_ok(); }

    /**
     * @brief retursn true if the result is Err
     *
     * @returnresult is Err variant
     */
    inline bool isErr() { return m_result.is_err(); }

    /**
     * @brief unwraps the inner result T, will panic rust if Result is an error.
     *
     * @return the Task result value
     */
    inline T unwrap() && { return m_result.expect("TaskResult is not Ok"_rs).cpp().take<T>(); }

    /**
     * @brief Unwraps the inner retult error, will panic rust if Result is a value.
     *
     * @return the Task error value
     */
    inline TaskError unwrapErr() && { return m_result.unwrap_err(); }

    /**
     * @brief calls a function with a refrence to contained value if `Ok`
     *
     * @param func function to call
     * @return the unmodified Result which rust has moved
     */
    inline TaskResult<T> inspect(std::function<void(const T&)> func) &&
    {
        return m_result.inspect(InspectFn<RustCxxAny>::make_box([func = std::move(func)](Ref<RustCxxAny> value) {
            func(value.cpp().rcast<T>());
            return Unit{};
        }));
    }

    /**
     * @brief calls a function with a refrence to the contained value if `Err`
     *
     * @param func function to call
     * @return the unmodified REsult which rust has moved
     */
    inline TaskResult<T> inspectErr(std::function<void(Ref<TaskError>)> func) &&
    {
        return m_result.inspect_err(InspectFn<TaskError>::make_box([func = std::move(func)](Ref<TaskError> error) {
            func(error);
            return Unit{};
        }));
    }

    /**
     * @brief A std::visit analog for TaskResult. calls `okFunc`
     * with the unwrapped `T` return value  of the task if the Result is Ok
     * otherwise calls `errFunc` witht he unwrapped `Err` value
     *
     * @tparam U Rerturn type of the passed functions
     * @param okFunc function to call on Ok value
     * @param errFunc function to call on Err value
     * @return return value of `okFunc` or `errFunc`
     */
    template <typename U>
    inline U visit(std::function<U(T)> okFunc, std::function<U(TaskError)> errFunc) &&
    {
        if (m_result.is_ok()) {
            return okFunc(std::move(m_result.unwrap_unchecked().cpp()).take<T>());
        } else {
            return errFunc(m_result.unwrap_err_unchecked());
        }
    }
};

template <>
class TaskResult<void> {
   private:
    Result<Unit, TaskError> m_result;

    TaskResult() : m_result(Result<Unit, TaskError>::Ok(Unit{})) {}
    TaskResult(TaskError&& error) : m_result(Result<Unit, TaskError>::Err(std::move(error))) {}

   public:
    /**
     * @brief Wrap a Rust Task Result
     *
     * @param result Result<void, TaskError> to wrap
     */
    TaskResult(Result<Unit, TaskError>&& result) : m_result(std::move(result)) {}

    /**
     * @brief Construct a Ok Task Result
     *
     * @return TaskResult
     */
    static inline TaskResult<void> Ok() { return TaskResult(); }

    /**
     * @brief Construct an Err Task Result
     *
     * @param error Err value
     * @return Task Result
     */
    static inline TaskResult<void> Err(TaskError&& error) { return TaskResult(std::move(error)); }

    /**
     * @brief Returns true if the result is Ok
     *
     * @return result is Ok variant
     */
    inline bool isOk() { return m_result.is_ok(); }

    /**
     * @brief retursn true if the result is Err
     *
     * @returnresult is Err variant
     */
    inline bool isErr() { return m_result.is_err(); }

    /**
     * @brief Unwraps the inner retult error, will panic rust if Result is a value.
     *
     * @return the Task error value
     */
    inline TaskError unwrapErr() && { return m_result.unwrap_err(); }

    /**
     * @brief calls a function with a refrence to the contained value if `Err`
     *
     * @param func function to call
     * @return the unmodified REsult which rust has moved
     */
    inline TaskResult<void> inspectErr(std::function<void(Ref<TaskError>)> func) &&
    {
        return m_result.inspect_err(InspectFn<TaskError>::make_box([func = std::move(func)](Ref<TaskError> error) {
            func(error);
            return Unit{};
        }));
    }

    /**
     * @brief A std::visit analog for TaskResult. calls `okFunc`
     * with the unwrapped `T` return value  of the task if the Result is Ok
     * otherwise calls `errFunc` witht he unwrapped `Err` value
     *
     * @tparam U Rerturn type of the passed functions
     * @param okFunc function to call on Ok value
     * @param errFunc function to call on Err value
     * @return return value of `okFunc` or `errFunc`
     */
    template <typename U>
    inline U visit(std::function<U()> okFunc, std::function<U(TaskError)> errFunc) &&
    {
        if (m_result.is_ok()) {
            return okFunc();
        } else {
            return errFunc(m_result.unwrap_err_unchecked());
        }
    }
};

/**
 * @class task_spawn_error
 * @brief An exception representing a TaskSpawnError
 *
 */
struct task_spawn_error : public std::runtime_error {
   public:
    task_spawn_error(TaskSpawnError&& err);

   private:
    task_spawn_error(String&& err);
};

struct task_canceled : public std::runtime_error {
   public:
    task_canceled();
};

/**
 * @class AbortHandle
 * @brief A handle to abort a Tokio task
 *
 */
class AbortHandle {
   public:
    AbortHandle(Option<FfiAbortHandle>&& handle);

   public:
    /**
     * @brief Abort the assoceated tokio task
     */
    void abort();

   private:
    Option<FfiAbortHandle> m_handle;
};

/**
 * @brief A Task running in the TaskManager
 *
 * @tparam T Return type of the Task
 * @param task the backing task from rust
 * @return a Task
 */
template <typename T>
class Task {
   private:
    FfiTaskAny m_task;

   public:
    Task(FfiTaskAny&& task);

   public:
    /**
     * @brief Create a new Task awaiting this Task
     *
     * @tparam T2 return type of the new Task
     * @param func function to call with this Tasks's result
     * @param options spawn options for the new Task
     * @return a new Task
     */
    template <typename T2>
    Task<T2> then(std::function<T2(TaskResult<T>)> func, std::optional<TaskOptions> options = std::nullopt);
    /**
     * @brief Create a new Task awaiting this Task with a progress context
     *
     * @tparam T2 return type of the new Task
     * @param func function to call with this Tasks's result and a context for the new task
     * @param options spawn options for the new Task
     * @return a new Task
     */
    template <typename T2>
    Task<T2> then(std::function<T2(TaskResult<T>, RefTaskContext)> func, std::optional<TaskOptions> options = std::nullopt);

    Task<void> then(std::function<void(TaskResult<T>)> func, std::optional<TaskOptions> options = std::nullopt);
    Task<void> then(std::function<void(TaskResult<T>, RefTaskContext)> func, std::optional<TaskOptions> options = std::nullopt);

    AbortHandle on_progress(std::function<void(Ref<TaskProgress>)>) const;

   public:
    TaskId id() const;
    bool isFinished() const;
    bool isCanceled() const;
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
    Task<T2> then(std::function<T2(TaskResult<void>)> func, std::optional<TaskOptions> options = std::nullopt);
    template <typename T2>
    Task<T2> then(std::function<T2(TaskResult<void>, RefTaskContext)> func, std::optional<TaskOptions> options = std::nullopt);

    Task<void> then(std::function<void(TaskResult<void>)> func, std::optional<TaskOptions> options = std::nullopt);
    Task<void> then(std::function<void(TaskResult<void>, RefTaskContext)> func, std::optional<TaskOptions> options = std::nullopt);

    AbortHandle on_progress(std::function<void(Ref<TaskProgress>)>) const;

   public:
    TaskId id() const;
    bool is_finished() const;
    bool is_canceled() const;
    void cancel();
};

class TaskManager {
   private:
    rust::crate::task::TaskManager m_manager;

   public:
    TaskManager();

   public:
    template <typename T>
    Task<T> newTask(std::function<T()>, std::optional<TaskOptions> options = std::nullopt);
    template <typename T>
    Task<T> newTask(std::function<T(RefTaskContext)>, std::optional<TaskOptions> options = std::nullopt);

    SubscriptionHandle subscribe(EventType event, std::function<void(Ref<TaskMetadata>)> callback);
    SubscriptionHandle subscribe(EventType event,
                                 std::function<void(Ref<TaskMetadata>)> callback,
                                 std::function<bool(TaskId, Ref<RustTaskOptions>)> filter);
    void unsubscribe(SubscriptionHandle& handle);
};

inline Option<TaskSpawnOptions> transform_options_ffi(std::optional<task::TaskOptions>& options)
{
    if (options.has_value()) {
        return Option<TaskSpawnOptions>::Some(std::move(*options).take());
    } else {
        return Option<TaskSpawnOptions>::None();
    }
}

/**
 * @brief Wrap a function call with `try {...} catch {...}`
 * and convert the return value into a type which can cross the c++/rust boundry
 *
 * @param func The function to call
 * @param args the arguments to cal the function with in order
 * @return a rust Result which can pass the FFI boundry
 */
template <typename Try, typename... Args>
inline Result<RustCxxAny, TaskError> trycatch_any(Try&& func, Args&&... args)
{
    try {
        return Result<RustCxxAny, TaskError>::Ok(
            RustCxxAny(rust::ZngurCppOpaqueOwnedObject::build<task::ffi::CxxAny>(func(std::forward<Args>(args)...))));
    } catch (const task_canceled&) {
        return Result<RustCxxAny, TaskError>::Err(TaskError::TaskCanceled());
    } catch (const ::std::exception& e) {
        auto what = e.what();
        try {
            throw;  // rethrow to ensure we capture full derived type
        } catch (...) {
            return Result<RustCxxAny, TaskError>::Err(TaskError::Error(
                FfiError::Exception(rust::ZngurCppOpaqueOwnedObject::build<task::ffi::CxxException>(std::current_exception(), what))));
        }
    } catch (...) {
        return Result<RustCxxAny, TaskError>::Err(TaskError::Error(
            FfiError::Exception(rust::ZngurCppOpaqueOwnedObject::build<task::ffi::CxxException>(std::current_exception()))));
    }
}

/**
 * @brief Wrap a function call with `try {...} catch {...}`
 * and convert the return value into a type which can cross the c++/rust boundry
 * specialized for void return types
 *
 * @param func The function to call
 * @param args the arguments to cal the function with in order
 * @return a rust Result which can pass the FFI boundry
 */
template <typename Try, typename... Args>
inline Result<Unit, TaskError> trycatch_unit(Try&& func, Args&&... args)
{
    try {
        func(std::forward<Args>(args)...);
        return Result<Unit, TaskError>::Ok(Unit());
    } catch (const task_canceled&) {
        return Result<Unit, TaskError>::Err(TaskError::TaskCanceled());
    } catch (const ::std::exception& e) {
        auto what = e.what();
        try {
            throw;  // rethrow to ensure we capture full derived type
        } catch (...) {
            return Result<Unit, TaskError>::Err(TaskError::Error(
                FfiError::Exception(rust::ZngurCppOpaqueOwnedObject::build<task::ffi::CxxException>(std::current_exception(), what))));
        }
    } catch (...) {
        return Result<Unit, TaskError>::Err(TaskError::Error(
            FfiError::Exception(rust::ZngurCppOpaqueOwnedObject::build<task::ffi::CxxException>(std::current_exception()))));
    }
}

// -----------------------
// Task Creation
// -----------------------

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

// void specialized

template <>
Task<void> TaskManager::newTask(std::function<void()> f, std::optional<TaskOptions> options);
template <>
Task<void> TaskManager::newTask(std::function<void(Ref<TaskContext>)> f, std::optional<TaskOptions> options);

// -------------------------------
// Task Continuations
// -------------------------------

template <typename T>
template <typename T2>
Task<T2> Task<T>::then(std::function<T2(TaskResult<T>)> func, std::optional<TaskOptions> options)
{
    auto result =
        m_task.as_ref().then(TaskContFn<RustCxxAny, RustCxxAny>::make_box([func = std::move(func)](Result<RustCxxAny, TaskError> result) {
                                 auto task_result = TaskResult<T>(std::move(result));
                                 return trycatch_any(func, std::move(task_result));
                             }),
                             transform_options_ffi(options));

    if (result.is_err()) {
        throw task_spawn_error(result.err().unwrap());
    }
    return result.unwrap();
}
template <typename T>
template <typename T2>
Task<T2> Task<T>::then(std::function<T2(TaskResult<T>, Ref<TaskContext>)> func, std::optional<TaskOptions> options)
{
    auto result = m_task.as_ref().then_with_ctx(TaskContCtxFn<RustCxxAny, RustCxxAny>::make_box(
                                                    [func = std::move(func)](Result<RustCxxAny, TaskError> result, Ref<TaskContext> ctx) {
                                                        auto task_result = TaskResult<T>(std::move(result));
                                                        return trycatch_any(func, std::move(task_result), ctx);
                                                    }),
                                                transform_options_ffi(options));
    if (result.is_err()) {
        throw task_spawn_error(result.err().unwrap());
    }
    return result.unwrap();
}

template <typename T>
Task<void> Task<T>::then(std::function<void(TaskResult<T>)> func, std::optional<TaskOptions> options)
{
    auto result =
        m_task.as_ref().then(TaskContFn<RustCxxAny, Unit>::make_box([func = std::move(func)](Result<RustCxxAny, TaskError> result) {
                                 auto task_result = TaskResult<T>(std::move(result));
                                 return trycatch_unit(func, std::move(task_result));
                             }),
                             transform_options_ffi(options));
    if (result.is_err()) {
        throw task_spawn_error(result.err().unwrap());
    }
    return result.unwrap();
}
template <typename T>
Task<void> Task<T>::then(std::function<void(TaskResult<T>, Ref<TaskContext>)> func, std::optional<TaskOptions> options)
{
    auto result = m_task.as_ref().then_with_ctx(
        TaskContCtxFn<RustCxxAny, Unit>::make_box([func = std::move(func)](Result<RustCxxAny, TaskError> result, Ref<TaskContext> ctx) {
            auto task_result = TaskResult<T>(std::move(result));
            return trycatch_any(func, std::move(task_result), ctx);
        }),
        transform_options_ffi(options));
    if (result.is_err()) {
        throw task_spawn_error(result.err().unwrap());
    }
    return result.unwrap();
}

template <typename T2>
Task<T2> Task<void>::then(std::function<T2(TaskResult<void>)> func, std::optional<TaskOptions> options)
{
    auto result = m_task.as_ref().then(TaskContFn<Unit, RustCxxAny>::make_box([func = std::move(func)](Result<Unit, TaskError> result) {
                                           auto task_result = TaskResult<void>(std::move(result));
                                           return trycatch_any(func, std::move(task_result));
                                       }),
                                       transform_options_ffi(options));
    if (result.is_err()) {
        throw task_spawn_error(result.err().unwrap());
    }
    return result.unwrap();
}

template <typename T2>
Task<T2> Task<void>::then(std::function<T2(TaskResult<void>, Ref<TaskContext>)> func, std::optional<TaskOptions> options)
{
    auto result = m_task.as_ref().then_with_ctx(
        TaskContCtxFn<Unit, RustCxxAny>::make_box([func = std::move(func)](Result<Unit, TaskError> result, Ref<TaskContext> ctx) {
            auto task_result = TaskResult<void>(std::move(result));
            return trycatch_unit(func, std::move(task_result), ctx);
        }),
        transform_options_ffi(options));
    if (result.is_err()) {
        throw task_spawn_error(result.err().unwrap());
    }
    return result.unwrap();
}

template <>
Task<void> Task<void>::then(std::function<void(TaskResult<void>)> func, std::optional<TaskOptions> options);

template <>
Task<void> Task<void>::then(std::function<void(TaskResult<void>, Ref<TaskContext>)> func, std::optional<TaskOptions> options);

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
TaskId Task<T>::id() const
{
    return m_task.as_ref().id();
}

template <typename T>
bool Task<T>::isFinished() const
{
    return m_task.as_ref().is_finished();
}

template <typename T>
bool Task<T>::isCanceled() const
{
    return m_task.as_ref().is_canceled();
}

template <typename T>
void Task<T>::cancel()
{
    m_task.as_ref().cancel();
}

}  // namespace task
