//! Choosing and switching the OSRS cache: the File menu, the recent list and the first-run card.
//!
//! A cache is a folder holding `main_file_cache.dat2` and the `.idx` files. It is identified by its
//! fingerprint against the bundled build profiles (`osrs_cache::registry`), so any supported build
//! opens without configuration and an unknown one is reported with what is supported.

use super::EditorApp;
use crate::assets::Assets;
use eframe::egui;
use osrs_world::RegionStreamer;
use std::path::{Path, PathBuf};

const RECENT_LIMIT: usize = 8;
const DATA_FILE: &str = "main_file_cache.dat2";

/// Whether `dir` looks like a cache folder.
pub fn looks_like_cache(dir: &Path) -> bool {
    dir.join(DATA_FILE).is_file()
}

impl EditorApp {
    /// Ask for a cache folder with the system dialog and open it.
    pub(super) fn pick_cache_folder(&mut self, ctx: &egui::Context) {
        let mut dialog = rfd::FileDialog::new()
            .set_title("Choose the OSRS cache folder (it contains main_file_cache.dat2)");
        if let Some(parent) = self.cache_dir.parent() {
            dialog = dialog.set_directory(parent);
        }
        if let Some(folder) = dialog.pick_folder() {
            self.open_cache(folder, ctx);
        }
    }

    /// Switch to the cache in `dir`: a new streamer and asset worker, every loaded region dropped.
    pub(super) fn open_cache(&mut self, dir: PathBuf, ctx: &egui::Context) {
        if !looks_like_cache(&dir) {
            self.cache_error = Some(format!(
                "{} has no {DATA_FILE}. Choose the folder that contains it.",
                dir.display()
            ));
            return;
        }
        self.cache_error = None;
        self.cache_build = None;
        // The old workers finish their current job before stopping; do not wait for them here.
        let old_streamer = std::mem::replace(
            &mut self.streamer,
            RegionStreamer::spawn(dir.clone(), Self::presentation(false, false)),
        );
        let old_assets = std::mem::replace(&mut self.assets, Assets::new(dir.clone()));
        std::thread::spawn(move || {
            drop(old_streamer);
            drop(old_assets);
        });
        self.rebuild_all();
        self.infos.clear();
        self.infos_version += 1;
        self.edited.clear();
        self.history.clear();
        self.redo_stack.clear();
        self.height_edited.clear();
        self.dirty_regions.clear();
        self.stroke = None;
        self.spline.points.clear();
        self.texture_handles.clear();
        self.floor_textures = None;
        self.palette = None;
        self.palette_rx = Some(Self::spawn_palette_loader(dir.clone()));
        self.ready = false;

        let text = dir.to_string_lossy().into_owned();
        self.prefs.cache_dir = Some(text.clone());
        self.prefs.recent_caches.retain(|recent| *recent != text);
        self.prefs.recent_caches.insert(0, text);
        self.prefs.recent_caches.truncate(RECENT_LIMIT);
        self.cache_dir = dir;
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(
            "RustOSRS: opening cache...".to_owned(),
        ));
    }

    /// The matched build arrived from the streamer.
    pub(super) fn cache_opened(&mut self, ctx: &egui::Context, profile_id: String, build: u32) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
            "RustOSRS: build {build}"
        )));
        self.notice = Some((
            format!("Opened cache build {build} ({profile_id})"),
            std::time::Instant::now(),
        ));
        self.cache_build = Some((build, profile_id));
    }

    pub(super) fn spawn_palette_loader(
        dir: PathBuf,
    ) -> std::sync::mpsc::Receiver<Result<osrs_world::FloorPalette, osrs_world::WorldError>> {
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(osrs_world::load_floor_palette(dir));
        });
        receiver
    }

    /// File menu entries: open, and the recent caches.
    pub(super) fn cache_menu(&mut self, ui: &mut egui::Ui) {
        if ui
            .button(format!(
                "{}  Open cache folder...",
                egui_phosphor::regular::FOLDER_OPEN
            ))
            .clicked()
        {
            ui.close();
            let ctx = ui.ctx().clone();
            self.pick_cache_folder(&ctx);
        }
        let recent: Vec<String> = self
            .prefs
            .recent_caches
            .iter()
            .filter(|path| Path::new(path.as_str()).is_dir())
            .cloned()
            .collect();
        if !recent.is_empty() {
            ui.menu_button("Recent caches", |ui| {
                for path in recent {
                    let current = Path::new(&path) == self.cache_dir;
                    if ui.selectable_label(current, &path).clicked() {
                        ui.close();
                        let ctx = ui.ctx().clone();
                        self.open_cache(PathBuf::from(path), &ctx);
                    }
                }
            });
        }
        if let Some((build, profile)) = &self.cache_build {
            ui.separator();
            ui.weak(format!("Build {build}"));
            ui.weak(profile.as_str());
        }
    }

    /// The first-run / failure card: shown while no cache is open.
    pub(super) fn cache_welcome(&mut self, ctx: &egui::Context) {
        let Some(error) = self.cache_error.clone() else {
            return;
        };
        egui::Window::new("OSRS cache")
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .collapsible(false)
            .resizable(false)
            .order(egui::Order::Foreground)
            .default_width(460.0)
            .show(ctx, |ui| {
                ui.label(egui::RichText::new("Choose an OSRS cache").heading());
                ui.add_space(4.0);
                ui.label(
                    "RustOSRS reads the game cache from a folder on this computer. \
                     Pick the folder that contains main_file_cache.dat2.",
                );
                ui.add_space(6.0);
                ui.colored_label(ui.visuals().warn_fg_color, &error);
                ui.add_space(8.0);
                if ui
                    .add(
                        egui::Button::new(format!(
                            "{}  Choose cache folder...",
                            egui_phosphor::regular::FOLDER_OPEN
                        ))
                        .min_size(egui::vec2(240.0, 32.0)),
                    )
                    .clicked()
                {
                    self.pick_cache_folder(ctx);
                }
                let recent: Vec<String> = self
                    .prefs
                    .recent_caches
                    .iter()
                    .filter(|path| Path::new(path.as_str()).is_dir())
                    .cloned()
                    .collect();
                if !recent.is_empty() {
                    crate::theme::section_heading(ui, "Recent");
                    for path in recent {
                        if ui.button(&path).clicked() {
                            self.open_cache(PathBuf::from(path), ctx);
                        }
                    }
                }
                crate::theme::section_heading(ui, "Supported builds");
                if let Ok(profiles) = osrs_cache::registry::bundled_profiles() {
                    for profile in profiles {
                        ui.weak(format!(
                            "build {}  \u{b7}  OpenRS2 cache {}  \u{b7}  {}",
                            profile.cache_source.build,
                            profile.cache_source.cache_id,
                            profile.cache_source.timestamp
                        ));
                    }
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.link("Get a cache from OpenRS2").clicked() {
                        let _ = open::that("https://archive.openrs2.org/caches");
                    }
                    ui.weak("(download the \"disk\" zip of a supported build and unzip it)");
                });
            });
    }

    /// The cache to start with: the command line, the environment, the last one used, else the default.
    pub fn initial_cache_dir(arg: Option<PathBuf>) -> PathBuf {
        if let Some(dir) = arg {
            return dir;
        }
        if let Ok(dir) = std::env::var("RUSTOSRS_TARGET_CACHE_DIR") {
            return PathBuf::from(dir);
        }
        if let Some(path) = crate::settings::path()
            && let Some(dir) = crate::settings::Settings::load(&path).cache_dir
            && looks_like_cache(Path::new(&dir))
        {
            return PathBuf::from(dir);
        }
        let home = std::env::var("HOME").map(PathBuf::from).unwrap_or_default();
        home.join("RustOSRS-cache/b241/cache")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_without_the_data_file_is_not_a_cache() {
        assert!(!looks_like_cache(Path::new("/definitely/not/a/cache")));
        assert!(!looks_like_cache(&std::env::temp_dir()));
    }
}
