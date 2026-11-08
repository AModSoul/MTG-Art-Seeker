fn main() {
    // tauri-build embeds `icons/icon.ico` in the Windows executable, but only
    // re-runs when the config or capabilities change: a new icon would keep
    // the old one in the taskbar until something else did.
    println!("cargo:rerun-if-changed=icons");
    tauri_build::build();
}
