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
mod camera;
#[cfg(not(target_arch = "wasm32"))]
mod export_flow;
mod fonts;
mod grid;
mod heights;
mod hud;
mod icons;
mod interact;
mod keymap;
mod paint;
mod pick;
mod platform;
mod player;
mod plugin;
mod screenshot;
mod session;
mod settings;
mod sound;
mod style;
mod terrain_path;
mod theme;
mod user_tools;
mod wiki;

#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

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

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args
        .get(1)
        .is_some_and(|arg| arg == "--verify-authored-snapshot")
    {
        return session::authored_snapshot::run_cli(&args[2..])
            .map_err(|error| eframe::Error::AppCreation(error.into()));
    }
    if args.get(1).is_some_and(|arg| arg == "--reproduce-building") {
        return app::reproduce_cli(&args[2..])
            .map_err(|error| eframe::Error::AppCreation(error.into()));
    }
    // The command line, then RUSTOSRS_TARGET_CACHE_DIR, then the cache used last time.
    let cache_dir = app::EditorApp::initial_cache_dir(args.get(1).map(PathBuf::from));
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

/// In the browser the editor draws into the page's `#rustosrs_canvas`; the cache folder is chosen
/// from the first-run card (File System Access API) or `?cache=<url prefix>` for development.
#[cfg(target_arch = "wasm32")]
fn main() {
    use wasm_bindgen::JsCast;
    console_error_panic_hook::set_once();
    wasm_bindgen_futures::spawn_local(async {
        let canvas = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.get_element_by_id("rustosrs_canvas"))
            .and_then(|element| element.dyn_into::<web_sys::HtmlCanvasElement>().ok());
        let Some(canvas) = canvas else {
            web_sys::console::error_1(&"index.html has no #rustosrs_canvas".into());
            return;
        };
        let options = eframe::WebOptions {
            wgpu_options: wgpu_options(),
            ..Default::default()
        };
        let result = eframe::WebRunner::new()
            .start(
                canvas,
                options,
                Box::new(|cc| {
                    Ok(Box::new(app::EditorApp::new(
                        cc,
                        std::path::PathBuf::new(),
                        3176,
                        3176,
                    )?))
                }),
            )
            .await;
        platform::web::hide_loading();
        if let Err(error) = result {
            web_sys::console::error_1(&error);
        }
    });
}
