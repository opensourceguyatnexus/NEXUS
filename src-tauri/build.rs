fn main() {
    // The icon is compiled into the exe, so a new icon has to trigger a rebuild.
    println!("cargo:rerun-if-changed=icons/icon.ico");
    tauri_build::build()
}
