# HorizonWiiU

Porting Horizon 26.3 to the Nintendo Wii U. Horizon source is kept at the repository root; Wii U entry-point, platform, and renderer work will live under `wiiu/`.

## Status

- Horizon upstream is checked out at `69fab597a6f0e8a6ce524d38d1b73e599e343034` (`main`, 2026-11-06).
- The read-only reference `C:\dev\personal\Codex\MCHorizonUWP` has a real native D3D11 FL9_1 rendering path on its `uwp-phone-d3d11` branch (local commit `43c80ed9b`). It accepts Horizon CPU `Frame`/`SectionMesh` data and implements terrain, entities, translucent passes, hand, HUD, depth, atlases and indexed meshes without routing rendering through `wgpu`. The public remote [`history/uwp-native-mipmaps`](https://github.com/megabytesme/MCHorizonUWP/tree/history/uwp-native-mipmaps) branch (`8a7f0148`) contains the same native-renderer architecture; `uwp-phone-d3d11` itself is local-only. The remote backend notes report x64 UWP readback checks and ARM32 compile/strict-link checks, not a verified phone run. This is the renderer reference to adapt architecturally, not a directly reusable graphics backend.
- Horizon's `horizon-client-render` crate owns the standard concrete `wgpu` renderer. The UWP reference added a `PresentationBackend` / `TerrainBackend` seam and a separate D3D11 renderer that consumes CPU `Frame`/`SectionMesh` data. Reuse that seam design for GX2; implement GX2-specific shaders, buffers, texture atlases, depth state and draw submission rather than calling `wgpu` or copying D3D11 code.
- The old `rust-wiiu/wut` Rust crate was archived on 2026-02-20. The current rust-wiiu organization is experimental, Linux-only, and has moved to `cafe-sys`, `cafe-target-spec`, a Rust fork, and `cargo-wiiu`. Book for U warns that its guide is being rewritten.
- WSL Ubuntu has upstream Rust nightly, `cargo-wiiu` 0.4.1, and the official devkitPro WUT `wiiu-dev` package. The project builds a Rust `no_std` static library, initializes WUT/GX2 for TV and GamePad, and packages RPX/WUHB. Horizon's `horizon-core` has a `wiiu-core` feature that compiles its existing `BlockPos`/`SectionPos`/`ChunkPos` types without `std`; Rust supplies a single projected block's position, color and UV vertices, which C uploads to GX2. An original 8×8 RGBA atlas is sampled with point filtering, and GX2 depth testing is enabled for the TV and GamePad contexts.

## Build and emulator results

The current textured/depth build completed from WSL Ubuntu with:

```sh
cd /mnt/c/dev/personal/HorizonWiiU/wiiu/bootstrap
env HORIZON_WIIU_BUILD_DIR=/tmp/horizonwiiu-textured \
  CAFEGLSL_ROOT=/tmp/horizonwiiu-triangle/deps/cafeglsl-1.0.0-rc1-linux-x86_64 \
  bash build.sh
```

To launch the RPX from PowerShell with the tested portable Cemu build:

```powershell
& "$env:TEMP\HorizonWiiU-Cemu-v2.6\Cemu_2.6\Cemu.exe" -g "\\wsl.localhost\Ubuntu\tmp\horizonwiiu-textured\horizon_wiiu_bootstrap.rpx"
```

Clean build directory: `/tmp/horizonwiiu-textured`. It produced `horizon_wiiu_bootstrap.elf` (ELF32, big-endian PowerPC), `horizon_wiiu_bootstrap.rpx`, and `horizon_wiiu_bootstrap.wuhb`. Cemu 2.6 was fetched from the [official Cemu release](https://github.com/cemu-project/Cemu/releases) as a portable build and launched with `-g` against this RPX. The visible Cemu window showed the textured, three-face block and reported about 60 FPS. The emulator screen was inspected live; no screenshot file was saved. This is a successful Cemu runtime check, not a console test. The CafeGLSL auto-download path was separately exercised and verified its published SHA-256. Without `CAFEGLSL_ROOT`, `build.sh` downloads and verifies the compiler automatically.

## Architecture decisions

- Keep Horizon's source and gameplay/data crates identifiable and isolate Wii U code under `wiiu/`.
- Do not try to run `wgpu` directly on GX2. `gx2gl` is an OpenGL 3.3 translation layer, not a `wgpu` backend; using it would still require a renderer and shader/resource adaptation.
- Keep the CafeOS entry point in WUT C and application logic in Rust. Rust supplies per-frame clear color and a fixed single-block vertex mesh based on Horizon's actual `BlockPos`; C uses GX2R to upload the mesh and procedural RGBA atlas, binds a point sampler, enables depth testing, and draws with an embedded GFD shader. WUT's `WHBGfxClearColor` clears color and depth for the selected TV/GamePad context ([implementation](https://github.com/devkitPro/wut/blob/master/libraries/libwhb/src/gfx.c)). The `wiiu-core` feature exposes only the portable position and version modules; the rest of `horizon-core` is gated off because those modules currently need `std`. The build fetches and SHA-256 verifies CafeGLSL v1.0.0-rc1, then compiles the GLSL pair offline; no shader compiler runs on the Wii U. Build uses devkitPro's current `powerpc-eabi-cmake` and WUT libraries, then `cargo-wiiu` to package. GCC 16 emits a zero-size `.note.GNU-stack` marker that `elf2rpl` rejects, so packaging strips that non-runtime marker from a copy of the ELF first.
- Adapt the UWP `PresentationBackend` / `TerrainBackend` pattern to a GX2 backend that consumes Horizon CPU `SectionMesh` and frame data. The full desktop client remains coupled to `wgpu` and desktop window/input/audio dependencies and cannot be cross-compiled unchanged.

## Next steps

1. Use GamePad input to move a camera around multiple blocks, then add texture atlas indexing from block IDs.
2. Adapt Horizon's CPU `SectionMesh` and resource atlas data to the GX2 vertex/texture path; replace the single smoke-test block with real Horizon-produced geometry.
3. Expand the `horizon-core` embedded feature and integrate portable world generation behind Wii U platform interfaces.
4. Run each packaged increment in Cemu and on hardware when available; report Cemu results separately from console results.
