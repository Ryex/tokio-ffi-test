#include "tasks-ffi/tasks.hpp"

namespace task {

task_spawn_error::task_spawn_error(TaskSpawnError&& err) : task_spawn_error(err.to_string()) {}
task_spawn_error::task_spawn_error(String&& err)
    : std::runtime_error(std::string(reinterpret_cast<const char*>(err.as_bytes().as_ptr()), err.as_bytes().len()))
{}

AbortHandle::AbortHandle(Option<FfiAbortHandle>&& handle) : m_handle(std::move(handle)) {}

void AbortHandle::abort()
{
    if (m_handle.is_some()) {
        m_handle.as_ref().unwrap().abort();
    }
}

TaskManager::TaskManager() : m_manager(rust::crate::tasks::TaskManager::new_()) {}

template <typename T>
Task<T>::Task(FfiTaskAny&& task) : m_task(std::move(task))
{}

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

template <>
Task<void> Task<void>::then(std::function<void()> func, std::function<void(Ref<TaskError>)> fail, std::optional<TaskOptions> options)
{
    auto result = m_task.as_ref().then(BoxDyn<Fn<Result<Unit, TaskError>, Result<Unit, TaskError>>, Send>::make_box(
                                           [func = std::move(func), fail = std::move(fail)](Result<Unit, TaskError> result) {
                                               if (result.is_ok()) {
                                                   return trycatch_unit(func);
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

template <>
Task<void> Task<void>::then(std::function<void(Ref<TaskContext>)> func,
                            std::function<void(Ref<TaskError>, Ref<TaskContext>)> fail,
                            std::optional<TaskOptions> options)
{
    auto result = m_task.as_ref().then_with_ctx(
        BoxDyn<Fn<Result<Unit, TaskError>, Ref<TaskContext>, Result<Unit, TaskError>>, Send>::make_box(
            [func = std::move(func), fail = std::move(fail)](Result<Unit, TaskError> result, Ref<TaskContext> ctx) {
                if (result.is_ok()) {
                    return trycatch_unit(func, ctx);
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

AbortHandle Task<void>::on_progress(std::function<void(Ref<TaskProgress>)> func) const
{
    return AbortHandle(m_task.as_ref().on_progress(
        Box<Dyn<Fn<Ref<TaskProgress>, Unit>, Send>>::make_box([func = std::move(func)](Ref<TaskProgress> progress) {
            func(progress);
            return Unit{};
        })));
}

TokioId Task<void>::id() const
{
    return m_task.as_ref().id();
}

bool Task<void>::is_finished() const
{
    return m_task.as_ref().is_finished();
}

bool Task<void>::is_cancelled() const
{
    return m_task.as_ref().is_cancelled();
}

void Task<void>::cancel()
{
    m_task.as_ref().cancel();
}

}  // namespace task
