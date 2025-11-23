#pragma once

#include <QObject>
#include <mutex>
#include <optional>

#include <QDebug>

#include "task-ffi/task.hpp"

struct task_not_finished_error : public std::runtime_error {
   public:
    task_not_finished_error() : std::runtime_error("Can not fetch result, Task has not finished.") {};
};

struct task_result_moved : public std::runtime_error {
   public:
    task_result_moved() : std::runtime_error("Can not fetch result, Result already moved.") {};
};

struct task_not_assigned_error : public std::runtime_error {
   public:
    task_not_assigned_error() : std::runtime_error("Can not fetch task id, Task has been assigned.") {};
};

class TaskWatcherBase : public QObject {
    Q_OBJECT

   public:
    TaskWatcherBase(QObject* parent) : QObject(parent) {}

   signals:

    void taskFinished() const;
    void progressChanged(uint64_t progress, uint64_t maximum) const;

   public slots:
    virtual void cancel() = 0;

   protected:
    void emitProgress(uint64_t progress, uint64_t maximum) const;
    void emitTaskFinished() const;
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
    std::optional<::task::AbortHandle> m_progress_handle;
    std::optional<::task::Task<void>> m_continuation_handle;

    mutable std::mutex m_mutex;
    mutable bool m_result_taken;
    std::optional<task::TaskResult<T>> m_result;

   public:
    TaskWatcher(QObject* parent = nullptr)
        : TaskWatcherBase(parent), m_task(std::nullopt), m_result(std::nullopt), m_continuation_handle(std::nullopt), m_result_taken(false)
    {}
    TaskWatcher(task::Task<T>&& task, QObject* parent = nullptr)
        : TaskWatcherBase(parent)
        , m_task(std::move(task))
        , m_result(std::nullopt)
        , m_continuation_handle(std::nullopt)
        , m_result_taken(false)
    {
        setUpTask();
    }

    void setUpTask()
    {
        m_progress_handle =
            m_task->on_progress([this](task::RefTaskProgress progress) { this->emitProgress(progress.progress(), progress.maximum()); });
        m_continuation_handle = m_task->then([this](task::TaskResult<T> result) {
            {
                std::lock_guard<std::mutex> guard(this->m_mutex);
                this->m_result = std::move(result);
                this->m_result_taken = false;
            }
            this->emitTaskFinished();
        });
    }

    void setTask(task::Task<T>&& task)
    {
        m_task = std::move(task);
        setUpTask();
    }

    bool isRunning() const { return m_task && !m_task.isFinished(); }
    bool isFinished() const { return m_task && m_task.isFinished(); }
    bool isCanceled() const { return m_task && m_task.isCanceled(); }

    task::TaskId taskId() const
    {
        if (!m_task) {
            throw task_not_assigned_error();
        }
        return m_task->id();
    }

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

    task::TaskResult<T> result()
    {
        std::lock_guard<std::mutex> guard(m_mutex);
        if (m_result && !m_result_taken) {
            m_result_taken = true;
            return *std::exchange(m_result, std::nullopt);    
        } else if (m_result_taken) {
            throw task_result_moved();
        } else {
            throw task_not_finished_error();
        }
    }
};
