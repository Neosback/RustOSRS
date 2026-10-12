//! Animation Smoothing: RuneLite's plugin of the same name as an editor plugin.
//!
//! The plugin only installs a filter; the client blends the current frame toward the next by
//! the elapsed share of the frame. The engine does that (`interpolate_legacy_frames`); this plugin
//! owns the switch and its persisted setting. Off by default, like RuneLite.

use crate::plugin::{EditorCommand, EditorPlugin, PluginContext};
use eframe::egui;

pub struct AnimationSmoothingPlugin {
    enabled: bool,
    announced: Option<bool>,
}

impl AnimationSmoothingPlugin {
    pub fn new() -> Self {
        Self {
            enabled: false,
            announced: None,
        }
    }
}

impl EditorPlugin for AnimationSmoothingPlugin {
    fn name(&self) -> &'static str {
        "animation_smoothing"
    }

    fn title(&self) -> &'static str {
        "Animation smoothing"
    }

    fn description(&self) -> &'static str {
        "Blends the frames of animated objects, like RuneLite's plugin"
    }

    fn set_enabled(&mut self, enabled: bool, cx: &mut PluginContext) {
        // Off: the game's own stepping. On: announce the saved choice again.
        self.announced = None;
        if !enabled {
            cx.send(EditorCommand::SetAnimationSmoothing(false));
        }
    }

    fn update(&mut self, cx: &mut PluginContext) {
        if self.announced != Some(self.enabled) {
            self.announced = Some(self.enabled);
            cx.send(EditorCommand::SetAnimationSmoothing(self.enabled));
        }
    }

    fn has_menu(&self) -> bool {
        true
    }

    fn menu(&mut self, ui: &mut egui::Ui, _cx: &mut PluginContext) {
        crate::theme::toggle(ui, &mut self.enabled, "Enabled");
    }

    fn has_properties(&self) -> bool {
        true
    }

    fn properties(&mut self, ui: &mut egui::Ui, _cx: &mut PluginContext) {
        crate::theme::toggle(ui, &mut self.enabled, "Blend frames of animated objects");
        ui.weak(
            "Matches RuneLite's Animation Smoothing (off by default). Wheels, sails and a few \
             other sequences are left alone, as in the plugin.",
        );
    }

    fn save_settings(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({ "enabled": self.enabled }))
    }

    fn load_settings(&mut self, value: &serde_json::Value) {
        self.enabled = value
            .get("enabled")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
    }
}
