list(
	APPEND
	CMAKE_MODULE_PATH
	"${CMAKE_SOURCE_DIR}/kernel"
	"${CMAKE_SOURCE_DIR}/tools/cmake-tool/helpers")

include(application_settings)

correct_platform_strings()

find_package(seL4 REQUIRED)
sel4_configure_platform_settings()

ApplyCommonSimulationSettings("${KernelSel4Arch}")
ApplyCommonReleaseVerificationSettings(${RELEASE} ${VERIFICATION})
