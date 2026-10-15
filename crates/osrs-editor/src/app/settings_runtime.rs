//! Persisted preferences and key bindings. Runtime camera/plugin mirrors are composed by the
//! app into a snapshot; this owner controls when and where that snapshot is saved.
use crate::{
    keymap::Keymap,
    settings::{self, Settings},
};
use std::path::PathBuf;
use web_time::Instant;
pub(super) struct SettingsRuntime {
    pub(super) prefs: Settings,
    pub(super) keymap: Keymap,
    path: Option<PathBuf>,
    saved: Settings,
    checked: Instant,
}
impl SettingsRuntime {
    pub(super) fn new() -> Self {
        Self {
            prefs: Settings::default(),
            keymap: Keymap::new(),
            path: None,
            saved: Settings::default(),
            checked: Instant::now(),
        }
    }
    pub(super) fn load(&mut self) -> Option<Settings> {
        self.path = settings::path();
        self.path.as_deref().map(Settings::load)
    }
    pub(super) fn mark_loaded(&mut self, snapshot: Settings) {
        self.saved = snapshot;
    }
    pub(super) fn save_due(&mut self) -> bool {
        if self.path.is_none() || self.checked.elapsed().as_secs_f32() < 1.0 {
            return false;
        }
        self.checked = Instant::now();
        true
    }
    pub(super) fn save_changed(&mut self, snapshot: Settings) {
        if snapshot != self.saved
            && let Some(path) = &self.path
            && snapshot.save(path).is_ok()
        {
            self.saved = snapshot;
        }
    }
    pub(super) fn save_on_exit(&self, snapshot: &Settings) {
        if let Some(path) = &self.path {
            let _ = snapshot.save(path);
        }
    }
}
