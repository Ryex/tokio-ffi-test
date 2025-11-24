#include "task-ffi/task.hpp"
#include <algorithm>
#include "task-ffi/generated.h"

namespace task {

task_spawn_error::task_spawn_error(TaskSpawnError&& err) : task_spawn_error(err.to_string()) {}
task_spawn_error::task_spawn_error(String&& err)
    : std::runtime_error(std::string(reinterpret_cast<const char*>(err.as_bytes().as_ptr()), err.as_bytes().len()))
{}

task_canceled::task_canceled() : std::runtime_error(std::string("task canceled")) {}

AbortHandle::AbortHandle(Option<FfiAbortHandle>&& handle) : m_handle(std::move(handle)) {}

void AbortHandle::abort()
{
    if (m_handle.is_some()) {
        m_handle.as_ref().unwrap().abort();
    }
}

// -------------------------------------
// Task Manager creation
// -------------------------------------

TaskManager::TaskManager() : m_manager(rust::crate::task::TaskManager::new_()) {}

SubscriptionHandle TaskManager::subscribe(EventType event, std::function<void(Ref<TaskMetadata>)> callback)
{
    return m_manager.subscribe(std::move(event), Option<EventFilterFn>::None(),
                               EventCallbackFn::make_box([callback = std::move(callback)](Ref<TaskMetadata> meta) {
                                   callback(meta);
                                   return Unit{};
                               }));
}
SubscriptionHandle TaskManager::subscribe(EventType event,
                                          std::function<void(Ref<TaskMetadata>)> callback,
                                          std::function<bool(TaskId, Ref<RustTaskOptions>)> filter)
{
    return m_manager.subscribe(std::move(event),
                               Option<EventFilterFn>::Some(EventFilterFn::make_box(
                                   [filter = std::move(filter)](TaskId id, Ref<RustTaskOptions> options) { return filter(id, options); })),
                               EventCallbackFn::make_box([callback = std::move(callback)](Ref<TaskMetadata> meta) {
                                   callback(meta);
                                   return Unit{};
                               }));
}
void TaskManager::unsubscribe(SubscriptionHandle& handle) {}

// -------------------------------------
// Task creation
// -------------------------------------

Task<void>::Task(FfiTaskVoid&& task) : m_task(std::move(task)) {}

template <>
Task<void> TaskManager::newTask(std::function<void()> f, std::optional<TaskOptions> options)
{
    return Task<void>(
        m_manager.new_blocking(BoxDyn<Fn<Result<Unit, TaskError>>, Send>::make_box([f = std::move(f)]() { return trycatch_unit(f); }),
                               transform_options_ffi(options)));
}
template <>
Task<void> TaskManager::newTask(std::function<void(Ref<TaskContext>)> f, std::optional<TaskOptions> options)
{
    return Task<void>(m_manager.new_blocking_with_ctx(BoxDyn<Fn<Ref<TaskContext>, Result<Unit, TaskError>>, Send>::make_box(
                                                          [f = std::move(f)](Ref<TaskContext> ctx) { return trycatch_unit(f, ctx); }),
                                                      transform_options_ffi(options)));
}

// -------------------------------------
// Task Continuation Specialisations
// -------------------------------------

// void Specialisation
template <>
Task<void> Task<void>::then(std::function<void(TaskResult<void>)> func, std::optional<TaskOptions> options)
{
    auto result = m_task.as_ref().then(TaskContFn<Unit, Unit>::make_box([func = std::move(func)](Result<Unit, TaskError> result) {
                                           auto task_result = TaskResult<void>(std::move(result));

                                           return trycatch_unit(func, std::move(task_result));
                                       }),
                                       transform_options_ffi(options));
    if (result.is_err()) {
        throw task_spawn_error(result.err().unwrap());
    }
    return result.unwrap();
}

template <>
Task<void> Task<void>::then(std::function<void(TaskResult<void>, Ref<TaskContext>)> func, std::optional<TaskOptions> options)
{
    auto result = m_task.as_ref().then_with_ctx(TaskContCtxFn<Unit, Unit>::make_box(

                                                    [func = std::move(func)](Result<Unit, TaskError> result, Ref<TaskContext> ctx) {
                                                        auto task_result = TaskResult<void>(std::move(result));

                                                        return trycatch_unit(func, std::move(task_result), ctx);
                                                    }),
                                                transform_options_ffi(options));

    if (result.is_err()) {
        throw task_spawn_error(result.err().unwrap());
    }
    return result.unwrap();
}

AbortHandle Task<void>::on_progress(std::function<void(Ref<TaskProgress>)> func) const
{
    return AbortHandle(m_task.as_ref().on_progress(
        Box<Dyn<Fn<Ref<TaskProgress>, Unit>, Send>>::make_box([func = std::move(func)](Ref<TaskProgress> progress) {
            func(progress);
            return Unit{};
        })));
}

TaskId Task<void>::id() const
{
    return m_task.as_ref().id();
}

bool Task<void>::is_finished() const
{
    return m_task.as_ref().is_finished();
}

bool Task<void>::is_canceled() const
{
    return m_task.as_ref().is_canceled();
}

void Task<void>::cancel()
{
    m_task.as_ref().cancel();
}

}  // namespace task
