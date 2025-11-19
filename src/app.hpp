#pragma once

#include <QApplication>

#include <QFutureWatcher>

#include "window.hpp"
#include "task-ffi/task.hpp"
#include "TaskWatcher.hpp"

class Application : public QApplication {
    Q_OBJECT
   public:
    Application(int& argc, char** argv);

    void showMainWindow();
    void setUpTasks();

   private:
    QPointer<MainWindow> m_mainWindow = nullptr;
    task::TaskManager m_taskManager;

    TaskWatcher<std::int64_t> m_taskWatcher;

};
