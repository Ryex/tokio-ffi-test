#pragma once

#include <QPointer>
#include <QMainWindow>

#include <QLabel>
#include <QPointer>
#include <QProgressBar>
#include <QPushButton>
#include <functional>

#include <QtWidgets/QVBoxLayout>
#include <QtWidgets/QWidget>

// namespace Ui {
// class MainWindow;
// }

class MainWindow : public QMainWindow {
    Q_OBJECT

   public:
    explicit MainWindow(QWidget* parent = 0);

    void setLabel(const QString& label);
    void setProgress(int current);
    void setProgressMaximum(int value);

    void setupUi();

    void enableButton();

    void setOnstart(std::function<void(QPointer<QPushButton>)>);

   private:
    // Ui::MainWindow* ui;

    QMetaObject::Connection m_onStartConnection;
    std::function<void(QPointer<QPushButton>)> m_onStart;

   private:
    QWidget* centralwidget;
    QVBoxLayout* verticalLayout_2;
    QLabel* label;
    QProgressBar* progressBar;
    QPushButton* pushButton;
};
