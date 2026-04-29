fn main() {
    #[cfg(windows)]
    if std::env::var_os("CARGO_FEATURE_AUTO_ELEVATION").is_some() {
        println!("cargo:rustc-link-arg=/MANIFESTUAC:level=requireAdministrator uiAccess=false");
    }

    tauri_build::build()
}
