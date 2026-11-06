cmake_minimum_required(VERSION 3.13)
project(horizon_wiiu_bootstrap C)

find_program(CARGO_EXECUTABLE cargo REQUIRED)
find_program(POWERPC_OBJCOPY_EXECUTABLE powerpc-eabi-objcopy REQUIRED)
find_program(CAFEGLSL_COMPILER_EXECUTABLE glslcompiler REQUIRED)
set(HORIZON_WIIU_CAPTURE_DIR "" CACHE PATH "Horizon render-capture directory; empty uses the fixture scene")
set(HORIZON_WIIU_CAPTURE_MAX_VERTICES 65536 CACHE STRING "Maximum captured solid vertices converted for GX2")

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
      ../../crates/horizon-core/Cargo.toml
      ../../crates/horizon-core/src/lib.rs
      ../../crates/horizon-core/src/position.rs
      ../../crates/horizon-core/src/version.rs
      src/lib.rs
   WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
   COMMENT "Building the Rust CafeOS application library"
   VERBATIM
)
add_custom_target(horizon_wiiu_rust DEPENDS "${RUST_STATICLIB}")

add_library(horizon_wiiu_rustlib STATIC IMPORTED GLOBAL)
set_target_properties(horizon_wiiu_rustlib PROPERTIES IMPORTED_LOCATION "${RUST_STATICLIB}")

set(GSH "${CMAKE_CURRENT_BINARY_DIR}/horizon_block.gsh")
set(GSH_OBJECT "${CMAKE_CURRENT_BINARY_DIR}/horizon_block_gsh.o")
add_custom_command(
   OUTPUT "${GSH}"
   COMMAND "${CAFEGLSL_COMPILER_EXECUTABLE}"
      -vs "${CMAKE_CURRENT_SOURCE_DIR}/shaders/block.vert"
      -ps "${CMAKE_CURRENT_SOURCE_DIR}/shaders/block.frag"
      -o "${GSH}"
   DEPENDS shaders/block.vert shaders/block.frag
   COMMENT "Compiling the GX2 block shaders"
   VERBATIM
)
add_custom_command(
   OUTPUT "${GSH_OBJECT}"
   COMMAND "${POWERPC_OBJCOPY_EXECUTABLE}"
      -I binary -O elf32-powerpc -B powerpc:common
      horizon_block.gsh horizon_block_gsh.o
   DEPENDS "${GSH}"
   WORKING_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}"
   COMMENT "Embedding the GFD shader file in the RPX"
   VERBATIM
)
set_source_files_properties("${GSH_OBJECT}" PROPERTIES GENERATED TRUE EXTERNAL_OBJECT TRUE)
add_custom_target(horizon_wiiu_shaders DEPENDS "${GSH_OBJECT}")

set(HORIZON_WIIU_CAPTURE_SOURCE "")
if(HORIZON_WIIU_CAPTURE_DIR)
   find_package(Python3 REQUIRED COMPONENTS Interpreter)
   set(HORIZON_WIIU_CAPTURE_SOURCE "${CMAKE_CURRENT_BINARY_DIR}/horizon_wiiu_capture.c")
   add_custom_command(
      OUTPUT "${HORIZON_WIIU_CAPTURE_SOURCE}"
      COMMAND "${Python3_EXECUTABLE}"
         "${CMAKE_CURRENT_SOURCE_DIR}/../../tools/horizon_wiiu_capture.py"
         "${HORIZON_WIIU_CAPTURE_DIR}"
         --output "${HORIZON_WIIU_CAPTURE_SOURCE}"
         --max-vertices "${HORIZON_WIIU_CAPTURE_MAX_VERTICES}"
      DEPENDS
         "${CMAKE_CURRENT_SOURCE_DIR}/../../tools/horizon_wiiu_capture.py"
         "${HORIZON_WIIU_CAPTURE_DIR}/scene.json"
         "${HORIZON_WIIU_CAPTURE_DIR}/sections.json"
         "${HORIZON_WIIU_CAPTURE_DIR}/sections.bin"
         "${HORIZON_WIIU_CAPTURE_DIR}/sprites.bin"
   COMMENT "Converting captured Horizon sections for GX2"
      VERBATIM
   )
endif()

add_executable(horizon_wiiu_bootstrap src/main.c "${GSH_OBJECT}" "${HORIZON_WIIU_CAPTURE_SOURCE}")
if(HORIZON_WIIU_CAPTURE_SOURCE)
   target_compile_definitions(horizon_wiiu_bootstrap PRIVATE HORIZON_WIIU_CAPTURE=1)
endif()
add_dependencies(horizon_wiiu_bootstrap horizon_wiiu_rust)
add_dependencies(horizon_wiiu_bootstrap horizon_wiiu_shaders)
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
