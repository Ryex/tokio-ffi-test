#include "app.hpp"

int main(int argc, char* argv[])
{
  Application app(argc, argv);

  app.showMainWindow();
  app.setUpTasks();

  app.exec();

}
