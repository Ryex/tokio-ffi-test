#include "TaskWatcher.hpp"
#include "task-ffi/task.hpp"

#include <QEventLoop>

void TaskWatcherBase::emitProgress(uint64_t progress, uint64_t maximum) const
{
    emit progressChanged(progress, maximum);
}

void TaskWatcherBase::emitTaskSucceded() const
{
    emit taskSucceded();
    emit taskFinished();
}
void TaskWatcherBase::emitTaskFailed() const
{
    emit taskFailed();
    emit taskFinished();
}
void TaskWatcherBase::emitTaskCanceled() const
{
    emit taskCanceled();
    emit taskFinished();
}


void TaskProgressWatcher::cancel()
{
    if (m_progress_handle) {
        m_progress_handle->abort();
    }
}
