//! The browser has no files to open or save, so projects are native-only.

use super::EditorApp;
use eframe::egui;
use osrs_cache::source::CacheSource;
use osrs_core::coords::RegionCoord;
use std::path::PathBuf;

impl EditorApp {
    /// Browser project persistence is unavailable, so there is no project dirty-state lifecycle to
    /// guard. Keep cache/source replacement routed through the same call site as native builds.
    pub(super) fn request_source_change(
        &mut self,
        source: CacheSource,
        label: PathBuf,
        ctx: &egui::Context,
    ) {
        self.session.reset();
        self.open_source(source, label, ctx);
    }

    pub(super) fn script_project(&mut self, _action: &str, _path: &str) -> Result<String, String> {
        Err("projects are not available in the browser".to_owned())
    }

    pub(super) fn project_menu(&mut self, _ui: &mut egui::Ui) {}

    pub(super) fn project_actions(&mut self, _ctx: &egui::Context) {}

    pub(super) fn quit_guard(&mut self, _ctx: &egui::Context) {}

    pub(super) fn poll_project(&mut self) {}

    pub(super) fn apply_project_region(&mut self, _region: RegionCoord) {}
}
