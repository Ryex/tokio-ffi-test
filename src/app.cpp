#include "app.hpp"
#include <qfuturewatcher.h>

#include <QCommandLineParser>
#include <QFuture>
#include <QPromise>
#include <QPushButton>
#include <memory>

#include "tasks-ffi/tasks.hpp"

#include <chrono>
#include <thread>

Application::Application(int& argc, char** argv) : QApplication(argc, argv)
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
    connect(&m_futureWatcher, &QFutureWatcher<std::int64_t>::finished, this, [this]() {
        m_mainWindow->setLabel(QString("final value %1").arg(m_futureWatcher.result()));
        m_mainWindow->enableButton();
    });

    connect(&m_futureWatcher, &QFutureWatcher<std::int64_t>::progressValueChanged, this,
            [this](int progress) { m_mainWindow->setProgress(progress); });

    connect(&m_futureWatcher, &QFutureWatcher<std::int64_t>::progressRangeChanged, this,
            [this](int min, int maximum) { m_mainWindow->setProgressMaximum(maximum); });

    m_mainWindow->setOnstart([this](QPointer<QPushButton> button) {
        button->setEnabled(false);

        std::shared_ptr<QPromise<std::int64_t>> promise = std::make_shared<QPromise<std::int64_t>>();
        QFuture<std::int64_t> future = promise->future();
        m_futureWatcher.setFuture(future);
        auto task = this->m_taskManager.newTask<std::int64_t>([](task::RefTaskContext ctx) {
            int n = 20;  // Number of Fibonacci terms to generate
            std::int64_t t1 = 0, t2 = 1, nextTerm;

            ctx.set_progress_maximum(n);

            for (int i = 2; i < n; ++i) {
                nextTerm = t1 + t2;
                t1 = t2;
                t2 = nextTerm;
                ctx.set_progress(i);
                std::this_thread::sleep_for(std::chrono::milliseconds(100));
            }
            ctx.set_progress(n);

            return nextTerm;
        }, task::TaskOptions());
        m_mainWindow->setLabel("Working ...");
        promise->start();
        auto t = task.then(
            [promise](int val) mutable {
                promise->addResult(val);
                promise->finish();
            },
            [](task::RefTaskError) {});
        QPointer<MainWindow> window = m_mainWindow;
        // don't try to capture the window, this will get called from another thread;
        task.on_progress([promise](task::RefTaskProgress progress) {
            promise->setProgressRange(0, progress.maximum());
            promise->setProgressValue(progress.progress());
        });
    });
}
