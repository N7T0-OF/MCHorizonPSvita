# HorizonWiiU

Porting Horizon 26.3 to the Nintendo Wii U. Horizon source is kept at the repository root; Wii U entry-point, platform, and renderer work will live under `wiiu/`.

## Status

- Horizon upstream is checked out at `69fab597a6f0e8a6ce524d38d1b73e599e343034` (`main`, 2026-11-06).
- The read-only reference `C:\dev\personal\Codex\MCHorizonUWP` has a real native D3D11 FL9_1 rendering path on its `uwp-phone-d3d11` branch (local commit `43c80ed9b`). It accepts Horizon CPU `Frame`/`SectionMesh` data and implements terrain, entities, translucent passes, hand, HUD, depth, atlases and indexed meshes without routing rendering through `wgpu`. The public remote [`history/uwp-native-mipmaps`](https://github.com/megabytesme/MCHorizonUWP/tree/history/uwp-native-mipmaps) branch (`8a7f0148`) contains the same native-renderer architecture; `uwp-phone-d3d11` itself is local-only. The remote backend notes report x64 UWP readback checks and ARM32 compile/strict-link checks, not a verified phone run. This is the renderer reference to adapt architecturally, not a directly reusable graphics backend.
- Horizon's `horizon-client-render` crate owns the standard concrete `wgpu` renderer. The UWP reference added a `PresentationBackend` / `TerrainBackend` seam and a separate D3D11 renderer that consumes CPU `Frame`/`SectionMesh` data. Reuse that seam design for GX2; implement GX2-specific shaders, buffers, texture atlases, depth state and draw submission rather than calling `wgpu` or copying D3D11 code.
- The old `rust-wiiu/wut` Rust crate was archived on 2026-02-20. The current rust-wiiu organization is experimental, Linux-only, and has moved to `cafe-sys`, `cafe-target-spec`, a Rust fork, and `cargo-wiiu`. Book for U warns that its guide is being rewritten.
- WSL Ubuntu has upstream Rust nightly, `cargo-wiiu` 0.4.1, and the official devkitPro WUT `wiiu-dev` package. The project builds a Rust `no_std` static library, initializes WUT/GX2 for TV and GamePad, and packages RPX/WUHB. Horizon's `horizon-core` `wiiu-core` feature now exposes `BlockPos`/`SectionPos`/`ChunkPos`, its Java-compatible random streams, and its production Perlin noise sampler without `std`; `alloc`-based noise APIs link through a WUT/newlib `malloc`/`free` allocator. A new `horizon-terrain-data` crate shares the exact 24-byte `TerrainVertex` format between desktop and Wii U code. The no-capture build now creates one bounded 16×16×32 block chunk at runtime from seed 12345, stores air/grass/dirt/stone states, and emits exposed top/side faces as Horizon `TerrainVertex` quads. A height map from the same block data keeps the camera at eye height while the GamePad moves across the chunk. This first runtime generator uses Horizon's real RNG and Perlin primitive but not `horizon-world`'s vanilla density graph, registry, or proto-chunk pipeline. Capture builds still consume real captured `SectionMesh::solid` data and the captured sprite atlas/lightmap. A separate adapter expands quads to GX2 triangle lists and converts fixed-point UVs, light coordinates, and packed RGBA to GX2 floats. GX2 depth testing is enabled for TV and GamePad contexts. Translucent meshes remain unsupported.
- The adapter consumes actual `SectionMesh::solid` vertices from Horizon's render capture. `tools/horizon_wiiu_capture.py` ranks nonempty sections by camera distance, decodes their host-endian 24-byte records, combines the nearest sections within a vertex budget, rebases them around one camera-relative origin, and emits target-compiled C constants. Captures include Horizon's current block sprite atlas, 16×16 RGBA lightmap, and view parameters. The latest scratch Horizon 26.3 creative capture contains 5,412 sections, 98,236,704 mesh bytes, 21,315,584 sprite bytes, and 1,024 lightmap bytes. The converter selects 17 sections at the default 65,536-vertex budget, using 24 sprite layers packed into a guarded 330×330 RGBA atlas. The GX2 fragment shader bilinearly samples the captured colored lightmap per fragment using interpolated vertex sky/block coordinates; lighting is fixed to the captured frame. Capture builds use the captured yaw, pitch, FOV, and aspect ratio with Horizon's zero-to-one depth projection. The left stick and D-pad move relative to the view; the right stick changes yaw and pitch. Cemu 2.6 loaded the lightmap RPX and ran the title (latest checksum `0x55b4b4b8`). This verifies real Horizon mesh and texture conversion, cross-compilation, packaging, and RPX startup; it does not verify visual fidelity on hardware or GamePad controls in Cemu. Translucent meshes remain unsupported.

## Build and emulator results

The GX2 terrain quad-adapter build completed from WSL Ubuntu with:

```sh
cd /mnt/c/dev/personal/HorizonWiiU/wiiu/bootstrap
env HORIZON_WIIU_BUILD_DIR=/tmp/horizonwiiu-section-adapter \
  CAFEGLSL_ROOT=/tmp/horizonwiiu-triangle/deps/cafeglsl-1.0.0-rc1-linux-x86_64 \
  bash build.sh
```

The no_std adapter's host tests passed (2/2) with:

```sh
cd /tmp
CARGO_TARGET_DIR=/tmp/horizonwiiu-host-tests-final cargo +nightly test \
  --manifest-path /mnt/c/dev/personal/HorizonWiiU/wiiu/bootstrap/Cargo.toml \
  --lib --jobs "$(nproc)"
```

The capture converter host tests passed (4/4) with:

```sh
python -m unittest tests.test_horizon_wiiu_capture -v
```

The Wii U no_std terrain adapter tests passed (5/5) after adding captured perspective, camera-relative movement, and lightmap coordinates:

```sh
CARGO_TARGET_DIR=/tmp/horizonwiiu-horizon-tests cargo +nightly test \
  --manifest-path /mnt/c/dev/personal/HorizonWiiU/wiiu/bootstrap/Cargo.toml \
  --lib --jobs 32
```

The runtime chunk increment was tested from the workspace with:

```sh
CARGO_TARGET_DIR=/dev/shm/horizonwiiu-runtime-world-tests cargo +nightly test \
  --manifest-path wiiu/bootstrap/Cargo.toml --lib --jobs "$(nproc)"
CARGO_TARGET_DIR=/dev/shm/horizonwiiu-core-wiiu-tests cargo +nightly test \
  -p horizon-core --lib --no-default-features --features wiiu-core --jobs "$(nproc)"
```

Results: bootstrap 7/7 passed; the embedded `horizon-core` feature 14 passed, 1 ignored. The standard `horizon-core` suite also passed 126 tests, with 3 ignored. The `wiiu-core` feature tests exercise Horizon's Perlin, normal-noise, RNG, and position code in the same no_std configuration used by the Wii U dependency.

The PowerPC build and Cemu launch for the runtime chunk completed on 2026-11-06:

```sh
cd /mnt/c/dev/personal/HorizonWiiU/wiiu/bootstrap
HORIZON_WIIU_BUILD_DIR=/dev/shm/horizonwiiu-live-chunk bash build.sh
```

This produced ELF, RPX and WUHB files. The latest RPX SHA-256 is `176715e6dec6ffdb0fa2ef48d4f7875a2c0cbddcf580d1ae9aecd893bec420e0`; the WUHB SHA-256 is `0764259712f4708d014725df6e2eb1fa7420e0dca0c5748b3d0ef002b3cc7bd5`. Cemu 2.6 loaded that RPX (module checksum `0xf4900314`) and logged `Run title`; its window title reported about 60 FPS. This is a Cemu launch and active-title-loop check, not visual proof: no screenshot or GamePad movement was verified, and Cemu did not include the title's `OSReport` line in `log.txt`. No real-hardware test has been performed. Copies of the final RPX and WUHB were preserved in `%TEMP%\HorizonWiiU-live-world`.

`/dev/shm` is cleared when the WSL distro shuts down. Keep the distro alive while launching Cemu from `\\wsl.localhost\Ubuntu\dev\shm\horizonwiiu-live-chunk\horizon_wiiu_bootstrap.rpx`, or use a persistent build directory with enough free disk space.

To build with an existing Horizon render capture (see [docs/graphics.md](docs/graphics.md), “Render capture and replay”):

```sh
HORIZON_WIIU_CAPTURE_DIR=/path/to/capture \
HORIZON_WIIU_BUILD_DIR=/tmp/horizonwiiu-captured-section \
  bash build.sh
```

The verified real-capture build used WSL Ubuntu and the capture from a scratch Horizon world:

```sh
HORIZON_WIIU_CAPTURE_DIR=/tmp/horizonwiiu-lightmap-capture.iyr7c2 \
HORIZON_WIIU_BUILD_DIR=/tmp/horizonwiiu-lightmap-real \
CAFEGLSL_ROOT=/tmp/horizonwiiu-triangle/deps/cafeglsl-1.0.0-rc1-linux-x86_64 \
  bash build.sh
```

To make another capture, run Horizon with isolated data directories and the prepared 26.3 resources, then use `horizon-ctl render-capture on` followed by `horizon-ctl render-capture DIR`. WSLg needs its normal `XDG_RUNTIME_DIR` for the Wayland socket; keep the game, launcher, and XDG data/config directories in scratch locations. `docs/graphics.md` describes the capture contents.

To launch the RPX from PowerShell with the tested portable Cemu build:

```powershell
& "$env:TEMP\HorizonWiiU-Cemu-v2.6\Cemu_2.6\Cemu.exe" -g "\\wsl.localhost\Ubuntu\tmp\horizonwiiu-lightmap-real\horizon_wiiu_bootstrap.rpx"
```

Build directory: `/tmp/horizonwiiu-lightmap-real` for the real Horizon capture and `/tmp/horizonwiiu-lightmap-fixture` for the procedural fixture. Both produced an ELF32 big-endian PowerPC executable, RPX, and WUHB. The portable Cemu 2.6 build was launched with `-g` against the lightmap RPX. Its fresh `portable/log.txt` records `Loaded module 'horizon_wiiu_bootstrap'` (checksum `0x55b4b4b8`) followed by `Run title`. The log reports three RPLLoader section-size warnings (sections 11, 16, and 17), with no launch failure. This verifies RPX loading and title startup, not a screenshot-based visual check or a console run. Cemu GamePad input is not configured. The CafeGLSL auto-download path was separately exercised and verified its published SHA-256. Without `CAFEGLSL_ROOT`, `build.sh` downloads and verifies the compiler automatically.

## Architecture decisions

- Keep Horizon's source and gameplay/data crates identifiable and isolate Wii U code under `wiiu/`.
- Do not try to run `wgpu` directly on GX2. `gx2gl` is an OpenGL 3.3 translation layer, not a `wgpu` backend; using it would still require a renderer and shader/resource adaptation.
- Keep the CafeOS entry point in WUT C and application logic in Rust. Rust now builds one seeded 16×16×32 runtime chunk, exports its height map, and emits exposed faces using Horizon's shared `TerrainVertex`. The embedded build reuses Horizon's `LegacyRandom` and `PerlinNoise` implementations but has a small Wii U-specific block/material adapter; it does not run `horizon-world` generation or claim vanilla terrain parity. An alignment-aware `GlobalAlloc` implementation uses WUT/newlib `malloc` and `free` for Horizon's `alloc` APIs. The GX2 adapter expands each quad to six vertices and converts position, sprite UV, lightmap UV, and packed RGBA color. C polls VPAD, uploads through GX2R, binds the placeholder atlas and lightmap, enables depth testing, and follows the generated height map at 1.62 blocks eye height. The current movement is chunk-bounded horizontal walking without jumping or full block collision. Capture builds use Horizon's camera rotation and FOV with the same zero-to-one clip depth used by Horizon's `wgpu` projection; the right stick updates yaw and pitch. Captured sections are rebased to a shared camera-relative origin, converted into C numeric constants for target ABI layout, and rendered with the captured sprite atlas/lightmap. No desktop `wgpu` code is linked. Translucent meshes remain unsupported. WUT's `WHBGfxClearColor` clears color and depth for TV/GamePad ([implementation](https://github.com/devkitPro/wut/blob/master/libraries/libwhb/src/gfx.c)). `horizon-terrain-data` is `no_std`; the full `SectionMesh` and asset-driven client mesher remain in `horizon-client-render`, which depends on `std` and desktop libraries. GCC 16 emits a zero-size `.note.GNU-stack` marker that `elf2rpl` rejects, so packaging strips that non-runtime marker from a copy of the ELF first.
- Adapt the UWP `PresentationBackend` / `TerrainBackend` pattern to a GX2 backend that consumes Horizon CPU `SectionMesh` and frame data. The full desktop client remains coupled to `wgpu` and desktop window/input/audio dependencies and cannot be cross-compiled unchanged.

- A full Cafe `std` route exists upstream on the [`rust-wiiu/rust` `powerpc-nintendo-cafe` branch](https://github.com/rust-wiiu/rust/tree/powerpc-nintendo-cafe), with Cafe implementations for filesystem, threads, time, networking, random, and standard I/O. It is not installed or built here. The documented stage-1 compiler/library build needs roughly 10–15 GB of free build storage. C: is nearly full, but Ben-Desktop's E: volume has 61 GB free and is mounted in WSL at `/mnt/e`; the next step is to build the fork there and validate `horizon-world` under Cafe `std`.

## Next steps

1. Validate the `powerpc-nintendo-cafe` stage-1 toolchain, then cross-compile Horizon's real `GeneratorData::load` / `OverworldGenerator::worker().generate_proto` path with a legally supplied 26.3 worldgen bundle and block registry.
2. Feed `ProtoChunk::section_states` into a Wii U voxel mesher using real registry state/material mappings, then replace the current single chunk with neighboring chunk load/unload around the player.
3. Add block collision, vertical movement, block interaction, and save/load; configure Cemu input and visually verify the current camera/world before claiming those features.
4. After runtime world generation, update lighting from world state and add the translucent pass. Keep Cemu and real-hardware results distinct.
