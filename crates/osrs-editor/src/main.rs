//! RustOSRS map viewer/editor shell (eframe + wgpu).
//!
//! First runnable slice: loads a real scene window from the pinned build-241 cache, renders it
//! with the reference wgpu renderer inside an egui viewport, and lets you fly around.
//!
//! usage: osrs-editor [cache-dir] [base-x base-y]
//! The cache directory defaults to `$RUSTOSRS_TARGET_CACHE_DIR`, then `~/RustOSRS-cache/b241/cache`.

mod app;
mod camera;
mod pick;
mod theme;

use std::path::PathBuf;

fn default_cache_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("RUSTOSRS_TARGET_CACHE_DIR") {
        return PathBuf::from(dir);
    }
    let home = std::env::var("HOME").map(PathBuf::from).unwrap_or_default();
    home.join("RustOSRS-cache/b241/cache")
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let cache_dir = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(default_cache_dir);
    // Default window: the 3x3 regions around Lumbridge.
    let base_x = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(3176);
    let base_y = args.get(3).and_then(|v| v.parse().ok()).unwrap_or(3176);

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1360.0, 860.0])
            .with_title("RustOSRS"),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "RustOSRS",
        options,
        Box::new(move |cc| {
            Ok(Box::new(app::EditorApp::new(
                cc, cache_dir, base_x, base_y,
            )?))
        }),
    )
}
