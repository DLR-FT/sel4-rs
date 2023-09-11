cmake_minimum_required(VERSION 3.18.4)
project(rootserver C)

add_executable(rootserver src/main.c)
target_link_libraries(rootserver sel4muslcsys  muslc)

DeclareRootserver(rootserver)

