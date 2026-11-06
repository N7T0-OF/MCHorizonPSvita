# HorizonWiiU

Porting Horizon 26.3 to the Nintendo Wii U. Horizon source is kept at the repository root; Wii U entry-point, platform, and renderer work will live under `wiiu/`.

## Status

- Horizon upstream is checked out at `69fab597a6f0e8a6ce524d38d1b73e599e343034` (`main`, 2026-11-06).
- The read-only reference `C:\dev\personal\Codex\MCHorizonUWP` has a real native D3D11 FL9_1 rendering path on its `uwp-phone-d3d11` branch (local commit `43c80ed9b`). It accepts Horizon CPU `Frame`/`SectionMesh` data and implements terrain, entities, translucent passes, hand, HUD, depth, atlases and indexed meshes without routing rendering through `wgpu`. The public remote [`history/uwp-native-mipmaps`](https://github.com/megabytesme/MCHorizonUWP/tree/history/uwp-native-mipmaps) branch (`8a7f0148`) contains the same native-renderer architecture; `uwp-phone-d3d11` itself is local-only. The remote backend notes report x64 UWP readback checks and ARM32 compile/strict-link checks, not a verified phone run. This is the renderer reference to adapt architecturally, not a directly reusable graphics backend.
- Horizon's `horizon-client-render` crate owns the standard concrete `wgpu` renderer. The UWP reference added a `PresentationBackend` / `TerrainBackend` seam and a separate D3D11 renderer that consumes CPU `Frame`/`SectionMesh` data. Reuse that seam design for GX2; implement GX2-specific shaders, buffers, texture atlases, depth state and draw submission rather than calling `wgpu` or copying D3D11 code.
- The old `rust-wiiu/wut` Rust crate was archived on 2026-02-20. The current rust-wiiu organization is experimental, Linux-only, and has moved to `cafe-sys`, `cafe-target-spec`, a Rust fork, and `cargo-wiiu`. Book for U warns that its guide is being rewritten.
- WSL Ubuntu has upstream Rust nightly, `cargo-wiiu` 0.4.1, and the official devkitPro WUT `wiiu-dev` package. The project builds a Rust `no_std` static library, initializes WUT/GX2 for TV and GamePad, compiles a GLSL vertex/color shader pair with CafeGLSL, embeds the resulting GFD shader, draws a colored triangle, and packages RPX/WUHB. Cemu is not installed or discoverable.

## Build and emulator results

Project bootstrap built successfully from WSL Ubuntu with:

```sh
cd /mnt/c/dev/personal/HorizonWiiU/wiiu/bootstrap
env HORIZON_WIIU_BUILD_DIR=/tmp/horizonwiiu-final \
  CAFEGLSL_ROOT=/tmp/horizonwiiu-triangle/deps/cafeglsl-1.0.0-rc1-linux-x86_64 \
  bash build.sh
```

Clean build directory: `/tmp/horizonwiiu-final`. It produced `horizon_wiiu_bootstrap.elf` (ELF32, big-endian PowerPC), `horizon_wiiu_bootstrap.rpx` (Cafe OS ELF), and `horizon_wiiu_bootstrap.wuhb`. `readelf` confirmed the linked Rust frame-color function, embedded GFD shader, and WUT imports for coreinit, GX2 and ProcUI. The auto-download path was exercised in `/tmp/horizonwiiu-triangle`; it downloaded CafeGLSL v1.0.0-rc1 and verified its published SHA-256. This verifies compilation and packaging only; the triangle has not been visually tested because Cemu is not installed or discoverable. Without `CAFEGLSL_ROOT`, `build.sh` downloads and verifies the compiler automatically.

## Architecture decisions

- Keep Horizon's source and gameplay/data crates identifiable and isolate Wii U code under `wiiu/`.
- Do not try to run `wgpu` directly on GX2. `gx2gl` is an OpenGL 3.3 translation layer, not a `wgpu` backend; using it would still require a renderer and shader/resource adaptation.
- Keep the CafeOS entry point in WUT C and gameplay/application logic in Rust. Rust currently supplies a per-frame clear color; C calls WUT's GX2 API to clear both displays and draw a colored triangle from an embedded GFD shader and GX2R vertex buffer. Host build fetches and SHA-256 verifies CafeGLSL v1.0.0-rc1, then compiles the GLSL pair offline; no shader compiler runs on the Wii U. Build uses devkitPro's current `powerpc-eabi-cmake` and WUT libraries, then `cargo-wiiu` to package. GCC 16 emits a zero-size `.note.GNU-stack` marker that both `elf2rpl` and `cargo-wiiu` 0.4.1 reject, so packaging strips that non-runtime marker from a copy of the ELF first.
- Adapt the UWP `PresentationBackend` / `TerrainBackend` pattern to a GX2 backend that consumes Horizon CPU `SectionMesh` and frame data. The full desktop client remains coupled to `wgpu` and desktop window/input/audio dependencies and cannot be cross-compiled unchanged.

## Next steps

1. Visually verify the triangle in Cemu or on a console, then add texture sampling, depth state, and camera movement.
2. Add a GX2 terrain backend that consumes Horizon `SectionMesh` data and resource atlas data.
3. Integrate portable Horizon world generation and input behind Wii U platform interfaces.
4. Run each packaged increment in Cemu when available; until then report only build/package verification, not runtime behavior.
