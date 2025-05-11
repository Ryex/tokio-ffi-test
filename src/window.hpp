#pragma once

#include <qpointer.h>
#include <QMainWindow>

#include <QLabel>
#include <QPointer>
#include <QProgressBar>
#include <QPushButton>
#include <functional>

namespace Ui {
class MainWindow;
}

class MainWindow : public QMainWindow {
    Q_OBJECT

   public:
    explicit MainWindow(QWidget* parent = 0);

    void setLabel(const QString& label);
    void setProgress(int current);
    void setProgressMaximum(int value);

    void enableButton();

    void setOnstart(std::function<void(QPointer<QPushButton>)>);

   private:
    Ui::MainWindow* ui;

    QMetaObject::Connection m_onStartConnection;
    std::function<void(QPointer<QPushButton>)> m_onStart;
};
