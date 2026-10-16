//! How the NPC spawn layer is shown (see the NPC panel and Settings > Interface > NPCs).
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NpcSettings {
    /// Show the NPC spawns an OpenRune server places (loaded from the installation).
    pub spawns: bool,
    /// NPCs farther than this many tiles from the camera are not drawn.
    pub distance: u16,
    /// The most NPCs drawn at once (nearest first).
    pub max_drawn: u16,
    /// Outline each spawn's tile footprint.
    pub footprints: bool,
    /// Draw the NPC models (off: outlines only).
    pub models: bool,
}

impl Default for NpcSettings {
    fn default() -> Self {
        Self {
            spawns: false,
            distance: 64,
            max_drawn: 300,
            footprints: false,
            models: true,
        }
    }
}

impl NpcSettings {
    pub fn clamped(mut self) -> Self {
        self.distance = self.distance.clamp(8, 184);
        self.max_drawn = self.max_drawn.clamp(10, 2000);
        self
    }
}
