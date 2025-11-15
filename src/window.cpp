#include "window.hpp"
#include <QIcon>
#include <QWindow>

#include <QVBoxLayout>
// #include "ui_main.h"

MainWindow::MainWindow(QWidget* parent) : QMainWindow(parent)
{
    setWindowIcon(QIcon::fromTheme(QIcon::ThemeIcon::MediaFlash));
    setupUi();
    label->setText("Press button to start tasks");
    pushButton->setText("Start");
}

void MainWindow::setupUi()
{
    if (this->objectName().isEmpty())
        this->setObjectName("this");
    this->resize(200, 150);
    centralwidget = new QWidget(this);
    centralwidget->setObjectName("centralwidget");
    QSizePolicy sizePolicy(QSizePolicy::Policy::Minimum, QSizePolicy::Policy::Minimum);
    sizePolicy.setHorizontalStretch(0);
    sizePolicy.setVerticalStretch(0);
    sizePolicy.setHeightForWidth(centralwidget->sizePolicy().hasHeightForWidth());
    centralwidget->setSizePolicy(sizePolicy);
    centralwidget->setMinimumSize(QSize(200, 150));
    centralwidget->setAutoFillBackground(true);
    verticalLayout_2 = new QVBoxLayout(centralwidget);
    verticalLayout_2->setObjectName("verticalLayout_2");
    label = new QLabel(centralwidget);
    label->setObjectName("label");

    verticalLayout_2->addWidget(label);

    progressBar = new QProgressBar(centralwidget);
    progressBar->setObjectName("progressBar");
    progressBar->setValue(0);

    verticalLayout_2->addWidget(progressBar);

    pushButton = new QPushButton(centralwidget);
    pushButton->setObjectName("pushButton");

    verticalLayout_2->addWidget(pushButton);

    this->setCentralWidget(centralwidget);

    // retranslateUi(this);

    QMetaObject::connectSlotsByName(this);
}

void MainWindow::setLabel(const QString& label)
{
    this->label->setText(label);
}

void MainWindow::setProgress(int current)
{
    progressBar->setValue(current);
}

void MainWindow::setProgressMaximum(int val)
{
    progressBar->setMinimum(0);
    progressBar->setMaximum(val);
}

void MainWindow::enableButton()
{
    pushButton->setEnabled(true);
}

void MainWindow::setOnstart(std::function<void(QPointer<QPushButton>)> func)
{
    if (m_onStartConnection) {
        QObject::disconnect(m_onStartConnection);
    }
    m_onStart = func;
    m_onStartConnection = QObject::connect(this->pushButton, &QPushButton::clicked, this, [this]() { m_onStart(this->pushButton); });
}
