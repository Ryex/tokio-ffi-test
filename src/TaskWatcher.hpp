#pragma once

#include <QObject>
#include <optional>

#include "task-ffi/task.hpp"

struct task_not_succeded_error : public std::runtime_error {
   public:
    task_not_succeded_error() : std::runtime_error("Can not fetch result, Task has not finished.") {};
};

struct task_not_failed_error : public std::runtime_error {
   public:
    task_not_failed_error() : std::runtime_error("Can not fetch error, Task has not failed.") {};
};

class TaskWatcherBase : public QObject {
    Q_OBJECT

   public:
    TaskWatcherBase(QObject* parent) : QObject(parent) {}

   signals:

    void taskSucceded() const;
    void taskFailed() const;
    void taskFinished() const;
    void taskCanceled() const;
    void progressChanged(uint64_t progress, uint64_t maximum) const;

   public slots:
    virtual void cancel() = 0;

   protected:
    void emitProgress(uint64_t progress, uint64_t maximum) const;
    void emitTaskSucceded() const;
    void emitTaskFailed() const;
    void emitTaskCanceled() const;
};

class TaskProgressWatcher : public QObject {
    Q_OBJECT

   private:
    std::optional<::task::AbortHandle> m_progress_handle;

   public:
    TaskProgressWatcher(QObject* parent) : QObject(parent) {}

    template <typename T>
    TaskProgressWatcher(task::Task<T> task, QObject* parent) : QObject(parent)
    {
        setTask(task);
    }

    template <typename T>
    void setTask(task::Task<T>& task)
    {
        m_progress_handle =
            task.on_progress([this](task::RefTaskProgress progress) { this->emitProgress(progress.progress(), progress.maximum()); });
    }

   signals:
    void progressChanged(uint64_t progress, uint64_t maximum) const;

   public slots:
    void cancel();

   protected:
    void emitProgress(uint64_t progress, uint64_t maximum) const;
};

template <typename T>
class TaskWatcher : public TaskWatcherBase {
   private:
    std::optional<task::Task<T>> m_task;
    std::optional<T> m_result;
    std::optional<::task::AbortHandle> m_progress_handle;
    std::optional<::task::Task<void>> m_continuation_handle;
    std::optional<std::string> m_error;

   public:
    TaskWatcher(QObject* parent = nullptr) : TaskWatcherBase(parent), m_task() {}
    TaskWatcher(task::Task<T>&& task, QObject* parent = nullptr) : TaskWatcherBase(parent), m_task() { setTask(std::move(task)); }

    void setTask(task::Task<T>&& task)
    {
        m_task = std::optional(std::move(task));
        m_progress_handle =
            m_task->on_progress([this](task::RefTaskProgress progress) { this->emitProgress(progress.progress(), progress.maximum()); });
        m_continuation_handle = m_task->then(
            [this](T val) {
                this->m_result = val;
                this->emitTaskSucceded();
            },
            [this](task::RefTaskError err) {
                if (err.as_task_canceled().is_some()) {
                    this->emitTaskCanceled();
                } else {
                    this->m_error = rust_util::string::to_string(err.to_string());
                    this->emitTaskFailed();
                }
            });
    }

    bool isRunning() const { return m_task && !m_task.isFinished(); }
    bool isFinished() const { return m_task && m_task.isFinished(); }
    bool isCanceled() const { return m_task && m_task.isCanceled(); }

    task::TaskId taskId() const { return m_task->id(); }

    void cancel() override
    {
        if (m_task) {
            m_task->cancel();
        }
        if (m_continuation_handle) {
            m_continuation_handle->cancel();
        }
        if (m_progress_handle) {
            m_progress_handle->abort();
        }
    };

    T result() const
    {
        if (m_result) {
            return *m_result;
        } else {
            throw task_not_succeded_error();
        }
    }
    std::string error() const
    {
        if (m_error) {
            return *m_error;
        } else {
            throw task_not_failed_error();
        }
    }

    std::optional<T> tryResult() const { return m_result; }
    std::optional<std::string> tryError() const { return m_error; };
};
