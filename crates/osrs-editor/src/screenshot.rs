//! Smoke-test hook: save the editor window to a PNG and exit.
//!
//! `RUSTOSRS_SCREENSHOT=path.png` captures after `RUSTOSRS_SCREENSHOT_FRAME` frames (default 300, enough for
//! the first regions to stream in). It captures the whole window as egui drew it, so panels, windows and
//! menus are included. Used to look at the UI instead of guessing.

use eframe::egui;
use std::path::PathBuf;

pub struct ScreenshotHook {
    path: PathBuf,
    at_frame: u64,
    requested: bool,
}

impl ScreenshotHook {
    pub fn from_env() -> Option<Self> {
        let path = PathBuf::from(std::env::var_os("RUSTOSRS_SCREENSHOT")?);
        let at_frame = std::env::var("RUSTOSRS_SCREENSHOT_FRAME")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(300);
        Some(Self {
            path,
            at_frame,
            requested: false,
        })
    }

    /// Call once per frame. Closes the window after the capture is saved.
    pub fn tick(&mut self, ctx: &egui::Context, frame: u64) {
        // Ask again every 30 frames until the capture arrives (a request can be dropped while a
        // popup is opening).
        if frame >= self.at_frame && (!self.requested || frame.is_multiple_of(30)) {
            self.requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        let captured = ctx.input(|input| {
            input.events.iter().find_map(|event| match event {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = captured {
            let (width, height) = (image.size[0] as u32, image.size[1] as u32);
            let bytes: Vec<u8> = image
                .pixels
                .iter()
                .flat_map(|pixel| pixel.to_array())
                .collect();
            match image::RgbaImage::from_raw(width, height, bytes) {
                Some(rgba) => match rgba.save(&self.path) {
                    Ok(()) => eprintln!("screenshot: wrote {}", self.path.display()),
                    Err(error) => eprintln!("screenshot: {error}"),
                },
                None => eprintln!("screenshot: bad buffer size"),
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}
