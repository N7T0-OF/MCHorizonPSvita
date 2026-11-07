# HorizonWiiU

Porting Horizon 26.3 to the Nintendo Wii U. Horizon source is kept at the repository root; Wii U entry-point, platform, and renderer work will live under `wiiu/`.

## Status

- Horizon upstream is checked out at `69fab597a6f0e8a6ce524d38d1b73e599e343034` (`main`, 2026-11-06).
- The read-only reference `C:\dev\personal\Codex\MCHorizonUWP` has a real native D3D11 FL9_1 rendering path on its `uwp-phone-d3d11` branch (local commit `43c80ed9b`). It accepts Horizon CPU `Frame`/`SectionMesh` data and implements terrain, entities, translucent passes, hand, HUD, depth, atlases and indexed meshes without routing rendering through `wgpu`. The public remote [`history/uwp-native-mipmaps`](https://github.com/megabytesme/MCHorizonUWP/tree/history/uwp-native-mipmaps) branch (`8a7f0148`) contains the same native-renderer architecture; `uwp-phone-d3d11` itself is local-only. The remote backend notes report x64 UWP readback checks and ARM32 compile/strict-link checks, not a verified phone run. This is the renderer reference to adapt architecturally, not a directly reusable graphics backend.
- Horizon's `horizon-client-render` crate owns the standard concrete `wgpu` renderer. The UWP reference added a `PresentationBackend` / `TerrainBackend` seam and a separate D3D11 renderer that consumes CPU `Frame`/`SectionMesh` data. Reuse that seam design for GX2; implement GX2-specific shaders, buffers, texture atlases, depth state and draw submission rather than calling `wgpu` or copying D3D11 code.
- The old `rust-wiiu/wut` Rust crate was archived on 2026-02-20. The current rust-wiiu organization is experimental, Linux-only, and has moved to `cafe-sys`, `cafe-target-spec`, a Rust fork, and `cargo-wiiu`. Book for U warns that its guide is being rewritten.
- WSL Ubuntu has upstream Rust nightly, `cargo-wiiu` 0.4.1, and the official devkitPro WUT `wiiu-dev` package. The project builds a Rust `no_std` static library, initializes WUT/GX2 for TV and GamePad, and packages RPX/WUHB. Horizon's `horizon-core` `wiiu-core` feature now exposes `BlockPos`/`SectionPos`/`ChunkPos`, its Java-compatible random streams, and its production Perlin noise sampler without `std`; `alloc`-based noise APIs link through a WUT/newlib `malloc`/`free` allocator. A new `horizon-terrain-data` crate shares the exact 24-byte `TerrainVertex` format between desktop and Wii U code. The default `build.sh` path now uses the Cafe-`std` worldgen bridge: it loads Horizon's registry, terrain assets, biome data, and `horizon-world` generator, then meshes runtime proto-chunks with Horizon's original `horizon-terrain-mesh::Mesher` before GX2 submission. The default-path Cafe cross-build and RPX/WUHB packaging pass. The newest instrumented package loaded in Cemu and logged `Run title` (checksum `0x5d341cdb`), but emitted no captured console stream or fresh Horizon log file and has no verified frame. The explicitly selected no-worldgen fixture is separate diagnostic code, not the client path. Capture builds consume real captured `SectionMesh::solid` data and the captured sprite atlas/lightmap. The GX2 adapter expands quads to triangle lists and converts fixed-point UVs, light coordinates, and packed RGBA to GX2 floats. Depth testing is enabled for TV and GamePad contexts; translucent meshes remain unsupported.
- The adapter consumes actual `SectionMesh::solid` vertices from Horizon's render capture. `tools/horizon_wiiu_capture.py` ranks nonempty sections by camera distance, decodes their host-endian 24-byte records, combines the nearest sections within a vertex budget, rebases them around one camera-relative origin, and emits target-compiled C constants. Captures include Horizon's current block sprite atlas, 16×16 RGBA lightmap, and view parameters. The latest scratch Horizon 26.3 creative capture contains 5,412 sections, 98,236,704 mesh bytes, 21,315,584 sprite bytes, and 1,024 lightmap bytes. The converter selects 17 sections at the default 65,536-vertex budget, using 24 sprite layers packed into a guarded 330×330 RGBA atlas. The GX2 fragment shader bilinearly samples the captured colored lightmap per fragment using interpolated vertex sky/block coordinates; lighting is fixed to the captured frame. Capture builds use the captured yaw, pitch, FOV, and aspect ratio with Horizon's zero-to-one depth projection. The left stick and D-pad move relative to the view; the right stick changes yaw and pitch. Cemu 2.6 loaded the lightmap RPX and ran the title (checksum `0x55b4b4b8`). This verifies conversion, cross-compilation, packaging, and RPX startup, not a Horizon menu, verified terrain frame, or GamePad controls. Translucent meshes remain unsupported.

## Build and emulator results

### Client renderer seam prototype (2026-11-06)

- Added Horizon `PresentationBackend` and `TerrainBackend` contracts, routed game presentation and streamed section lifecycle calls through them, and made the desktop renderer return allocation errors so CPU meshes can be retried. The budgeted uploader now continues past a failed section instead of starving later meshes; the scene replay example reports upload failures.
- This is an initial routing seam, not a selectable GX2 implementation: `Game`, `Loading`, and `Missing` still own concrete `Renderer` values, and the contracts remain inside `horizon-client-render`, whose manifest requires `wgpu` and `winit`. The Horizon `Frame` and its inputs must be made available from a portable layer before a separate backend can consume them.
- Verified on WSL Ubuntu x64: `cargo +nightly check -p horizon-client --no-default-features --jobs 32` passed; `cargo +nightly test -p horizon-client-render --lib presentation::tests --example scene_replay --jobs 32` passed (3 focused tests; the example compiled); `cargo +nightly fmt --all -- --check` passed. These are host checks only, not a Cafe build or Cemu test.
- Known renderer behavior: a failed WGPU remesh can leave the previous GPU section absent until its retained CPU mesh retries successfully; WGPU currently releases the old section before allocating the replacement to reclaim arena space.

### Portable Horizon mesher and platform inputs (2026-11-06)

- Horizon's original CPU terrain mesher, model/asset/biome/texture inputs, `SectionMesh`, side sorting, and compact packing now live in `horizon-terrain-mesh`. `horizon-client-render` re-exports the modules at their existing paths, including `horizon-client-render::mesher`, so upstream call sites continue using the same code. Its `MeshChunk` adapter borrows canonical client chunks; it does not copy or replace chunk storage. There is one production Horizon mesher.
- `horizon-terrain-mesh` has no `wgpu` or `winit` dependency. It still uses Rust `std`, so its dependency graph needs the Cafe `std` toolchain. The opt-in Wii U worldgen bridge now uses the extracted mesher; host results and target verification are recorded below.
- `horizon-client-platform` owns Winit-free event-loop scheduling, portable window/device event data, key/keyboard/mouse identities, and Horizon's existing logger. `horizon-client-input` builds without its default `desktop` feature: shared clipboard shortcuts consume portable key identities and optional text, while Winit conversion, Arboard system clipboard, monitor modes, fullscreen calls, and icons live behind the desktop adapter. The game now dispatches its shared input handlers from platform events, but the outer `ApplicationHandler`, window lifecycle, and client startup are still Winit-bound.
- `horizon-threads` keeps mimalloc enabled by default for desktop and exposes it as an optional feature. `horizon-terrain-mesh --no-default-features` omits mimalloc from its normal dependency graph, allowing an embedded allocator to be supplied later without changing Horizon's CPU mesher.
- WSL Ubuntu host verification: `cargo +nightly test -p horizon-terrain-mesh --lib --jobs 32` passed 135 tests (18 ignored); `cargo +nightly test -p horizon-client-render --test terrain_mesh_view --jobs 32` passed; `cargo +nightly test -p horizon-client-platform -p horizon-client-input --lib --jobs 32` passed 2 platform and 107 input tests; `cargo +nightly check -p horizon-terrain-mesh --no-default-features --jobs 32` passed and its normal dependency tree had no mimalloc, while desktop defaults retained mimalloc; and `cargo +nightly check -p horizon-client --no-default-features --jobs 32` passed. Existing warnings remain in `horizon-client-render` (wgpu recursion-depth and audio unreachable code) and unrelated manifests (missing workspace lint inheritance).
- Renderer and event-loop checks above remain host-only; only the input crate's no-default slice has a Cafe cross-check below. The desktop game still owns a concrete WGPU renderer and Winit window/event loop, so GX2 is not yet a selectable full-client backend.
- The earlier portable-key slice passed 109 input tests and a Cafe target check. The current full event-conversion slice and Cafe target check are recorded in the 2026-11-07 section below.

### Horizon client event ingress and Cafe logging (2026-11-07)

- `horizon-client-platform::events` defines the native-neutral keyboard, mouse, focus, resize, close/redraw, IME, dropped-file, and relative mouse-motion payloads. The desktop adapter converts Winit values once at ingress; Horizon's shared `Game` handlers receive these types. GamePad, control/replay input and other synthetic paths now enter the same dispatch path. This changes API boundaries only; it adds no second gameplay or screen implementation.
- The outer Winit `ApplicationHandler`, window creation, renderer construction, and `run_loading` launch path remain in the desktop game. The event conversion compiles for Cafe, but a Cafe event loop cannot yet start Horizon's original `Loading` and `Title` screens.
- The existing Horizon logger was moved intact from `horizon-client-render` into `horizon-client-platform`; `horizon-client-render::logging` remains a re-export for existing call sites. With the `cafe` feature on PowerPC, Horizon log records additionally call WUT `OSReport` for serial/debug output. Cafe stderr uses the Rust Cafe stdio route, and the existing logger writes its normal `logs/latest.log` plus vanilla-style rolled archives under the supplied game folder. The Wii U diagnostic supplies `/vol/external01/wiiu/apps/HorizonWiiU` as that folder.
- Verification: `cargo +nightly check -p horizon-client --no-default-features --jobs 32` passed in 2m37s; `cargo +nightly test -p horizon-client-platform -p horizon-client-input --lib --jobs 32` passed (120 tests); the `powerpc-nintendo-cafe` check of `horizon-client-platform` and `horizon-client-input` with the Cafe logger feature passed in 39.42s. These checks do not prove Cafe runtime log output. The latest instrumented Cemu process loaded the RPX and logged `Run title`, but its captured console stream was empty and neither `logs/latest.log` nor a fresh worldgen status file appeared. The user's observed screen is white with a loading title; no menu or frame has been verified.

### Horizon runtime rendering diagnostic (2026-11-06 onward)

- `HORIZON_WIIU_WORLDGEN=1` links Horizon's 26.3 registry, terrain assets, biome data, generator and canonical terrain mesher into the WUT/GX2 bootstrap. It generates proto-chunks at runtime, meshes radius-zero or radius-one regions, and streams replacement regions on a Cafe `std::thread`. GX2 handles texture upload, depth testing, quad conversion, and draw submission. This is a renderer bring-up diagnostic; it does not start Horizon's `Game` or implement the client's gameplay/save loop.
- Profile `0` runs Horizon's Overworld generator without structures. Profile `1` uses a deliberately reduced bedrock/dirt/grass flat fixture with features and lakes disabled. The flat profile is for deterministic renderer diagnostics, not a vanilla Superflat world or a client world. Both paths use Horizon generation and block/model asset logic.
- The Wii U camera is diagnostic free-fly only: left stick/D-pad move, right stick looks, and A/B rise or descend. The bridge has no player collision, raycast, block breaking/placement, edit overlay, or save/persistence API. Generated state comes directly from the selected Horizon diagnostic generator. The hand-meshed fallback is available only through the explicit `HORIZON_WIIU_DIAGNOSTIC_FIXTURE=1` build option; it is not the Horizon client.
- The earlier underground start was a bootstrap bug: the standalone viewer hardcoded camera Y and did not run Horizon's player/spawn camera path. The current viewer reads the spawn surface from Horizon's `ProtoChunk::surface_height` and applies a diagnostic-only view margin. This fixes the viewer starting point; it is not a port of Horizon player behavior and says nothing about a running client's spawn logic.
- Earlier host runs produced 60,368 vertices for one structure-free Overworld chunk and 21,504 vertices for a 3×3 reduced-flat region. Post-cleanup probe, run with the complete prepared 26.3 bundle and seed `20261107`, passed: nine generated chunks, 18,432 canonical mesh vertices, valid finite positions/UVs, and a nonempty 2048×8192 RGBA atlas (67,108,864 bytes).
- Cemu 2.6 evidence remains limited. An earlier default-worldgen RPX loaded with checksum `0x9295ade7` and Cemu logged `Run title`; the newer instrumented package loaded with checksum `0x5d341cdb` and also logged `Run title`. The latest process remained alive around 574 MiB, but emitted no captured OSReport/OSConsoleWrite output and created no fresh Horizon `latest.log` or worldgen status file. Earlier terrain-only runs completed base sprite loading and then remained in Horizon's block-model bake for five minutes with Cemu responsive around 599 MiB. No Horizon menu or render frame has been verified. No real-hardware test exists.
- An earlier standalone edit experiment and its `wiiu-edits` sidecar were removed because they duplicated gameplay and persistence outside Horizon's `Game`/save systems. Historical edit logs from that prototype do not represent current functionality or client saves.
- GCC 16 emits a zero-size `.note.GNU-stack` marker that `elf2rpl` rejects; the build script strips that non-runtime marker from a copy of the ELF before packaging. Cemu startup has also failed intermittently with Windows `0xC0000409`; the successful title-startup records are evidence of RPX loading, not visual terrain verification.

The latest default Cafe RPX/WUHB build used WSL Ubuntu. `HORIZON_WIIU_WORLDGEN` is intentionally unset here to verify `build.sh` selects Horizon worldgen by default:

```sh
cd /mnt/c/dev/personal/HorizonWiiU/wiiu/bootstrap
CAFEGLSL_ROOT=/home/ben/HorizonWiiU-toolchain/build-async-default/deps/cafeglsl-1.0.0-rc1-linux-x86_64 \
HORIZON_WIIU_WORLD_PROFILE_SUPERFLAT=1 \
HORIZON_WIIU_WORLD_REGION_RADIUS=1 \
HORIZON_WIIU_STD_RUSTC=/home/ben/HorizonWiiU-toolchain/rust-wiiu-cafe/build/host/stage1/bin/rustc \
HORIZON_WIIU_WORLDGEN_TARGET_DIR=/tmp/horizonwiiu-menu-path/rust-target \
HORIZON_WIIU_BUILD_DIR=/tmp/horizonwiiu-menu-path/build \
  bash build.sh
```

Verified on 2026-11-07 after the portable-event and logger changes. The Cafe `std` worldgen dependency, Rust static library, GX2 bootstrap, RPX, and WUHB all built successfully. The RPX is a 32-bit big-endian PowerPC Cafe OS executable (5.0 MiB); SHA-256 `1733caa4172c95cb8321965e14126ff6bad4aecac1030b298a8f11f45bd3d89f`. The WUHB is 5.0 MiB; SHA-256 `0ed67a1198d3072e105c9dfb190bdf8826ecccf3584c4eba09542b7261b811ba`. This package was not launched in Cemu; the open Cemu process is running the earlier `0x5d341cdb` build, which logged `Run title` but did not produce captured console output, a fresh Horizon log, or a verified frame.

The diagnostic fixture is deliberately opt-in and can be built with `HORIZON_WIIU_WORLDGEN=0 HORIZON_WIIU_DIAGNOSTIC_FIXTURE=1 HORIZON_WIIU_BUILD_DIR=/tmp/horizonwiiu-fixture bash build.sh`. Render-capture builds select `HORIZON_WIIU_CAPTURE_DIR` and default to capture mode automatically. Direct CMake builds require one of worldgen, capture, or the explicit diagnostic fixture mode.

Set `HORIZON_WIIU_WORLD_PROFILE_SUPERFLAT=0 HORIZON_WIIU_WORLD_REGION_RADIUS=0` for a one-chunk Overworld renderer diagnostic without structures. The supported radius is 0 or 1; the default profile is the reduced flat renderer fixture, not a vanilla Superflat client world.

Build the atlas from the player's prepared local resources, writing it into the SD resource root:

```sh
cargo +nightly run --manifest-path wiiu/worldgen/Cargo.toml --example atlas_builder -- \
  .local/horizon-client/26.3 /path/to/resources/26.3/wiiu-terrain-atlas.rgba
```

The focused host probe command was `CARGO_TARGET_DIR=/tmp/horizonwiiu-worldgen-cleanup cargo +nightly run --jobs 32 --manifest-path wiiu/worldgen/Cargo.toml --example world_probe -- /mnt/c/Users/ben/AppData/Local/Temp/HorizonWiiU-Cemu-Worldgen/portable/sdcard/wiiu/apps/HorizonWiiU/resources/26.3 20261107 flat 1`; it passed with nine chunks, 18,432 vertices, a 2048×8192 atlas, and the Horizon-derived spawn surface at Y=-60 (diagnostic camera anchor Y=-56). The Cafe build above produced `C:\Users\ben\AppData\Local\Temp\HorizonWiiU-default-worldgen\build\horizon_wiiu_bootstrap.rpx` and `horizon_wiiu_bootstrap.wuhb`; a later instrumented rebuild loaded in Cemu as checksum `0x5d341cdb` and logged `Run title`. The visible result remains the user's white loading screen; neither Horizon's logger nor a rendered menu/terrain frame has been verified.

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

The Cemu 2.6 Windows executable was restored from its official release asset at that temp path. Its portable `sdcard` directory is linked to the existing worldgen Cemu test data under `%TEMP%\HorizonWiiU-Cemu-Worldgen\portable\sdcard`.

Build directory: `/tmp/horizonwiiu-lightmap-real` for the real Horizon capture and `/tmp/horizonwiiu-lightmap-fixture` for the procedural fixture. Both produced an ELF32 big-endian PowerPC executable, RPX, and WUHB. The portable Cemu 2.6 build was launched with `-g` against the lightmap RPX. Its fresh `portable/log.txt` records `Loaded module 'horizon_wiiu_bootstrap'` (checksum `0x55b4b4b8`) followed by `Run title`. The log reports three RPLLoader section-size warnings (sections 11, 16, and 17), with no launch failure. This verifies RPX loading and title startup, not a screenshot-based visual check or a console run. Cemu GamePad input is not configured. The CafeGLSL auto-download path was separately exercised and verified its published SHA-256. Without `CAFEGLSL_ROOT`, `build.sh` downloads and verifies the compiler automatically.

### Canonical Horizon terrain path (2026-11-07)

- The opt-in `wiiu/worldgen` bridge uses Horizon's original `Assets`, terrain-only asset reload, `RenderAssets`, registry, biome tables, `horizon-world` generator and `horizon-terrain-mesh::Mesher`. Borrowed chunk/section views expose the existing region state and quart-biome arrays. The bridge has one canonical production mesher and no block-edit remeshing or duplicate face/material implementation.
- GX2 queries the generated atlas dimensions and copies rows using the destination pitch. Its one-edge-padded sprite cells and clamped UVs match Horizon's terrain sampling path. Temporary full-sky lighting remains; translucent meshes and emissive rendering are not implemented.
- The terrain-only asset loader lazily resolves blockstate-referenced models through Horizon's resolver, model baker and sprite pipeline. It preserves connected-texture and Fusion data when present, while skipping item-model library loading, standalone/gallery models, LabPBR maps and emissive metadata that this diagnostic renderer cannot draw. Desktop `Assets::reload` remains unchanged. The prepared-cache fast path is gated by the pinned preparation stamp/schema and cache-ready marker; `atlas_builder` invalidates the cache before rebuilding.
- Earlier host checks passed for the canonical mesher, Cafe atomic compatibility and terrain-only assets. The current `world_probe` now verifies runtime generation, canonical mesh vertex validity, atlas contents/dimensions and stage timings only; it no longer tests gameplay or persistence. Its post-cleanup resource-backed probe passed with nine chunks and 18,432 vertices.
- Cemu 2.6 loaded prior Cafe RPXs and logged `Run title`. One full desktop asset-reload build remained in `Assets::reload` for 10m26s at about 581 MiB; the terrain-only path later completed base sprites then stayed in block-model baking for five minutes at about 599 MiB. A full Overworld build also remained in chunk generation after roughly 50 seconds, with a prior radius-one attempt exceeding five minutes. Cemu remained responsive in the block-bake run and exited cleanly. None of these runs reached terrain-ready or visually verified a frame; no hardware run has been made.
- The shared `StateModels` catalog resolves each distinct model identifier once while preserving sorted per-transform bake calls. That optimization removed 5,281 repeated resolutions in the prepared 26.3 model set and passed its equivalence fixture and Cafe target check, but a subsequent Cemu RPX still remained in block-model baking for five minutes.
- This increment removes the standalone block raycast, AABB/player physics, break/place calls, and `wiiu-edits` NBT/region sidecar. The region generator/mesher/GX2 path remains a renderer diagnostic, not a playable Horizon client. Shared `Game` event dispatch now consumes platform-neutral events, while Winit still owns the outer application lifecycle and its renderer remains WGPU.

## Architecture decisions

- Keep Horizon's source and gameplay/data crates identifiable and isolate Wii U code under `wiiu/`.
- Keep Wii U compatibility behind opt-in target features and keep Horizon's desktop paths unchanged. The bootstrap selects Horizon-backed renderer diagnostic profiles: a reduced deterministic flat fixture or an Overworld generator without structures. The full Overworld path is host-verified, and Cemu has not yet completed Horizon's full block-model bake.
- Do not try to run `wgpu` directly on GX2. `gx2gl` is an OpenGL 3.3 translation layer, not a `wgpu` backend; using it would still require a renderer and shader/resource adaptation.
- Keep the CafeOS entry point in WUT C and Horizon systems in Rust. The normal `build.sh` path defaults to Horizon's Cafe-std generator, assets and canonical mesher, adapting region storage through borrowed `MeshChunk` views before sending vertices to GX2. This remains a renderer diagnostic: its free-fly camera has no player collision, interactions, gameplay loop or save path. Horizon's `Game` and save systems remain the intended sources of truth when platform APIs are connected. The non-vanilla hand-built chunk is now explicitly opt-in with `HORIZON_WIIU_DIAGNOSTIC_FIXTURE=1`; it is not the default RPX. The reduced flat Horizon generator profile is also only a deterministic renderer diagnostic, not a vanilla Superflat/client world. The terrain loader omits emissive data because GX2 has no emissive draw pass. Region generation uses Cafe `std::thread`; GPU conversion/upload stays on the render thread. Capture builds use Horizon-captured meshes, camera, atlas and lightmap. No desktop `wgpu` code is linked; translucent meshes remain unsupported. GCC 16 emits a zero-size `.note.GNU-stack` marker that `elf2rpl` rejects, so packaging strips that non-runtime marker from a copy of the ELF first.
- `horizon-terrain-mesh` owns Horizon's original CPU mesher and `SectionMesh`; the desktop renderer preserves upstream paths with re-exports, and the Wii U region adapter borrows its block/biome storage. `horizon-client-platform` owns scheduling, portable events/input, and the original Horizon logger. The shared `Game` event handlers now accept platform events, but startup, Winit's application lifecycle, and `Game`'s concrete WGPU renderer remain coupled. The `PresentationBackend` / `TerrainBackend` API is still a desktop-only forwarding seam; GX2 is not selectable by the full client yet. Keep the bootstrap renderer diagnostic-only.

- A full Cafe `std` route exists upstream on the [`rust-wiiu/rust` `powerpc-nintendo-cafe` branch](https://github.com/rust-wiiu/rust/tree/powerpc-nintendo-cafe), with Cafe implementations for filesystem, threads, time, networking, random, and standard I/O. A prior compiler is already available at `/home/ben/HorizonWiiU-toolchain/rust-wiiu-cafe/build/host/stage1/bin/rustc` (rustc 1.101.0-dev, `powerpc-nintendo-cafe` target and `std` present) and has produced earlier worldgen RPXs. Use it to validate current changes; rebuilding the pinned Rust/LLVM sources is unnecessary unless this compiler fails. An exploratory checkout remains at `E:\temp\horizonwiiu-rust-cafe-sparse` and is not part of the build path.

## Next steps

1. Make the original Horizon startup/loading path diagnosable on Cafe first. Confirm the C WUT entry point reaches Rust initialization and that the moved Horizon logger creates `logs/latest.log` and forwards a line to `OSReport`; the current Cemu run has title-start evidence only, no captured console output or fresh log file.
2. Port the original window/event-loop boundary to CafeOS/WUT so Cafe events call the existing `ClientWindowEvent` dispatch path. Keep Winit as the desktop adapter; do not add a Wii U menu or gameplay clone.
3. Route Horizon's existing loading UI and `Title` screen through a GX2 implementation of the render/presentation contracts. The first client milestone is seeing and controlling Horizon's actual main menu, not the renderer diagnostic screen. This requires the shared UI/frame data to reach GX2 and removing concrete WGPU renderer ownership from the loading and menu path.
4. Build and launch the main-menu RPX in Cemu, capture logs and a screenshot, and distinguish RPX loading from a verified menu frame/input result. Preserve the portable Cemu test environment currently open rather than claiming the white loading screen is success.
5. After the original menu works, continue through Horizon's existing player/world/save systems using the GX2 backend. Keep the worldgen/GX2 diagnostic-only, add Cafe audio through the existing `sound_engine` boundary, and defer networking until single-player works.
