cmake_minimum_required(VERSION 3.13)
project(horizon_wiiu_bootstrap C)

find_program(CARGO_EXECUTABLE cargo REQUIRED)
find_program(POWERPC_OBJCOPY_EXECUTABLE powerpc-eabi-objcopy REQUIRED)

set(RUST_TARGET_DIR "${CMAKE_CURRENT_BINARY_DIR}/rust-target")
set(RUST_TARGET_NAME "powerpc-cafe-nintendo")
set(RUST_STATICLIB "${RUST_TARGET_DIR}/${RUST_TARGET_NAME}/release/libhorizon_wiiu_bootstrap.a")

add_custom_command(
   OUTPUT "${RUST_STATICLIB}"
   COMMAND "${CMAKE_COMMAND}" -E env
      "CARGO_TARGET_DIR=${RUST_TARGET_DIR}"
      "${CARGO_EXECUTABLE}" +nightly build --release --lib --manifest-path "${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml"
   DEPENDS
      Cargo.toml
      rust-toolchain.toml
      .cargo/config.toml
      .cafe/powerpc-cafe-nintendo.json
      .cafe/linker.ld
      src/lib.rs
   WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
   COMMENT "Building the Rust CafeOS application library"
   VERBATIM
)
add_custom_target(horizon_wiiu_rust DEPENDS "${RUST_STATICLIB}")

add_library(horizon_wiiu_rustlib STATIC IMPORTED GLOBAL)
set_target_properties(horizon_wiiu_rustlib PROPERTIES IMPORTED_LOCATION "${RUST_STATICLIB}")

add_executable(horizon_wiiu_bootstrap src/main.c)
add_dependencies(horizon_wiiu_bootstrap horizon_wiiu_rust)
target_link_libraries(horizon_wiiu_bootstrap PRIVATE horizon_wiiu_rustlib)

set(PACKAGE_ELF "${CMAKE_CURRENT_BINARY_DIR}/horizon_wiiu_bootstrap.package.elf")
set(RPX "${CMAKE_CURRENT_BINARY_DIR}/horizon_wiiu_bootstrap.rpx")
set(WUHB "${CMAKE_CURRENT_BINARY_DIR}/horizon_wiiu_bootstrap.wuhb")

# GCC 16 emits a zero-sized .note.GNU-stack section in its startup object.
# Both elf2rpl and cargo-wiiu 0.4.1 currently reject it, so remove this
# link-time-only marker from a copy before converting the executable to RPX.
add_custom_command(
   OUTPUT "${RPX}"
   COMMAND "${CMAKE_COMMAND}" -E copy "$<TARGET_FILE:horizon_wiiu_bootstrap>" "${PACKAGE_ELF}"
   COMMAND "${POWERPC_OBJCOPY_EXECUTABLE}" --strip-debug --remove-section=.note.GNU-stack "${PACKAGE_ELF}"
   COMMAND "${CARGO_EXECUTABLE}" wiiu rpx "${PACKAGE_ELF}" "${RPX}"
   DEPENDS horizon_wiiu_bootstrap
   COMMENT "Packaging CafeOS ELF as RPX"
   VERBATIM
)
add_custom_target(horizon_wiiu_rpx ALL DEPENDS "${RPX}")

add_custom_command(
   OUTPUT "${WUHB}"
   COMMAND "${CARGO_EXECUTABLE}" wiiu wuhb --name "Horizon Wii U Bootstrap" "${RPX}" "${WUHB}"
   DEPENDS "${RPX}"
   COMMENT "Packaging RPX as WUHB"
   VERBATIM
)
add_custom_target(horizon_wiiu_package ALL DEPENDS "${WUHB}")
