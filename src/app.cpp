#include "app.hpp"
#include <qfuturewatcher.h>

#include <QCommandLineParser>
#include <QFuture>
#include <QPromise>
#include <QPushButton>
#include <QRandomGenerator>

#include "task-ffi/task.hpp"
#include "task-ffi/util.hpp"

#include <chrono>
#include <exception>
#include <stdexcept>
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
        m_taskWatcher.result().visit<void>(
            [this](long value) {
                m_mainWindow->setLabel(QString("final value %1").arg(value));
                m_mainWindow->enableButton();
            },
            [this](task::TaskError error) {
                auto msg_rust = error.to_string();
                auto msg = QString::fromUtf8(reinterpret_cast<const char*>(msg_rust.as_str().as_ptr()), msg_rust.len());
                if (auto inner_err = error.as_error(); inner_err.is_some()) {
                    if (auto ex = inner_err.unwrap().as_external(); ex.is_some()) {
                        try {
                            std::rethrow_exception(ex.unwrap().cpp().exception());
                        } catch (const std::runtime_error& err) {
                            msg += " (caught std::runtime_error)";
                        } catch (const std::invalid_argument& err) {
                            msg += " (caught std::invalid_argument)";
                        } catch (const std::exception& err) {
                            msg += " (caught std::exception)";
                        } catch (...) {
                            msg += " (caught unknown error)";
                        }
                    }
                }
                m_mainWindow->setLabel(QString("Error in Task: %1").arg(msg));
                m_mainWindow->enableButton();
            });
    });

    connect(&m_taskWatcher, &TaskWatcher<std::int64_t>::progressChanged, this, [this](uint64_t progress, uint64_t maximum) {
        m_mainWindow->setProgressMaximum(maximum);
        m_mainWindow->setProgress(progress);
    });

    m_mainWindow->setOnstart([this](QPointer<QPushButton> button) {
        button->setEnabled(false);

        auto task = this->m_taskManager.newTask<std::int64_t>(
            [](task::RefTaskContext ctx) {
                //
                // We are now in a new thread behind the Rust runtime but runnign a c++ closure!
                //

                int n = QRandomGenerator::global()->bounded(100);  // Number of Fibonacci terms to generate
                if (n % 3 == 0) {
                    throw std::runtime_error("No Muptiples of 3!");
                } else if (n % 5 == 0) {
                    throw std::invalid_argument("I don't like numbers divisible by 5");
                } else if (n % 4 == 0) {
                    throw "an unknown string error!?";
                }

                std::int64_t t1 = 0, t2 = 1, nextTerm;

                // the task context object, because we asked for it this task comes with a progress sender
                // lets set the maximum progress value
                ctx.set_progress_maximum(n);

                // tasks record thier id and context into thread_local storage and they can retreaved at any time!
                qDebug() << "TaskId:" << rust_util::to_string_view(task::current::id().to_string());
                auto tl_ctx = task::current::context();
                auto task_name = tl_ctx.name();
                // the `rust_util` namespace contains some tools to make working with rust types eaiser in c++
                qDebug() << "Current Task Name: "
                         << rust_util::to_string_view(task_name.matches_Some() ? task_name.unwrap().as_str() : "Option::None"_rs);

                auto dbg_line = qDebug();
                auto tags = tl_ctx.tags();
                dbg_line << "Task Tags:";
                // `rust_util::collection::CxxIterable` turns a type exposing the rust `std::iter::Iter` trait
                // interface into a c++ forward iterator! carefull under the hood it calls `unwrap()`
                // so each index can only beretreaed from the iterator once. best to leave iterating over it to the foreach loop.
                for (auto tag : rust_util::collection::CxxIterable(tags)) {
                    dbg_line << rust_util::to_string_view(tag);
                }

                for (int i = 2; i < n; ++i) {
                    nextTerm = t1 + t2;
                    t1 = t2;
                    t2 = nextTerm;
                    // set the current task progress and send a progress update!
                    ctx.set_progress(i);
                    // we're in another thread!~ this won't block the ui
                    std::this_thread::sleep_for(std::chrono::milliseconds(10));
                }
                ctx.set_progress(n);

                // return a value from our task
                return nextTerm;
            },
            task::TaskOptions("A_Task_Name").withTags(std::vector<std::string>{ "a-tag", "another-tag" }));
        m_mainWindow->setLabel("Working ...");
        m_taskWatcher.setTask(std::move(task));
        QPointer<MainWindow> window = m_mainWindow;
    });
}
