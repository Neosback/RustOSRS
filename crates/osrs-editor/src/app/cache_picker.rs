//! Choosing and switching the OSRS cache: the File menu, the recent list and the first-run card.
//!
//! A cache is a folder holding `main_file_cache.dat2` and the `.idx` files. It is identified by its
//! fingerprint against the bundled build profiles (`osrs_cache::registry`), so any supported build
//! opens without configuration and an unknown one is reported with what is supported.

use super::EditorApp;
use super::world_runtime::WorldRuntime;
use crate::assets::Assets;
use eframe::egui;
use osrs_cache::source::CacheSource;
use std::path::{Path, PathBuf};

const RECENT_LIMIT: usize = 8;
const DATA_FILE: &str = "main_file_cache.dat2";

/// Whether `dir` looks like a cache folder. (The browser has no folders to look in: it reads the
/// cache the page was given.)
pub fn looks_like_cache(dir: &Path) -> bool {
    cfg!(not(target_arch = "wasm32")) && dir.join(DATA_FILE).is_file()
}

impl EditorApp {
    /// Ask for a cache folder with the system dialog and open it.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn pick_cache_folder(&mut self, ctx: &egui::Context) {
        let mut dialog = rfd::FileDialog::new()
            .set_title("Choose the OSRS cache folder (it contains main_file_cache.dat2)");
        if let Some(parent) = self.project.cache_label().parent() {
            dialog = dialog.set_directory(parent);
        }
        if let Some(folder) = dialog.pick_folder() {
            self.open_cache(folder, ctx);
        }
    }

    /// Ask the browser for a cache folder (File System Access API) and read it into memory.
    #[cfg(target_arch = "wasm32")]
    pub(super) fn pick_cache_folder(&mut self, ctx: &egui::Context) {
        let repaint = ctx.clone();
        self.web_pick = Some(crate::platform::web::start_pick(None, move || {
            repaint.request_repaint();
        }));
    }

    /// Read the cache from `?cache=<url prefix>` (development: no folder dialog to click through).
    #[cfg(target_arch = "wasm32")]
    pub(super) fn pick_cache_url(&mut self, ctx: &egui::Context, base: String) {
        let repaint = ctx.clone();
        self.web_pick = Some(crate::platform::web::start_pick(Some(base), move || {
            repaint.request_repaint();
        }));
    }

    /// Open the cache the browser finished reading, or show why it could not.
    #[cfg(target_arch = "wasm32")]
    pub(super) fn poll_web_pick(&mut self, ctx: &egui::Context) {
        use crate::platform::web::PickState;
        let Some(pick) = &self.web_pick else {
            return;
        };
        let done = match &mut *pick.borrow_mut() {
            PickState::Reading(_) => return,
            PickState::Done(result) => std::mem::replace(result, Err(String::new())),
        };
        self.web_pick = None;
        match done {
            Ok(picked) => {
                let source = CacheSource::Memory(std::sync::Arc::new(picked.files));
                self.request_source_change(source, PathBuf::from(picked.name), ctx);
            }
            Err(error) if error.is_empty() => {}
            Err(error) => self.project.set_cache_error(error),
        }
    }

    /// Switch to the cache in `dir`, after the project lifecycle guard has protected unsaved work.
    pub(super) fn open_cache(&mut self, dir: PathBuf, ctx: &egui::Context) {
        if !looks_like_cache(&dir) {
            self.project.set_cache_error(format!(
                "{} has no {DATA_FILE}. Choose the folder that contains it.",
                dir.display()
            ));
            return;
        }
        self.request_source_change(CacheSource::Dir(dir.clone()), dir, ctx);
    }

    /// Restart the current native cache from disk. Project open uses this to discard the current
    /// semantic/projection working set and rebuild from the pristine source before applying diffs.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn restart_current_source(&mut self) {
        let dir = self.project.cache_label().clone();
        self.replace_source(CacheSource::Dir(dir.clone()), dir);
    }

    /// Switch to `source`; lifecycle callers must already have resolved unsaved work.
    pub(super) fn open_source(&mut self, source: CacheSource, label: PathBuf, ctx: &egui::Context) {
        self.replace_source(source, label);
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(
            "RustOSRS: opening cache...".to_owned(),
        ));
    }

    /// Replace streamer/assets and clear every resident projection tied to the previous source.
    fn replace_source(&mut self, source: CacheSource, label: PathBuf) {
        self.exports.reset_source();
        let dir = label;
        self.project.replace_source(source.clone(), dir.clone());
        // The old workers finish their current job before stopping; do not wait for them here.
        let old_streamer = self
            .world
            .replace_streamer(WorldRuntime::spawn_streamer(source.clone()));
        let old_assets = std::mem::replace(&mut self.assets, Assets::new(source.clone()));
        #[cfg(not(target_arch = "wasm32"))]
        std::thread::spawn(move || {
            drop(old_streamer);
            drop(old_assets);
        });
        #[cfg(target_arch = "wasm32")]
        {
            drop(old_streamer);
            drop(old_assets);
        }
        self.rebuild_all();
        self.world.clear_inspection();
        self.terrain.reset();
        self.interaction.clear_spline_points();
        self.palettes.reset();
        self.palettes.load(source.clone());
        self.start_catalog_loader(source.clone());

        if matches!(source, CacheSource::Dir(_)) {
            let text = dir.to_string_lossy().into_owned();
            self.settings.prefs.cache_dir = Some(text.clone());
            self.settings
                .prefs
                .recent_caches
                .retain(|recent| *recent != text);
            self.settings.prefs.recent_caches.insert(0, text);
            self.settings.prefs.recent_caches.truncate(RECENT_LIMIT);
        }
    }

    /// The matched build arrived from the streamer.
    pub(super) fn cache_opened(&mut self, ctx: &egui::Context, profile_id: String, build: u32) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
            "RustOSRS: build {build}"
        )));
        self.ui
            .notify(format!("Opened cache build {build} ({profile_id})"));
        self.project.source_opened(build, profile_id);
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
            .settings
            .prefs
            .recent_caches
            .iter()
            .filter(|path| Path::new(path.as_str()).is_dir())
            .cloned()
            .collect();
        if !recent.is_empty() {
            ui.menu_button("Recent caches", |ui| {
                for path in recent {
                    let current = Path::new(&path) == self.project.cache_label().as_path();
                    if ui.selectable_label(current, &path).clicked() {
                        ui.close();
                        let ctx = ui.ctx().clone();
                        self.open_cache(PathBuf::from(path), &ctx);
                    }
                }
            });
        }
        if let Some((build, profile)) = &self.project.cache_build() {
            ui.separator();
            ui.weak(format!("Build {build}"));
            ui.weak(profile.as_str());
        }
    }

    /// The first-run / failure card: shown while no cache is open.
    pub(super) fn cache_welcome(&mut self, ctx: &egui::Context) {
        let Some(error) = self.project.cache_error().clone() else {
            return;
        };
        super::modal::Modal::new("cache_welcome", "OSRS cache")
            .size(460.0, 0.0)
            .closable(false)
            .show(ctx, |ui| {
                ui.label(egui::RichText::new("Choose an OSRS cache").heading());
                ui.add_space(4.0);
                ui.label(
                    "RustOSRS reads the game cache from a folder on this computer. \
                     Pick the folder that contains main_file_cache.dat2.",
                );
                ui.add_space(6.0);
                #[cfg(target_arch = "wasm32")]
                if let Some(pick) = &self.web_pick
                    && let crate::platform::web::PickState::Reading(message) = &*pick.borrow()
                {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(message);
                    });
                    return;
                }
                ui.colored_label(ui.visuals().warn_fg_color, &error);
                ui.add_space(8.0);
                if ui
                    .add(
                        crate::theme::primary_button(format!(
                            "{}  Choose cache folder...",
                            egui_phosphor::regular::FOLDER_OPEN
                        ))
                        .min_width(240.0),
                    )
                    .clicked()
                {
                    self.pick_cache_folder(ctx);
                }
                let recent: Vec<String> = self
                    .settings
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
                        let _ = crate::platform::open_url("https://archive.openrs2.org/caches");
                    }
                    ui.weak("(download the \"disk\" zip of a supported build and unzip it)");
                });
            });
    }

    #[cfg(not(target_arch = "wasm32"))]
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
