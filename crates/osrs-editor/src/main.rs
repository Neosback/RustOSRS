//! RustOSRS map viewer/editor shell (eframe + wgpu).
//!
//! First runnable slice: loads a real scene window from the pinned build-241 cache, renders it
//! with the reference wgpu renderer inside an egui viewport, and lets you fly around.
//!
//! usage: osrs-editor [cache-dir] [base-x base-y]
//! The cache directory defaults to `$RUSTOSRS_TARGET_CACHE_DIR`, then `~/RustOSRS-cache/b241/cache`.

mod animation_smoothing;
mod app;
mod assets;
mod autotile;
mod camera;
mod fonts;
mod grid;
mod heights;
mod hud;
mod icons;
mod interact;
mod keymap;
mod paint;
mod pick;
mod plugin;
mod screenshot;
mod settings;
mod theme;
mod wiki;

use std::path::PathBuf;

fn default_cache_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("RUSTOSRS_TARGET_CACHE_DIR") {
        return PathBuf::from(dir);
    }
    let home = std::env::var("HOME").map(PathBuf::from).unwrap_or_default();
    home.join("RustOSRS-cache/b241/cache")
}

/// eframe's default wgpu setup plus GPU timestamp queries (for the scene pass timing) when the
/// adapter has them, and the texture-array limit the HD profile needs.
fn wgpu_options() -> eframe::egui_wgpu::WgpuConfiguration {
    use eframe::egui_wgpu::{WgpuConfiguration, WgpuSetup, WgpuSetupCreateNew};
    use std::sync::Arc;
    let mut setup = WgpuSetupCreateNew::without_display_handle();
    let default_descriptor = setup.device_descriptor.clone();
    setup.device_descriptor = Arc::new(move |adapter| {
        let mut descriptor = default_descriptor(adapter);
        descriptor.required_features |=
            adapter.features() & eframe::wgpu::Features::TIMESTAMP_QUERY;
        // The HD material texture array needs more layers than the WebGPU default.
        descriptor.required_limits =
            osrs_render::gpu::hd::raise_limits(descriptor.required_limits, &adapter.limits());
        descriptor
    });
    let mut configuration = WgpuConfiguration {
        wgpu_setup: WgpuSetup::CreateNew(setup),
        ..Default::default()
    };
    // `RUSTOSRS_VSYNC=0` presents without waiting for the display, to measure real frame rates.
    if std::env::var("RUSTOSRS_VSYNC").is_ok_and(|value| value == "0") {
        configuration.surface.present_mode = eframe::wgpu::PresentMode::AutoNoVsync;
    }
    configuration
}

/// 1360 x 860, or `RUSTOSRS_WINDOW="w,h"` (to look at the responsive layout).
fn window_size() -> [f32; 2] {
    std::env::var("RUSTOSRS_WINDOW")
        .ok()
        .and_then(|value| {
            let (w, h) = value.split_once(',')?;
            Some([w.trim().parse().ok()?, h.trim().parse().ok()?])
        })
        .unwrap_or([1360.0, 860.0])
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
            .with_inner_size(window_size())
            .with_title("RustOSRS"),
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: wgpu_options(),
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
