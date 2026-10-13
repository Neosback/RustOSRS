//! The browser has no files to open or save, so projects are native-only.

use super::EditorApp;
use eframe::egui;
use osrs_core::coords::RegionCoord;

#[derive(Default)]
pub(super) struct ProjectState {
    _native_only: (),
}

impl EditorApp {
    pub(super) fn script_project(&mut self, _action: &str, _path: &str) -> Result<String, String> {
        Err("projects are not available in the browser".to_owned())
    }

    pub(super) fn project_menu(&mut self, _ui: &mut egui::Ui) {}

    pub(super) fn project_actions(&mut self, _ctx: &egui::Context) {}

    pub(super) fn poll_project(&mut self) {
        // Keep the field uniform across targets; nothing ever starts a project job here.
        self.project = ProjectState::default();
    }

    pub(super) fn apply_project_region(&mut self, _region: RegionCoord) {}
}
