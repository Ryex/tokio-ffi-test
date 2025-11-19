#include "app.hpp"
#include <qfuturewatcher.h>

#include <QCommandLineParser>
#include <QFuture>
#include <QPromise>
#include <QPushButton>
#include <memory>

#include "task-ffi/task.hpp"

#include <chrono>
#include <thread>

#include <QtLogging>

Application::Application(int& argc, char** argv) : QApplication(argc, argv), m_taskWatcher()
{
    QCommandLineParser parser;
    parser.addHelpOption();
    parser.addVersionOption();

    parser.process(arguments());
}

void Application::showMainWindow()
{
    if (m_mainWindow) {
        m_mainWindow->raise();
    } else {
        m_mainWindow = new MainWindow(nullptr);
        m_mainWindow->show();
    }
}

void Application::setUpTasks()
{
    connect(&m_taskWatcher, &TaskWatcher<std::int64_t>::taskFinished, this, [this]() {
        m_mainWindow->setLabel(QString("final value %1").arg(m_taskWatcher.result()));
        m_mainWindow->enableButton();
    });

    connect(&m_taskWatcher, &TaskWatcher<std::int64_t>::progressChanged, this, [this](uint64_t progress, uint64_t maximum) {
        m_mainWindow->setProgressMaximum(maximum);
        m_mainWindow->setProgress(progress);
    });

    m_mainWindow->setOnstart([this](QPointer<QPushButton> button) {
        button->setEnabled(false);

        auto task = this->m_taskManager.newTask<std::int64_t>(
            [](task::RefTaskContext ctx) {
                int n = 20;  // Number of Fibonacci terms to generate
                std::int64_t t1 = 0, t2 = 1, nextTerm;

                ctx.set_progress_maximum(n);

                qDebug() << "TaskId:" << rust_util::string::to_string(task::current::id().to_string());

                for (int i = 2; i < n; ++i) {
                    nextTerm = t1 + t2;
                    t1 = t2;
                    t2 = nextTerm;
                    ctx.set_progress(i);
                    std::this_thread::sleep_for(std::chrono::milliseconds(100));
                }
                ctx.set_progress(n);

                return nextTerm;
            },
            task::TaskOptions());
        m_mainWindow->setLabel("Working ...");
        m_taskWatcher.setTask(std::move(task));
        QPointer<MainWindow> window = m_mainWindow;
    });
}
