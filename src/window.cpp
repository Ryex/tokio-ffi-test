#include "window.hpp"

#include <QVBoxLayout>
#include "ui_main.h"

MainWindow::MainWindow(QWidget* parent) : QMainWindow(parent), ui(new Ui::MainWindow)
{
    ui->setupUi(this);
    ui->label->setText("Press button to start tasks");
    ui->pushButton->setText("Start");
}

void MainWindow::setLabel(const QString& label)
{
    ui->label->setText(label);
}

void MainWindow::setProgress(int current)
{
    ui->progressBar->setValue(current);
}

void MainWindow::setProgressMaximum(int val)
{
    ui->progressBar->setMinimum(0);
    ui->progressBar->setMaximum(val);
}

void MainWindow::enableButton() {
  ui->pushButton->setEnabled(true);
}

void MainWindow::setOnstart(std::function<void(QPointer<QPushButton>)> func)
{
    if (m_onStartConnection) {
        QObject::disconnect(m_onStartConnection);
    }
    m_onStart = func;
    m_onStartConnection = QObject::connect(ui->pushButton, &QPushButton::clicked, this, [this]() { m_onStart(this->ui->pushButton); });
}
