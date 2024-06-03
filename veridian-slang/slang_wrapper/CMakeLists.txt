cmake_minimum_required(VERSION 3.20)

project(
    slang_wrapper
    LANGUAGES CXX
)

# Keep the version the same as the one in `build.rs`
find_package(slang 6.0 REQUIRED)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

add_library(
    slang_wrapper
    STATIC
    src/slang_lib.cpp
    src/basic_client.cpp
)
target_link_libraries(slang_wrapper PRIVATE slang::slang)
set_target_properties(slang_wrapper PROPERTIES OUTPUT_NAME "slangwrapper")

install(
    TARGETS slang_wrapper
    LIBRARY DESTINATION lib
    RUNTIME DESTINATION bin
)
