//! The browser has no folders to write to, so File > Export is native-only.

use super::EditorApp;
use eframe::egui;

pub(super) struct ExportJob;

impl EditorApp {
    pub(super) fn export_menu(&mut self, _ui: &mut egui::Ui) {}

    pub(super) fn poll_export(&mut self) {
        // Nothing ever starts a job in the browser; keep the field uniform across targets.
        self.export_job = None;
    }
}
