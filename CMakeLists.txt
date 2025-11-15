cmake_minimum_required(VERSION 3.23)

project(ffi-test)

set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)
set(CMAKE_AUTOUIC ON)

set(CMAKE_CXX_STANDARD_REQUIRED true)
set(CMAKE_C_STANDARD_REQUIRED true)
set(CMAKE_CXX_STANDARD 20)
set(CMAKE_C_STANDARD 11)

set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

include(FetchContent)

FetchContent_Declare(
    Corrosion
    GIT_REPOSITORY https://github.com/corrosion-rs/corrosion.git
    GIT_TAG v0.5.2 # Optionally specify a commit hash, version tag or branch here
)
# Set any global configuration variables such as `Rust_TOOLCHAIN` before this line!
FetchContent_MakeAvailable(Corrosion)

find_package(Qt6 REQUIRED COMPONENTS Core CoreTools Widgets Concurrent)

corrosion_import_crate(MANIFEST_PATH crates/Cargo.toml
  IMPORTED_CRATES imported_crates
)
message("-- Found crates: ${imported_crates}")

target_sources(tokio_ffi_test
  PUBLIC
  FILE_SET HEADERS
  BASE_DIRS crates/ffi/include
  FILES
  crates/ffi/include/tasks-ffi/generated.h
  crates/ffi/include/tasks-ffi/types.hpp
  crates/ffi/include/tasks-ffi/tasks.hpp
)
  

set(SOURCES
  src/app.cpp
  src/app.hpp
  src/window.cpp
  src/window.hpp
  # src/main.ui
)

add_library(ffitest_core STATIC ${SOURCES})
target_include_directories(ffitest_core PRIVATE src)
target_link_libraries(ffitest_core
  tokio_ffi_test
  Qt6::Core
  Qt6::Concurrent
  Qt6::Gui
  Qt6::Widgets
)

add_executable(tokio-ffi-test MACOSX_BUNDLE WIN32 src/main.cpp)
target_link_libraries(tokio-ffi-test ffitest_core)
