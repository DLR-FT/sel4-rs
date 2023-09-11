cmake_minimum_required(VERSION 3.18.4)
project(sel4-application NONE)

# tools/cmake-tool/default-CMakeLists.txt
include(settings.cmake)

# This is a default file that can be copied/symlinked into the top level of a project
# if you have the expected directory layout and you want no customizations beyond
# the defaults provided in the all.cmake

# Try a bunch of different default locations for the script. The different locations
# come about from different project layouts depending on how far through the transition
# from the old build system they are
include(tools/seL4/cmake-tool/all.cmake OPTIONAL RESULT_VARIABLE found_all_cmake)
if(NOT found_all_cmake)
    include(tools/cmake-tool/all.cmake OPTIONAL RESULT_VARIABLE found_all_cmake)
    if(NOT found_all_cmake)
        message(FATAL_ERROR "No path for including cmake-tool/all.cmake was successful")
    endif()
endif()

# Customization
GenerateSimulateScript()
