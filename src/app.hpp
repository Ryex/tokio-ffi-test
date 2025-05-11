#pragma once

#include <QApplication>

#include <QFutureWatcher>

#include "window.hpp"
#include "tasks-ffi/tasks.hpp"

class Application : public QApplication {
    Q_OBJECT
   public:
    Application(int& argc, char** argv);

    void showMainWindow();
    void setUpTasks();

   private:
    QPointer<MainWindow> m_mainWindow = nullptr;
    task::TaskManager m_taskManager;

    QFutureWatcher<std::int64_t> m_futureWatcher;
};
