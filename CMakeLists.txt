cmake_minimum_required(VERSION 3.23)

project(ffi-test)

set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)
set(CMAKE_AUTOUIC ON)

set(CMAKE_CXX_STANDARD_REQUIRED true)
set(CMAKE_C_STANDARD_REQUIRED true)
set(CMAKE_CXX_STANDARD 17)
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

# importing "tokio-ffi-test" will create the target as "tokio_ffi_test"
corrosion_import_crate(MANIFEST_PATH crates/Cargo.toml
  IMPORTED_CRATES imported_crates
  CRATES tokio-ffi-test
  FEATURES
)
message("-- Found crates: ${imported_crates}")

if(CMAKE_EXPORT_COMPILE_COMMANDS)
  set(CMAKE_CXX_STANDARD_INCLUDE_DIRECTORIES 
      ${CMAKE_CXX_IMPLICIT_INCLUDE_DIRECTORIES})
endif()

# fix crate target to attach it's includes
target_sources(tokio_ffi_test
  PUBLIC
  FILE_SET HEADERS
  BASE_DIRS crates/ffi/include
)

set(SOURCES
  src/app.cpp
  src/app.hpp
  src/window.cpp
  src/window.hpp
  src/TaskWatcher.hpp
  src/TaskWatcher.cpp
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

add_executable(tokio-ffi MACOSX_BUNDLE WIN32 src/main.cpp)
target_link_libraries(tokio-ffi ffitest_core)
