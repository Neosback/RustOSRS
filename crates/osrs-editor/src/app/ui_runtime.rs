//! Transient presentation state. Widgets may edit UI-local fields; product changes are emitted
//! as actions and routed by the application. No map, history, worker or GPU authority lives here.
mod script_window;
use eframe::egui;
use web_time::Instant;

/// How many recently used tools the picker rail keeps as shortcuts.
const RECENT_TOOLS: usize = 3;

pub(super) struct UiRuntime {
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) studio_quit_pending: bool,
    pub(super) prefab_workspace: super::prefab_studio::workspace::Workspace,
    pub(super) themes: super::theme_studio::ThemeStudio,
    pub(super) palette_search: String,
    pub(super) context_menu: Option<egui::Pos2>,
    pub(super) settings_open: bool,
    pub(super) settings_search: String,
    pub(super) settings_focus_search: bool,
    pub(super) capturing_binding: Option<&'static str>,
    pub(super) script_open: bool,
    pub(super) plugins_open: bool,
    pub(super) script_text: String,
    pub(super) tool_draft: Option<crate::user_tools::ToolDraft>,
    pub(super) hud_rect: Option<egui::Rect>,
    pub(super) rail_rect: Option<egui::Rect>,
    pub(super) quick_open: Option<egui::Pos2>,
    pub(super) world_map: super::worldmap::WorldMapView,
    pub(super) minimap_hud: super::minimap::MinimapHud,
    pub(super) teleport: crate::hud::TeleportWindow,
    pub(super) sound: super::sound_panel::SoundPanel,
    /// The editing tools used most recently, newest first (the picker rail shows them).
    recent_tools: Vec<crate::paint::Tool>,
    splash: super::splash::Splash,
    notice: Option<(String, Instant)>,
    status: Option<String>,
}
impl UiRuntime {
    pub(super) fn new() -> Self {
        Self {
            #[cfg(not(target_arch = "wasm32"))]
            studio_quit_pending: false,
            prefab_workspace: super::prefab_studio::workspace::Workspace::default(),
            themes: super::theme_studio::ThemeStudio::default(),
            palette_search: String::new(),
            context_menu: None,
            settings_open: std::env::var_os("RUSTOSRS_OPEN_SETTINGS").is_some(),
            settings_search: String::new(),
            settings_focus_search: false,
            capturing_binding: None,
            script_open: false,
            plugins_open: false,
            script_text: String::new(),
            tool_draft: None,
            hud_rect: None,
            rail_rect: None,
            quick_open: std::env::var_os("RUSTOSRS_QUICK").map(|_| egui::pos2(520.0, 300.0)),
            world_map: super::worldmap::WorldMapView::new(),
            minimap_hud: super::minimap::MinimapHud::default(),
            teleport: crate::hud::TeleportWindow::new(),
            sound: super::sound_panel::SoundPanel::default(),
            recent_tools: Vec::new(),
            splash: super::splash::Splash::default(),
            notice: None,
            status: None,
        }
    }
    /// Remember an editing tool (not Select) as the most recent; at most `RECENT_TOOLS` are kept.
    pub(super) fn note_tool(&mut self, tool: crate::paint::Tool) {
        if tool == crate::paint::Tool::Select {
            return;
        }
        self.recent_tools.retain(|t| *t != tool);
        self.recent_tools.insert(0, tool);
        self.recent_tools.truncate(RECENT_TOOLS);
    }
    pub(super) fn recent_tools(&self) -> &[crate::paint::Tool] {
        &self.recent_tools
    }
    pub(super) fn splash_active(&self) -> bool {
        self.splash.is_active()
    }
    pub(super) fn splash_step(&mut self, dt: f32, target: f32) -> Option<(f32, f32)> {
        self.splash.step(dt, target)
    }
    /// Show the loading screen (a cache is being opened).
    pub(super) fn begin_loading(&mut self) {
        self.splash.begin();
    }
    pub(super) fn cancel_loading(&mut self) {
        self.splash.cancel();
    }
    pub(super) fn notify(&mut self, message: impl Into<String>) {
        self.notice = Some((message.into(), Instant::now()));
    }
    pub(super) fn notice(&self) -> Option<&str> {
        self.notice
            .as_ref()
            .filter(|(_, shown)| shown.elapsed().as_secs_f32() < 6.0)
            .map(|(message, _)| message.as_str())
    }
    pub(super) fn set_status(&mut self, status: String) {
        self.status = Some(status);
    }
    pub(super) fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }
    pub(super) fn modal_open(&self, external_modal: bool) -> bool {
        external_modal
            || self.settings_open
            || self.script_open
            || self.plugins_open
            || self.tool_draft.is_some()
            || self.teleport.open
            || self.world_map.open
    }
    pub(super) fn keys_blocked(&self, wants_keyboard: bool, external_modal: bool) -> bool {
        wants_keyboard || self.capturing_binding.is_some() || self.modal_open(external_modal)
    }
    pub(super) fn open_settings(&mut self) {
        self.settings_open = true;
    }
    pub(super) fn open_only(&mut self, window: Option<&str>) {
        self.settings_open = window == Some("settings");
        self.plugins_open = window == Some("plugins");
        self.script_open = window == Some("script");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn window_choices_and_external_modals_consistently_block_tools() {
        let mut ui = UiRuntime::new();
        ui.settings_open = false;
        ui.quick_open = None;
        for name in ["settings", "plugins", "script"] {
            ui.open_only(Some(name));
            assert!(ui.modal_open(false));
            assert!(ui.keys_blocked(false, false));
        }
        ui.open_only(None);
        assert!(!ui.modal_open(false));
        assert!(ui.keys_blocked(false, true));
        assert!(ui.keys_blocked(true, false));
        ui.capturing_binding = Some("project.save");
        assert!(ui.keys_blocked(false, false));
    }
}
