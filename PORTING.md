# HorizonWiiU

Porting Horizon 26.3 to the Nintendo Wii U. Horizon source is kept at the repository root; Wii U entry-point, platform, and renderer work will live under `wiiu/`.

## Status

- Horizon upstream is checked out at `69fab597a6f0e8a6ce524d38d1b73e599e343034` (`main`, 2026-11-06).
- The read-only reference `C:\dev\personal\Codex\MCHorizonUWP` has a real native D3D11 FL9_1 rendering path on its `uwp-phone-d3d11` branch (local commit `43c80ed9b`). It accepts Horizon CPU `Frame`/`SectionMesh` data and implements terrain, entities, translucent passes, hand, HUD, depth, atlases and indexed meshes without routing rendering through `wgpu`. The public remote [`history/uwp-native-mipmaps`](https://github.com/megabytesme/MCHorizonUWP/tree/history/uwp-native-mipmaps) branch (`8a7f0148`) contains the same native-renderer architecture; `uwp-phone-d3d11` itself is local-only. The remote backend notes report x64 UWP readback checks and ARM32 compile/strict-link checks, not a verified phone run. This is the renderer reference to adapt architecturally, not a directly reusable graphics backend.
- Horizon's `horizon-client-render` crate owns the standard concrete `wgpu` renderer. The UWP reference added a `PresentationBackend` / `TerrainBackend` seam and a separate D3D11 renderer that consumes CPU `Frame`/`SectionMesh` data. Reuse that seam design for GX2; implement GX2-specific shaders, buffers, texture atlases, depth state and draw submission rather than calling `wgpu` or copying D3D11 code.
- The old `rust-wiiu/wut` Rust crate was archived on 2026-02-20. The current rust-wiiu organization is experimental, Linux-only, and has moved to `cafe-sys`, `cafe-target-spec`, a Rust fork, and `cargo-wiiu`. Book for U warns that its guide is being rewritten.
- WSL Ubuntu has upstream Rust nightly, `cargo-wiiu` 0.4.1, and the official devkitPro WUT `wiiu-dev` package. The project builds a Rust `no_std` static library, initializes WUT/GX2 for TV and GamePad, and packages RPX/WUHB. Horizon's `horizon-core` `wiiu-core` feature now exposes `BlockPos`/`SectionPos`/`ChunkPos`, its Java-compatible random streams, and its production Perlin noise sampler without `std`; `alloc`-based noise APIs link through a WUT/newlib `malloc`/`free` allocator. A new `horizon-terrain-data` crate shares the exact 24-byte `TerrainVertex` format between desktop and Wii U code. The default no-capture path still has a small bounded development chunk using Horizon RNG/noise primitives. The opt-in Cafe-`std` worldgen path below loads Horizon's actual registry and `horizon-world` generator, meshes its runtime proto-chunks into the same `TerrainVertex` format, and passes them to GX2. Capture builds consume real captured `SectionMesh::solid` data and the captured sprite atlas/lightmap. The GX2 adapter expands quads to triangle lists and converts fixed-point UVs, light coordinates, and packed RGBA to GX2 floats. Depth testing is enabled for TV and GamePad contexts; translucent meshes remain unsupported.
- The adapter consumes actual `SectionMesh::solid` vertices from Horizon's render capture. `tools/horizon_wiiu_capture.py` ranks nonempty sections by camera distance, decodes their host-endian 24-byte records, combines the nearest sections within a vertex budget, rebases them around one camera-relative origin, and emits target-compiled C constants. Captures include Horizon's current block sprite atlas, 16×16 RGBA lightmap, and view parameters. The latest scratch Horizon 26.3 creative capture contains 5,412 sections, 98,236,704 mesh bytes, 21,315,584 sprite bytes, and 1,024 lightmap bytes. The converter selects 17 sections at the default 65,536-vertex budget, using 24 sprite layers packed into a guarded 330×330 RGBA atlas. The GX2 fragment shader bilinearly samples the captured colored lightmap per fragment using interpolated vertex sky/block coordinates; lighting is fixed to the captured frame. Capture builds use the captured yaw, pitch, FOV, and aspect ratio with Horizon's zero-to-one depth projection. The left stick and D-pad move relative to the view; the right stick changes yaw and pitch. Cemu 2.6 loaded the lightmap RPX and ran the title (latest checksum `0x55b4b4b8`). This verifies real Horizon mesh and texture conversion, cross-compilation, packaging, and RPX startup; it does not verify visual fidelity on hardware or GamePad controls in Cemu. Translucent meshes remain unsupported.

## Build and emulator results

### Horizon runtime worldgen update (2026-11-06)

- The Cafe-`std` bridge lives under `wiiu/worldgen` and links Horizon's actual `horizon-world` generator into the existing WUT/GX2 executable. The FFI accepts profile `0` for full Overworld generation or `1` for Horizon's `OverworldGenerator::flat`; the bootstrap selects profile `1` for a fast runtime demo.
- Host probe results with the prepared 26.3 resources and seed `12345`: Overworld spawn `(103,-28)`, chunk `(6,-2)`, 60,368 mesh vertices, surface Y `128..136`; superflat same spawn/chunk, 3,072 mesh vertices, surface Y `-60`.
- The GX2 bootstrap generates a radius-one, 3×3 superflat region at runtime, combines the nine Horizon meshes, and uploads the result. When the camera nears an outer edge, the bridge generates a replacement region on Cafe `std::thread`, keeps rendering the current region while it runs, then installs the returned mesh and height map on the render thread. The current whole-region mesh culls internal chunk faces; host probe: `profile=flat`, seed `12345`, center chunk `(6,-2)`, region origin `(5,-3)`, 21,504 vertices, surface Y `-60..-60`.
- The 3×3 region retains its complete 48×48 surface-height map; C updates camera eye height from the generated column after horizontal movement. Host probes report 21,504 vertices for flat and 60,368 for one full Overworld chunk.
- The bridge now keeps one `GeneratorWorker` across the region instead of constructing a fresh worker for each chunk, preserving Horizon's per-worker caches and cumulative stage timings. Host debug probe timings for one full chunk were biomes 83 ms, noise 115 ms, surface 32 ms and carvers 41 ms; for nine chunks, 176/906/273/56 ms respectively. These are host debug-profile measurements, not Wii U performance results.
- `atlas_builder` converts eight textures from the user's prepared 26.3 resource tree into a 64×32 RGBA atlas at `wiiu-terrain-atlas.rgba`; only that derived raw atlas is needed on the Wii U SD card. The worldgen bridge loads it from the prepared resource root and maps grass, stone, dirt, sand, gravel, logs and planks to the atlas. If the file is absent, GX2 uses the generated placeholder atlas.
- The earlier production worldgen Cafe build in `/home/ben/HorizonWiiU-toolchain/build-worldgen-async` completed its initial region in Cemu 2.6 (module checksum `0x6a55de13`): center `(6,-2)`, nine chunks, 21,504 vertices, surface Y `-60`, `atlas=local`.
- Block edits now persist through Horizon's existing `ChunkRecord`, NBT, Zlib, and `RegionFile` APIs. The Wii U adapter stores per-chunk edit records under `saves/world-<seed>-<profile>/wiiu-edits/region/r.X.Z.mca`, separate from vanilla chunk regions. Host tests cover deterministic NBT, world identity/state validation, negative region coordinates, region rewrites, and reload. This is a transition layer over the current in-memory edit overlay, not Horizon's complete `SaveDir` world persistence.
- The Cafe `std` save-adapter build produced RPX/WUHB in `/home/ben/HorizonWiiU-toolchain/build-worldgen-save`; `horizon-wiiu-worldgen` host tests pass 6/6. RPX SHA-256: `fb610a32f4efa12653aa58873814bb16619f8fd959d7dad57229d8240edb61cd`; WUHB SHA-256: `d103e94bb04c34e129fcae6e13c64f8aec1237896b8da5ea52c3f305317721d4`. Cemu loaded the updated module (checksum `0x1ce27970`) and logged `Run title`, then Windows Error Reporting recorded Cemu 2.6 fail-fast `0xC0000409` at offset `0xFCA125`. An earlier 65-second launch of the preceding build stayed alive but stopped at `registry material map loaded`, so neither run verified the region save call. No hardware test was made.
- `wiiu/cemu/controller0.xml` maps WASD to movement, IJKL to look, Space to jump, keyboard X to GamePad X (break), and keyboard Z to GamePad Y (place). In a Cemu 2.6 input run, the status file recorded `block edited: 103,-61,-27 0->10`, confirming a dirt placement reached the live Horizon region. The host interaction probe separately verifies ray targeting, breaking, replacement, collision, mesh/height updates, and edit survival across region regeneration. Persistent region writes now have host coverage, but the new Cafe save path has not yet been exercised by an in-game edit. There is still no screenshot-based terrain verification or real-hardware input test.
- An earlier Cemu 2.6 run, before voxel AABB collision was added, held keyboard A (left stick left) and streamed from region center `(6,-2)` to `(7,-2)`.
- After voxel collision was added, a 14-second A hold did not reach the region-edge threshold. A 25-second hold then streamed into the next region; placing afterward recorded `block edited: 141,-60,-27 0->10`, well beyond the starting region. Cemu remained alive, verifying movement through the AABB checks, runtime region streaming, and placement after movement. The resulting terrain has not been screenshot-verified.
- `WorldChunk` now retains the actual Horizon state IDs for its full vertical range, the state collision/material maps, and the regenerated mesh. The 3×3 region mesh is built as a whole, culling faces between neighboring chunks; its flat-world count is now 21,504 vertices instead of 27,648. A five-block voxel DDA ray selects targets. X breaks an opaque block; Z places the registry's dirt state. Edits are reapplied to streamed regions and persisted through Horizon's low-level region/NBT APIs. They are stored separately from vanilla chunks and are not yet a normal Horizon/vanilla world save. The player uses a 0.6×1.8 full-cube AABB against opaque voxels for side and ceiling collision, with the existing surface-height map for grounding and one-block steps. Slabs, stairs, fluids, non-cube shapes, and inventory selection remain unsupported.
- Host validation with the complete prepared 26.3 bundle from the Cemu SD test tree: `world_probe ... 12345 flat 1` passed live collision, raycast, break, place, and region-reload assertions. `cargo +nightly test --manifest-path wiiu/worldgen/Cargo.toml --lib` passed 5/5, and the bootstrap `runtime-demo` tests passed 11/11. The latest worldgen Cafe `std` RPX/WUHB and no-worldgen WUT RPX/WUHB both rebuilt successfully. GCC still emits its known `.note.GNU-stack` packaging warning.
- Earlier Cemu 2.6 relaunches of both current and baseline RPXs failed after `Run title` with Windows `0xC0000409` at offset `0xFCA125`; one historical 38-second run of the worldgen build completed region generation and accepted GamePad-key input. The save-adapter RPX reproduced the same Cemu fail-fast. Cemu startup remains intermittent; no screenshot or hardware run has been made.
- A separate Cemu smoke build temporarily forced the camera to two region edges. It stayed alive for 26 seconds and the status moved from region center `(6,-2)` through `(7,-2)` to `(8,-2)`, exercising the Cafe worker, result polling, and successive region installation. The forced-camera hook was removed from source afterward. This does not verify actual GamePad movement or visual terrain, and no screenshot was captured.
- The standard no-worldgen Cafe bootstrap also builds successfully to `/home/ben/HorizonWiiU-toolchain/build-async-default` with the asynchronous-region C changes excluded.
- Region generation no longer runs on the render thread, but vertex conversion/upload still happens there when a region completes. No menu or full gameplay loop exists yet, and there is no real-hardware test.
- A full Overworld Cafe build with profile `0` and radius `0` loaded in Cemu, but after about 50 seconds its status still read `chunk generation started: 6,-2`; no proto-chunk returned. The responsive Cemu process was stopped. An earlier radius-one full Overworld run also exceeded five minutes. This points to a large Cafe/Cemu execution-cost gap; host timings do not yet identify its target-side cause. No menu or full gameplay loop exists yet, and there is no real-hardware test.

The latest Horizon-worldgen build used WSL Ubuntu:

```sh
cd /mnt/c/dev/personal/HorizonWiiU/wiiu/bootstrap
CAFEGLSL_ROOT=/home/ben/HorizonWiiU-toolchain/build-worldgen/deps/cafeglsl-1.0.0-rc1-linux-x86_64 \
HORIZON_WIIU_WORLDGEN=1 \
HORIZON_WIIU_WORLD_PROFILE_SUPERFLAT=1 \
HORIZON_WIIU_WORLD_REGION_RADIUS=1 \
HORIZON_WIIU_STD_RUSTC=/home/ben/HorizonWiiU-toolchain/rust-wiiu-cafe/build/host/stage1/bin/rustc \
HORIZON_WIIU_WORLDGEN_TARGET_DIR=/home/ben/HorizonWiiU-toolchain/worldgen-cafe-save \
HORIZON_WIIU_BUILD_DIR=/home/ben/HorizonWiiU-toolchain/build-worldgen-save \
  bash build.sh
```

The matching no-worldgen bootstrap also builds with `HORIZON_WIIU_BUILD_DIR=/home/ben/HorizonWiiU-toolchain/build-async-default bash build.sh`.

Set `HORIZON_WIIU_WORLD_PROFILE_SUPERFLAT=0 HORIZON_WIIU_WORLD_REGION_RADIUS=0` for a one-chunk full Overworld diagnostic build. The supported radius is 0 or 1; the live default remains the faster radius-one superflat region.

Build the atlas from the player's prepared local resources, writing it into the SD resource root:

```sh
cargo +nightly run --manifest-path wiiu/worldgen/Cargo.toml --example atlas_builder -- \
  .local/horizon-client/26.3 /path/to/resources/26.3/wiiu-terrain-atlas.rgba
```

The focused host region probe is `CARGO_TARGET_DIR=/home/ben/HorizonWiiU-toolchain/worldgen-host-inline cargo +nightly run --manifest-path wiiu/worldgen/Cargo.toml --example world_probe -- /path/to/resources/26.3 12345 flat 1` (`overworld` selects full terrain; radius `0` generates one chunk). With the generated atlas, it reported `atlas=local`; the reachable-rule behavior test passed with `cargo +nightly test -p horizon-world worldgen::material_condition::library::tests::forward_rule_holders_and_invalid_graphs_are_atomic -- --exact`.

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

## Architecture decisions

- Keep Horizon's source and gameplay/data crates identifiable and isolate Wii U code under `wiiu/`.
- Keep Wii U compatibility and loading optimizations behind the opt-in `cafe-terrain-only` feature. The bootstrap uses Horizon's real superflat generator for the live Cemu path. The full Overworld path is host-verified but currently does not finish in the tested Cemu run.
- Do not try to run `wgpu` directly on GX2. `gx2gl` is an OpenGL 3.3 translation layer, not a `wgpu` backend; using it would still require a renderer and shader/resource adaptation.
- Keep the CafeOS entry point in WUT C and application logic in Rust. The default no-capture path builds one seeded 16×16×32 runtime chunk using Horizon's RNG and Perlin primitives; the opt-in Cafe-`std` path loads Horizon's real registry and generator. The live profile retains all state IDs for a 48×48×384 region (about 3.4 MiB of `u32` block data), remeshes edits across chunk boundaries, and keeps a session edit map for region reloads. The player ray and AABB queries read this retained region directly; block edits also update the GX2 source mesh and ground-height map. The Wii U adapter uses WUT/newlib `malloc` and `free` for `alloc`, expands quads to GX2 triangle lists, converts vertex attributes, and uploads through GX2R. Region generation runs on a Cafe `std::thread` while the current mesh remains visible; GPU conversion/upload still happens on the render thread. Camera eye height follows the generated surface at 1.62 blocks, with jump/gravity, one-block stepping, and opaque full-cube collision. Capture builds use Horizon's camera and draw real captured meshes with the captured atlas/lightmap. No desktop `wgpu` code is linked; translucent meshes remain unsupported. The full `SectionMesh` and asset-driven client mesher remain in `horizon-client-render`, which depends on `std` and desktop libraries. GCC 16 emits a zero-size `.note.GNU-stack` marker that `elf2rpl` rejects, so packaging strips that non-runtime marker from a copy of the ELF first.
- Adapt the UWP `PresentationBackend` / `TerrainBackend` pattern to a GX2 backend that consumes Horizon CPU `SectionMesh` and frame data. The full desktop client remains coupled to `wgpu` and desktop window/input/audio dependencies and cannot be cross-compiled unchanged.

- A full Cafe `std` route exists upstream on the [`rust-wiiu/rust` `powerpc-nintendo-cafe` branch](https://github.com/rust-wiiu/rust/tree/powerpc-nintendo-cafe), with Cafe implementations for filesystem, threads, time, networking, random, and standard I/O. It is not installed or built here. The documented stage-1 compiler/library build needs roughly 10–15 GB of free build storage. C: is nearly full, but Ben-Desktop's E: volume has 61 GB free and is mounted in WSL at `/mnt/e`; the next step is to build the fork there and validate `horizon-world` under Cafe `std`.

## Next steps

1. Capture and inspect an actual Cemu GamePad View frame to verify the terrain visually; runtime status and title liveness do not prove the rendered image is correct.
2. Move the bridge toward Horizon's existing client runtime and `SaveDir`: those systems currently live inside `horizon-client`, whose mandatory `wgpu`/`winit` dependencies prevent using its game loop and save lifecycle in the Wii U build. Isolate platform/render APIs so the Wii U supplies CafeOS/WUT/GX2 implementations rather than growing parallel gameplay or persistence systems. Treat the current bridge controls and edit overlay as bootstrap code, not replacement client systems.
3. Instrument Cafe-side Overworld generation stages to explain why the real Overworld still stalls in Cemu despite much lower host timings.
4. Use Horizon's existing block-shape collision and inventory/item APIs when the client-runtime boundary is available; avoid expanding the current cube-only controls as a separate game implementation.
