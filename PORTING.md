# HorizonWiiU

Porting Horizon 26.3 to the Nintendo Wii U. Horizon source is kept at the repository root; Wii U entry-point, platform, and renderer work will live under `wiiu/`.

## Status

- Horizon upstream is checked out at `69fab597a6f0e8a6ce524d38d1b73e599e343034` (`main`, 2026-11-06).
- The read-only reference `C:\dev\personal\Codex\MCHorizonUWP` has a real native D3D11 FL9_1 rendering path on its `uwp-phone-d3d11` branch (local commit `43c80ed9b`). It accepts Horizon CPU `Frame`/`SectionMesh` data and implements terrain, entities, translucent passes, hand, HUD, depth, atlases and indexed meshes without routing rendering through `wgpu`. The public remote [`history/uwp-native-mipmaps`](https://github.com/megabytesme/MCHorizonUWP/tree/history/uwp-native-mipmaps) branch (`8a7f0148`) contains the same native-renderer architecture; `uwp-phone-d3d11` itself is local-only. The remote backend notes report x64 UWP readback checks and ARM32 compile/strict-link checks, not a verified phone run. This is the renderer reference to adapt architecturally, not a directly reusable graphics backend.
- Horizon's `horizon-client-render` crate owns the standard concrete `wgpu` renderer. The UWP reference added a `PresentationBackend` / `TerrainBackend` seam and a separate D3D11 renderer that consumes CPU `Frame`/`SectionMesh` data. Reuse that seam design for GX2; implement GX2-specific shaders, buffers, texture atlases, depth state and draw submission rather than calling `wgpu` or copying D3D11 code.
- The old `rust-wiiu/wut` Rust crate was archived on 2026-02-20. The current rust-wiiu organization is experimental, Linux-only, and has moved to `cafe-sys`, `cafe-target-spec`, a Rust fork, and `cargo-wiiu`. Book for U warns that its guide is being rewritten.
- WSL Ubuntu has upstream Rust nightly, `cargo-wiiu` 0.4.1, and the official devkitPro WUT `wiiu-dev` package. The project builds a Rust `no_std` static library, initializes WUT/GX2 for TV and GamePad, and packages RPX/WUHB. Horizon's `horizon-core` has a `wiiu-core` feature that compiles its existing `BlockPos`/`SectionPos`/`ChunkPos` types without `std`; a new `horizon-terrain-data` crate shares the exact 24-byte `TerrainVertex` format between the desktop mesher and Wii U adapter without `std`. Rust generates a projected 3×3, 15-block heightmap fixture with exposed-face culling into four-vertex quads using Horizon's vertex type. A separate adapter expands those quads to GX2 triangle lists and converts fixed-point UVs and packed RGBA color to the GX2 float vertex layout. The current shader ignores sprite layer and light and samples an original 8×8 RGBA atlas with point filtering. GX2 depth testing is enabled for the TV and GamePad contexts. The left stick and D-pad pan this test scene by rebuilding its small vertex buffer only when the camera moves.

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

To launch the RPX from PowerShell with the tested portable Cemu build:

```powershell
& "$env:TEMP\HorizonWiiU-Cemu-v2.6\Cemu_2.6\Cemu.exe" -g "\\wsl.localhost\Ubuntu\tmp\horizonwiiu-section-adapter\horizon_wiiu_bootstrap.rpx"
```

Build directory: `/tmp/horizonwiiu-section-adapter`. It produced `horizon_wiiu_bootstrap.elf` (ELF32, big-endian PowerPC), `horizon_wiiu_bootstrap.rpx`, and `horizon_wiiu_bootstrap.wuhb`. Cemu 2.6 was fetched from the [official Cemu release](https://github.com/cemu-project/Cemu/releases) as a portable build and launched with `-g` against the rebuilt RPX. The initial textured block build was visually inspected and reported about 60 FPS. For the latest quad-adapter RPX, Cemu's fresh `portable/log.txt` records `Loaded module 'horizon_wiiu_bootstrap'` (checksum `0x025f75ae`) followed by `Run title`; the GamePad View window title reported 60.50 FPS. Cemu also reports three RPLLoader section-size warnings (sections 11, 16, and 17), with no launch failure. The final mesh has not had a screenshot-based visual check, and GamePad movement has not been exercised: the portable Cemu profile has no input device assigned. This is not a console test. The CafeGLSL auto-download path was separately exercised and verified its published SHA-256. Without `CAFEGLSL_ROOT`, `build.sh` downloads and verifies the compiler automatically.

## Architecture decisions

- Keep Horizon's source and gameplay/data crates identifiable and isolate Wii U code under `wiiu/`.
- Do not try to run `wgpu` directly on GX2. `gx2gl` is an OpenGL 3.3 translation layer, not a `wgpu` backend; using it would still require a renderer and shader/resource adaptation.
- Keep the CafeOS entry point in WUT C and application logic in Rust. Rust generates a fixed 3×3 BlockPos fixture into four-vertex quads using Horizon's shared `TerrainVertex` type. A separate no_std adapter accepts that same vertex buffer, expands each quad to six GX2 vertices, and converts position, UV and packed colour into the current GX2 layout; C polls VPAD, supplies the bounded pan offsets, uploads through GX2R, binds the procedural RGBA atlas and point sampler, enables depth testing, and draws with an embedded GFD shader. Only exposed top and side faces are emitted. Sprite-array layers, lighting, and translucent meshes are not rendered yet. WUT's `WHBGfxClearColor` clears color and depth for the selected TV/GamePad context ([implementation](https://github.com/devkitPro/wut/blob/master/libraries/libwhb/src/gfx.c)). `horizon-terrain-data` is `no_std` and compiles in both the desktop renderer and Wii U static library; the full `SectionMesh` and meshing pipeline remain in `horizon-client-render`, which still depends on `std` and desktop libraries. The `wiiu-core` feature exposes only the portable position and version modules; the rest of `horizon-core` is gated off because those modules currently need `std`. The build fetches and SHA-256 verifies CafeGLSL v1.0.0-rc1, then compiles the GLSL pair offline; no shader compiler runs on the Wii U. Build uses devkitPro's current `powerpc-eabi-cmake` and WUT libraries, then `cargo-wiiu` to package. GCC 16 emits a zero-size `.note.GNU-stack` marker that `elf2rpl` rejects, so packaging strips that non-runtime marker from a copy of the ELF first.
- Adapt the UWP `PresentationBackend` / `TerrainBackend` pattern to a GX2 backend that consumes Horizon CPU `SectionMesh` and frame data. The full desktop client remains coupled to `wgpu` and desktop window/input/audio dependencies and cannot be cross-compiled unchanged.

## Next steps

1. Feed real `SectionMesh::solid` vertices into the quad adapter, then provide GX2 sprite-layer atlas and lighting support for terrain sections beyond the fixture buffer size.
2. Verify D-pad/stick pan input in Cemu with a configured GamePad device, then add a perspective camera and GamePad movement.
3. Expand the `horizon-core` embedded feature and integrate portable world generation behind Wii U platform interfaces.
4. Run each packaged increment in Cemu and on hardware when available; report Cemu results separately from console results.
