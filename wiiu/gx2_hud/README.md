# GX2 HUD presenter

This standalone backend consumes Horizon `HudVertex` values, `HudBatch::runs()` ranges,
RGBA atlas pixels, font patches, the physical viewport, and `HudBatch::scale`. Its caller
must initialize GX2 before creating the presenter, call it on the render thread, set the
GX2 color target, and provide a GFD from `hud.vert`/`hud.frag`. Keep the GFD
bytes alive until the presenter is destroyed. Atlas input is copied; patches and frame
data are borrowed only for the duration of a draw. The draw changes GX2 render state.
The Rust bridge in `wiiu/platform/src/hud.rs` accepts Horizon's original `Blend`
and font `Patch` values. It maps supported blends explicitly because Rust's
`Blend` enum and its `Depth`/`CitGlint` payloads have no C ABI.

Only alpha runs and the native GUI shader are implemented. Invert, additive, depth,
vignette, end portal, CIT glint, vanilla glint UVs and resource-pack text shaders return
`HORIZON_HUD_UNSUPPORTED` before modifying GPU resources. Invalid ranges, missing data,
capacity overflow and out-of-bounds patches return `HORIZON_HUD_INVALID`. A caller must
report or handle these results; it must not silently claim an unsupported frame rendered.
The backend is not connected to Horizon's client or startup menu yet.
Paired PowerPC C and Rust layout assertions check the bridge descriptors. A
temporary Cafe Rust staticlib has linked against this presenter. An RPX built
with `cargo wiiu rpx` reached Cemu's title, but an instrumented probe reported
allocation failure for both a direct C control and the Rust presenter before
either draw. Its GFD is 16-byte aligned and lives in the executable. The failure
stage has not yet been isolated; the Rust bridge has not been verified to draw.

Build the PowerPC archive and GFD in WSL with devkitPro WUT and CafeGLSL on PATH:

```sh
CAFEGLSL_ROOT=/path/to/cafeglsl bash wiiu/gx2_hud/build.sh
```

The default output is `/tmp/horizonwiiu-gx2-hud`; set `HORIZON_WIIU_HUD_BUILD_DIR`
to choose another directory. Exercise the portable data validation separately:

```sh
cc -std=c11 -Wall -Wextra -Werror wiiu/gx2_hud/hud_data.c wiiu/gx2_hud/tests/hud_data_test.c -lm -o /tmp/horizonwiiu-hud-data-test
/tmp/horizonwiiu-hud-data-test
```
