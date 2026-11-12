//! Links libvita2d (MIT, https://github.com/xerpi/libvita2d) and the system stubs it needs.
//! vita2d sets up GXM, the depth buffer and precompiled shaders; the 3D drawing is ours.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("vita") {
        return;
    }
    let sdk = std::env::var("VITASDK").expect("VITASDK must point to the Vita SDK");
    println!("cargo:rerun-if-env-changed=VITASDK");
    println!("cargo:rustc-link-search=native={sdk}/arm-vita-eabi/lib");
    // Order matters to the GNU linker: a library before the ones it needs.
    for lib in [
        "vita2d",
        "SceGxm_stub",
        "SceDisplay_stub",
        "ScePgf_stub",
        "ScePvf_stub",
        "SceCommonDialog_stub",
        "SceSysmodule_stub",
        "SceAppMgr_stub",
        "SceLibKernel_stub",
        "m",
    ] {
        println!("cargo:rustc-link-lib=static={lib}");
    }
}
