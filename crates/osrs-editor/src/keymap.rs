//! Rebindable keyboard shortcuts.
//!
//! Every shortcut is an *action* with a stable id (`"camera.forward"`, `"teleport.open"`, and
//! plugin actions such as `"grid.toggle"`), a label and a default [`Binding`]. Overrides are
//! persisted in the settings file as `{"action id": "Ctrl+Shift+G"}`; anything not overridden
//! uses the default, so new actions never disturb a saved keymap. The Keyboard Shortcuts window
//! (Edit menu) edits the overrides and rejects nothing, but shows conflicts.

use eframe::egui;
use std::collections::BTreeMap;

/// A key plus modifiers. `command` is Cmd on macOS and Ctrl elsewhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Binding {
    pub key: egui::Key,
    pub command: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Binding {
    pub const fn key(key: egui::Key) -> Self {
        Self {
            key,
            command: false,
            shift: false,
            alt: false,
        }
    }

    /// `"Ctrl+Shift+G"`-style text (the persisted form).
    pub fn describe(&self) -> String {
        let mut text = String::new();
        if self.command {
            text.push_str("Cmd+");
        }
        if self.alt {
            text.push_str("Alt+");
        }
        if self.shift {
            text.push_str("Shift+");
        }
        text.push_str(self.key.name());
        text
    }

    pub fn parse(text: &str) -> Option<Self> {
        let mut binding = Self::key(egui::Key::A);
        let mut parts = text.split('+').peekable();
        let mut key = None;
        while let Some(part) = parts.next() {
            if parts.peek().is_some() {
                match part {
                    "Cmd" | "Ctrl" | "Command" => binding.command = true,
                    "Shift" => binding.shift = true,
                    "Alt" => binding.alt = true,
                    _ => return None,
                }
            } else {
                key = egui::Key::from_name(part);
            }
        }
        binding.key = key?;
        Some(binding)
    }

    /// Capture a binding from this frame's input (first non-modifier key pressed).
    pub fn capture(input: &egui::InputState) -> Option<Self> {
        input.events.iter().find_map(|event| match event {
            egui::Event::Key {
                key,
                pressed: true,
                repeat: false,
                modifiers,
                ..
            } => Some(Self {
                key: *key,
                command: modifiers.command,
                shift: modifiers.shift,
                alt: modifiers.alt,
            }),
            _ => None,
        })
    }

    /// Pressed this frame with exactly these modifiers.
    pub fn pressed(&self, input: &egui::InputState) -> bool {
        input.key_pressed(self.key)
            && input.modifiers.command == self.command
            && input.modifiers.shift == self.shift
            && input.modifiers.alt == self.alt
    }

    /// Held, ignoring Shift (movement keys use Shift as a speed boost unless it is bound).
    pub fn held(&self, input: &egui::InputState) -> bool {
        input.key_down(self.key)
            && input.modifiers.command == self.command
            && input.modifiers.alt == self.alt
            && (self.shift == input.modifiers.shift || !self.shift)
    }
}

/// One bindable action.
#[derive(Debug, Clone, Copy)]
pub struct ActionDef {
    pub id: &'static str,
    pub label: &'static str,
    pub category: &'static str,
    pub default: Binding,
}

impl ActionDef {
    pub const fn new(
        id: &'static str,
        label: &'static str,
        category: &'static str,
        default: Binding,
    ) -> Self {
        Self {
            id,
            label,
            category,
            default,
        }
    }
}

use egui::Key;

/// Actions of the editor itself (plugins add their own).
pub const CORE_ACTIONS: &[ActionDef] = &[
    ActionDef::new(
        "camera.forward",
        "Move forward",
        "Camera",
        Binding::key(Key::W),
    ),
    ActionDef::new("camera.back", "Move back", "Camera", Binding::key(Key::S)),
    ActionDef::new("camera.left", "Move left", "Camera", Binding::key(Key::A)),
    ActionDef::new("camera.right", "Move right", "Camera", Binding::key(Key::D)),
    ActionDef::new("camera.up", "Move up", "Camera", Binding::key(Key::E)),
    ActionDef::new("camera.down", "Move down", "Camera", Binding::key(Key::Q)),
    ActionDef::new(
        "camera.mode",
        "Switch fly / orbit",
        "Camera",
        Binding::key(Key::C),
    ),
    ActionDef::new(
        "camera.focus",
        "Focus selection",
        "Camera",
        Binding::key(Key::F),
    ),
    ActionDef::new("plane.0", "Show plane 0", "View", Binding::key(Key::Num1)),
    ActionDef::new("plane.1", "Show plane 1", "View", Binding::key(Key::Num2)),
    ActionDef::new("plane.2", "Show plane 2", "View", Binding::key(Key::Num3)),
    ActionDef::new("plane.3", "Show plane 3", "View", Binding::key(Key::Num4)),
    ActionDef::new(
        "panel.properties",
        "Toggle side panel",
        "View",
        Binding::key(Key::N),
    ),
    ActionDef::new(
        "settings.open",
        "Settings...",
        "Edit",
        Binding {
            key: Key::Comma,
            command: true,
            shift: false,
            alt: false,
        },
    ),
    ActionDef::new(
        "contextmenu.style",
        "Switch editor / game context menu",
        "View",
        Binding {
            key: Key::M,
            command: false,
            shift: false,
            alt: true,
        },
    ),
    ActionDef::new("teleport.open", "Teleport...", "Go", Binding::key(Key::T)),
    ActionDef::new("worldmap.toggle", "World map", "Go", Binding::key(Key::M)),
    ActionDef::new(
        "selection.clear",
        "Clear selection",
        "Edit",
        Binding::key(Key::Escape),
    ),
];

/// Registered actions and the user's overrides.
pub struct Keymap {
    actions: Vec<ActionDef>,
    overrides: BTreeMap<String, Binding>,
}

impl Keymap {
    pub fn new() -> Self {
        Self {
            actions: CORE_ACTIONS.to_vec(),
            overrides: BTreeMap::new(),
        }
    }

    /// Add plugin actions (ids already registered are ignored).
    pub fn register(&mut self, actions: &[ActionDef]) {
        for action in actions {
            if !self.actions.iter().any(|known| known.id == action.id) {
                self.actions.push(*action);
            }
        }
    }

    pub fn actions(&self) -> &[ActionDef] {
        &self.actions
    }

    pub fn binding(&self, id: &str) -> Option<Binding> {
        self.overrides
            .get(id)
            .copied()
            .or_else(|| self.actions.iter().find(|a| a.id == id).map(|a| a.default))
    }

    /// The binding as text (`"Cmd+,"`), empty when unbound.
    pub fn describe(&self, id: &str) -> String {
        self.binding(id)
            .map(|binding| binding.describe())
            .unwrap_or_default()
    }

    pub fn is_overridden(&self, id: &str) -> bool {
        self.overrides.contains_key(id)
    }

    pub fn pressed(&self, input: &egui::InputState, id: &str) -> bool {
        self.binding(id).is_some_and(|b| b.pressed(input))
    }

    pub fn held(&self, input: &egui::InputState, id: &str) -> bool {
        self.binding(id).is_some_and(|b| b.held(input))
    }

    pub fn set(&mut self, id: &str, binding: Binding) {
        let default = self.actions.iter().find(|a| a.id == id).map(|a| a.default);
        if default == Some(binding) {
            self.overrides.remove(id);
        } else {
            self.overrides.insert(id.to_owned(), binding);
        }
    }

    pub fn reset(&mut self, id: &str) {
        self.overrides.remove(id);
    }

    pub fn reset_all(&mut self) {
        self.overrides.clear();
    }

    /// Other actions already using `binding`.
    pub fn conflicts(&self, id: &str, binding: Binding) -> Vec<&'static str> {
        self.actions
            .iter()
            .filter(|a| a.id != id && self.binding(a.id) == Some(binding))
            .map(|a| a.label)
            .collect()
    }

    /// Overrides as persisted text.
    pub fn to_saved(&self) -> BTreeMap<String, String> {
        self.overrides
            .iter()
            .map(|(id, binding)| (id.clone(), binding.describe()))
            .collect()
    }

    pub fn load(&mut self, saved: &BTreeMap<String, String>) {
        self.overrides = saved
            .iter()
            .filter_map(|(id, text)| Some((id.clone(), Binding::parse(text)?)))
            .collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bindings_round_trip_through_text() {
        let binding = Binding {
            key: Key::G,
            command: true,
            shift: true,
            alt: false,
        };
        assert_eq!(Binding::parse(&binding.describe()), Some(binding));
        assert_eq!(Binding::parse("W"), Some(Binding::key(Key::W)));
        assert_eq!(Binding::parse("Nonsense+W"), None);
    }

    #[test]
    fn overrides_fall_back_to_defaults_and_report_conflicts() {
        let mut keymap = Keymap::new();
        assert_eq!(keymap.binding("camera.forward"), Some(Binding::key(Key::W)));
        keymap.set("camera.forward", Binding::key(Key::I));
        assert_eq!(keymap.binding("camera.forward"), Some(Binding::key(Key::I)));
        // Setting a default back removes the override.
        keymap.set("camera.forward", Binding::key(Key::W));
        assert!(!keymap.is_overridden("camera.forward"));
        // Binding T (teleport) to another action reports the conflict.
        assert_eq!(
            keymap.conflicts("camera.focus", Binding::key(Key::T)),
            vec!["Teleport..."]
        );
    }
}
