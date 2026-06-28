//! Build script: on Windows, embed the application icon into the .exe so it
//! shows up in Explorer, the taskbar and Add/Remove Programs.
//!
//! On macOS/Linux this is a no-op (and `winresource` isn't even a dependency).

fn main() {
    #[cfg(windows)]
    embed_windows_icon();
}

#[cfg(windows)]
fn embed_windows_icon() {
    let icon = "assets/icon.ico";
    if std::path::Path::new(icon).exists() {
        let mut res = winresource::WindowsResource::new();
        res.set_icon(icon);
        if let Err(e) = res.compile() {
            println!("cargo:warning=failed to embed icon: {e}");
        }
    } else {
        println!("cargo:warning={icon} not found; run tools/iconforge first for an app icon");
    }
}
