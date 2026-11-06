# HorizonWiiU

Porting Horizon 26.3 to the Nintendo Wii U. Horizon source is kept at the repository root; Wii U entry-point, platform, and renderer work will live under `wiiu/`.

## Status

- Horizon upstream is checked out at `69fab597a6f0e8a6ce524d38d1b73e599e343034` (`main`, 2026-11-06).
- The read-only reference `C:\dev\personal\Codex\MCHorizonUWP` has a real native D3D11 FL9_1 rendering path on its `uwp-phone-d3d11` branch (local commit `43c80ed9b`). It accepts Horizon CPU `Frame`/`SectionMesh` data and implements terrain, entities, translucent passes, hand, HUD, depth, atlases and indexed meshes without routing rendering through `wgpu`. The public remote [`history/uwp-native-mipmaps`](https://github.com/megabytesme/MCHorizonUWP/tree/history/uwp-native-mipmaps) branch (`8a7f0148`) contains the same native-renderer architecture; `uwp-phone-d3d11` itself is local-only. The remote backend notes report x64 UWP readback checks and ARM32 compile/strict-link checks, not a verified phone run. This is the renderer reference to adapt architecturally, not a directly reusable graphics backend.
- Horizon's `horizon-client-render` crate owns the standard concrete `wgpu` renderer. The UWP reference added a `PresentationBackend` / `TerrainBackend` seam and a separate D3D11 renderer that consumes CPU `Frame`/`SectionMesh` data. Reuse that seam design for GX2; implement GX2-specific shaders, buffers, texture atlases, depth state and draw submission rather than calling `wgpu` or copying D3D11 code.
- The old `rust-wiiu/wut` Rust crate was archived on 2026-02-20. The current rust-wiiu organization is experimental, Linux-only, and has moved to `cafe-sys`, `cafe-target-spec`, a Rust fork, and `cargo-wiiu`. Book for U warns that its guide is being rewritten.
- WSL Ubuntu has upstream Rust nightly, `cargo-wiiu` 0.4.1, and the official devkitPro WUT `wiiu-dev` package. The project builds a Rust `no_std` static library, initializes WUT/GX2 for TV and GamePad, and packages RPX/WUHB. Horizon's `horizon-core` has a `wiiu-core` feature that compiles its existing `BlockPos`/`SectionPos`/`ChunkPos` types without `std`; a new `horizon-terrain-data` crate shares the exact 24-byte `TerrainVertex` format between the desktop mesher and Wii U adapter without `std`. Rust generates a projected 3×3, 15-block heightmap fixture with exposed-face culling into four-vertex quads using Horizon's vertex type. A separate adapter expands those quads to GX2 triangle lists and converts fixed-point UVs and packed RGBA color to the GX2 float vertex layout. The procedural fixture samples an 8×8 RGBA atlas; capture builds now sample a compact atlas of the selected Horizon section's real sprite layers and apply the packed light approximation described below. GX2 depth testing is enabled for the TV and GamePad contexts. The left stick and D-pad pan the fixture scene by rebuilding its small vertex buffer only when the camera moves.
- The adapter consumes actual `SectionMesh::solid` vertices from Horizon's render capture. `tools/horizon_wiiu_capture.py` selects the nearest nonempty solid section, decodes the host-endian 24-byte records, and emits target-compiled C constants with a camera-relative section origin. Captures now include Horizon's current block sprite atlas. A fresh Horizon 26.3 creative world produced 1,176 resident sections, 30,524,256 mesh bytes, and 21,315,584 sprite bytes. The converter selected section `(-2, 9, 0)` with 3,900 solid vertices using three sprite layers, packed into a guarded 132×132 RGBA atlas. The capture-fed RPX built successfully and Cemu 2.6 loaded its module and ran the title (latest checksum `0x9d10c1ed`). This verifies real Horizon mesh and texture conversion, cross-compilation, packaging, and RPX startup; it does not verify visual fidelity on hardware. The bootstrap honors cutout alpha and applies a grayscale max-channel approximation of packed sky/block light, but does not reproduce Horizon's time-dependent colored lightmap. Translucent meshes and captured yaw/pitch remain unsupported.

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

The Wii U no_std terrain adapter tests passed (3/3) after adding captured-light brightness:

```sh
CARGO_TARGET_DIR=/tmp/horizonwiiu-horizon-tests cargo +nightly test \
  --manifest-path /mnt/c/dev/personal/HorizonWiiU/wiiu/bootstrap/Cargo.toml \
  --lib --jobs 32
```

To build with an existing Horizon render capture (see [docs/graphics.md](docs/graphics.md), “Render capture and replay”):

```sh
HORIZON_WIIU_CAPTURE_DIR=/path/to/capture \
HORIZON_WIIU_BUILD_DIR=/tmp/horizonwiiu-captured-section \
  bash build.sh
```

The verified real-capture build used WSL Ubuntu and the capture from a scratch Horizon world:

```sh
HORIZON_WIIU_CAPTURE_DIR=/tmp/horizonwiiu-horizon-capture \
HORIZON_WIIU_BUILD_DIR=/tmp/horizonwiiu-real-textures \
CAFEGLSL_ROOT=/tmp/horizonwiiu-triangle/deps/cafeglsl-1.0.0-rc1-linux-x86_64 \
  bash build.sh
```

To make another capture, run Horizon with isolated data directories and the prepared 26.3 resources, then use `horizon-ctl render-capture on` followed by `horizon-ctl render-capture DIR`. WSLg needs its normal `XDG_RUNTIME_DIR` for the Wayland socket; keep the game, launcher, and XDG data/config directories in scratch locations. `docs/graphics.md` describes the capture contents.

To launch the RPX from PowerShell with the tested portable Cemu build:

```powershell
& "$env:TEMP\HorizonWiiU-Cemu-v2.6\Cemu_2.6\Cemu.exe" -g "\\wsl.localhost\Ubuntu\tmp\horizonwiiu-real-textures\horizon_wiiu_bootstrap.rpx"
```

Build directory: `/tmp/horizonwiiu-real-textures` for the real Horizon capture and `/tmp/horizonwiiu-procedural-after-textures` for the procedural fixture. Both produced an ELF32 big-endian PowerPC executable, RPX, and WUHB after the texture and vertex-layout changes. The portable Cemu 2.6 build was launched with `-g` against the real-capture RPX. Its fresh `portable/log.txt` records `Loaded module 'horizon_wiiu_bootstrap'` (checksum `0x9d10c1ed`) followed by `Run title`; the Cemu process remained live after launch. The log reports three RPLLoader section-size warnings (sections 11, 16, and 17), with no launch failure. This verifies RPX loading and title startup, not a screenshot-based visual check or a console run. Cemu GamePad input is not configured. The CafeGLSL auto-download path was separately exercised and verified its published SHA-256. Without `CAFEGLSL_ROOT`, `build.sh` downloads and verifies the compiler automatically.

## Architecture decisions

- Keep Horizon's source and gameplay/data crates identifiable and isolate Wii U code under `wiiu/`.
- Do not try to run `wgpu` directly on GX2. `gx2gl` is an OpenGL 3.3 translation layer, not a `wgpu` backend; using it would still require a renderer and shader/resource adaptation.
- Keep the CafeOS entry point in WUT C and application logic in Rust. Rust generates a fixed 3×3 BlockPos fixture into four-vertex quads using Horizon's shared `TerrainVertex` type. A separate no_std adapter accepts that same vertex buffer, expands each quad to six GX2 vertices, converts position and UV, and multiplies packed vertex color by the brighter packed sky/block light coordinate with a small ambient floor. This is a grayscale approximation, not Horizon's time-dependent colored lightmap. C polls VPAD, supplies bounded pan offsets, uploads through GX2R, binds a point-sampled atlas and enables depth testing. The fixture uses a procedural RGBA atlas; capture builds compact the selected mesh's referenced Horizon sprite layers into a guarded 2D atlas and use the vertex force-opaque flag for alpha cutout. With `HORIZON_WIIU_CAPTURE_DIR`, C passes one host-converted Horizon `SectionMesh::solid` section and its camera-relative origin to the same adapter. The converter emits C numeric constants so the Wii U compiler lays values out in its big-endian ABI; no desktop `wgpu` code is linked. The bootstrap still draws only solid meshes; translucent meshes and the captured camera's yaw/pitch are not implemented. WUT's `WHBGfxClearColor` clears color and depth for the selected TV/GamePad context ([implementation](https://github.com/devkitPro/wut/blob/master/libraries/libwhb/src/gfx.c)). `horizon-terrain-data` is `no_std` and compiles in both the desktop renderer and Wii U static library; the full `SectionMesh` and meshing pipeline remain in `horizon-client-render`, which still depends on `std` and desktop libraries. The `wiiu-core` feature exposes only the portable position and version modules; the rest of `horizon-core` is gated off because those modules currently need `std`. The build fetches and SHA-256 verifies CafeGLSL v1.0.0-rc1, then compiles the GLSL pair offline; no shader compiler runs on the Wii U. Build uses devkitPro's current `powerpc-eabi-cmake` and WUT libraries, then `cargo-wiiu` to package. GCC 16 emits a zero-size `.note.GNU-stack` marker that `elf2rpl` rejects, so packaging strips that non-runtime marker from a copy of the ELF first.
- Adapt the UWP `PresentationBackend` / `TerrainBackend` pattern to a GX2 backend that consumes Horizon CPU `SectionMesh` and frame data. The full desktop client remains coupled to `wgpu` and desktop window/input/audio dependencies and cannot be cross-compiled unchanged.

## Next steps

1. Transfer Horizon's time-dependent colored lightmap, then preserve and render translucent section meshes.
2. Add captured yaw/pitch and perspective camera movement, then verify D-pad/stick input in Cemu.
3. Expand the `horizon-core` embedded feature and integrate portable world generation behind Wii U platform interfaces.
4. Run each packaged increment in Cemu and on hardware when available; report Cemu results separately from console results.
