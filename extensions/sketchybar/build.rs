const SKETCHYBAR_MACH_SOURCE: &str = "c/sketchybar_mach.c";

fn main() {
    println!("cargo:rerun-if-changed={SKETCHYBAR_MACH_SOURCE}");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new()
            .file(SKETCHYBAR_MACH_SOURCE)
            .warnings(true)
            .compile("sketchybar_mach");
    }
}
