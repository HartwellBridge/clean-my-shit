// Hide the console window on Windows release builds (GUI app).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod categories;
mod cleaner;
mod safety;
mod scanner;
mod util;

/// App icon, embedded at compile time. Used for the window/taskbar/dock icon
/// and shown in the header.
pub(crate) const ICON_PNG: &[u8] = include_bytes!("../assets/icon.png");

fn main() -> eframe::Result<()> {
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_inner_size([780.0, 600.0])
        .with_min_inner_size([560.0, 440.0])
        .with_title("Clean My Shit");

    if let Ok(icon) = eframe::icon_data::from_png_bytes(ICON_PNG) {
        viewport = viewport.with_icon(std::sync::Arc::new(icon));
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Clean My Shit",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}
