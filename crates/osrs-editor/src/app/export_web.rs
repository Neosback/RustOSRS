//! Browser builds retain their existing native-only export boundary.
use super::EditorApp;
use eframe::egui;

#[derive(Default)]
pub(super) struct ExportRuntime {
    _native_only: (),
}

impl ExportRuntime {
    pub(in crate::app) fn reset_source(&mut self) {}
}

impl EditorApp {
    pub(super) fn export_menu(&mut self, _ui: &mut egui::Ui) {}
    pub(super) fn poll_export(&mut self) {}
    pub(super) fn export_window(&mut self, _ctx: &egui::Context) {}
    pub(super) fn export_modal_open(&self) -> bool {
        false
    }
    pub(super) fn script_export(&mut self, _action: &str, _path: &str) -> Result<bool, String> {
        Err("Export is available in the native editor.".to_owned())
    }
}
