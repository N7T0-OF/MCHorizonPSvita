# GX2 HUD diagnostic

This opt-in RPX exercises the actual GX2 HUD presenter with original test pixels:
an atlas upload and patch, ordered alpha/additive/alpha runs, and rejection of an unsupported
invert run. It samples independently coloured red, green and blue texels with white
vertices and checks that a fully transparent texel leaves the background unchanged.
It is a diagnostic fixture, not a Horizon menu or gameplay. The target
is built separately and is not linked into the normal Wii U client RPX.

With devkitPro/WUT, CMake, Ninja and CafeGLSL installed, from the project root in
WSL (adjust the tool paths for the machine):

```sh
CAFEGLSL_ROOT=/home/ben/HorizonWiiU-toolchain/build-async-default/deps/cafeglsl-1.0.0-rc1-linux-x86_64 \
HORIZON_WIIU_HUD_DIAG_BUILD_DIR=/mnt/c/Users/ben/AppData/Local/Temp/HorizonWiiU-hud-diagnostic/build \
bash wiiu/gx2_hud/diagnostic/build.sh
```

The generated `horizon_gx2_hud_diag.rpx` tests pixels with `GX2CopySurface`.
The copy uses `GX2_TILE_MODE_LINEAR_SPECIAL` because Cemu 2.6 forces a texture
readback for the observed tiled source and this destination. This is an emulator
verification technique, not evidence that this readback path works on hardware.
For this single-frame diagnostic, set `<AsyncCompile>false</AsyncCompile>` in
the isolated Cemu `portable/settings.xml`; otherwise the first draw may precede
asynchronous shader compilation. Its fixed readback positions require a TV
target wider than 1184 pixels and taller than 64 pixels.
Exit status is zero only if the sampled pixels pass. The diagnostic reports its
status via `OSReport` and, when an SD card is mounted and the directory exists,
`wiiu/apps/HorizonWiiU/hud-diagnostic-status.log`. Absence of the file is not a
pass; mount, client, open or write failures can prevent its creation.

## Recorded Cemu 2.6 run

The additive-blend diagnostic built on 2026-11-07 to RPX SHA-256
`5e625dc1e5a197ad4068498c1f54fbac7db9d1fb11b6d776e522d86d94ed3384`.
An isolated patched Cemu 2.6 run with synchronous shader compilation read back
`(153,102,76,191)` for additive `(102,51,25,128)` over the grey
`(51,51,51,63)` background, and `(0,0,255,255)` from the following alpha run.
All existing texture and transparency samples also passed. This is a diagnostic
fixture, not a live menu, and hardware has not been tested. Evidence is under
`E:\temp\HorizonWiiU-hud-additive` on the build machine.

The expanded RGBA diagnostic built on 2026-11-07 to RPX SHA-256
`36811cc0fbc7c9f35a6ac7461ecd158215937ceb5ecf3589191e56e4ae929aa8`.
An isolated patched Cemu 2.6 run with synchronous shader compilation reported
`readback=pass`: texture red `(255,0,0,255)`, texture green `(0,255,0,255)`,
texture blue `(0,0,255,255)`, and the transparent sample matched the background
`(51,51,51,255)`. The original vertex-tinted red and green checks also passed.
This checks fully transparent discard, not partial-alpha blending or real hardware.
A separate temporary build with red and green component selectors deliberately
swapped failed the new texture checks while the original tinted checks passed.

The official portable Cemu release was extracted in the temporary diagnostic
folder, with its own `portable/sdcard` and settings. The existing user SD and
Cemu installation were not used for the test. With the SD status directory
already created, this was the exact PowerShell launch:

```powershell
$base = "$env:TEMP\HorizonWiiU-hud-diagnostic"
Start-Process -FilePath "$base\cemu\Cemu_2.6\Cemu.exe" -ArgumentList @('-g', "`"$base\build\horizon_gx2_hud_diag.rpx`"") -WorkingDirectory "$base\cemu\Cemu_2.6" -WindowStyle Hidden -PassThru
```

The 2026-11-07 diagnostic RPX had SHA-256
`3C885E77FE414C79985BA67F0C0F11ED4BCDD908ED5E4CEA4D8657DFDFCB2B78`;
Cemu identified its module checksum as `0x40bb9ddf` and logged title exit 0.
The isolated SD status reported `readback=pass`, 1920x1080, source/destination
tile modes 4/16, and red `(255,0,0,255)`, green `(0,255,0,255)`, and background
`(51,51,51,255)` samples. The status and Cemu log were preserved in the temp
folder as `readback-pass-status.log` and `readback-pass-cemu.log`.

In the earlier run (RPX SHA-256
`9E0F3B52E2FCACD94177E11A3296A73A32B6DDC460941417C88B550B3402A253`,
Cemu checksum `0xb490389a`), the user directly observed a grey background with
red and green boxes. That version's linear-aligned readback returned all zeroes;
its original evidence is `first-run-status.log` and `first-run-cemu.log` in the
same temp folder. Neither run demonstrates actual Horizon `HudBatch` integration,
menu navigation, GamePad input, or execution on real hardware.
