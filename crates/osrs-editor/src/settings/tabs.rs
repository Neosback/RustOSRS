//! The dock's rail categories and the tabs inside the View category.
use serde::{Deserialize, Serialize};

/// Categories on the dock's icon rail; each swaps the dock panel's contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RailTab {
    /// The selected object and the hovered tile. (Plugins used to be a rail tab; it is a menu-bar
    /// window now, so old settings files naming it land here.)
    #[default]
    #[serde(alias = "Plugins")]
    Inspect,
    /// Camera, world, interface and graphics options used while working (tabs inside).
    View,
    /// Retired as a button (it is a tab of View); old settings files may still name it.
    Graphics,
    /// The objects that make sounds in the current map square.
    Sound,
    /// A mock player: stats, look, worn items and game variables to play with.
    Player,
    /// Offline NPC scenarios: import, place, edit and playtest.
    Npc,
}

impl RailTab {
    pub const ALL: [Self; 5] = [
        Self::Inspect,
        Self::View,
        Self::Sound,
        Self::Player,
        Self::Npc,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Inspect => "Inspector",
            Self::View => "View",
            Self::Graphics => "Graphics",
            Self::Sound => "Sound",
            Self::Player => "Player",
            Self::Npc => "NPCs",
        }
    }
}

/// Tabs inside the View category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ViewTab {
    ThemeStudio,
    /// Camera, world and interface.
    #[default]
    Scene,
    /// Render profile, lighting, fog and shadows.
    Graphics,
    /// What the viewport shows: HUDs, indicators, the grid, map icons.
    Overlays,
}

impl ViewTab {
    pub const ALL: [Self; 4] = [
        Self::Scene,
        Self::Graphics,
        Self::Overlays,
        Self::ThemeStudio,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::ThemeStudio => "Theme Studio",
            Self::Scene => "Scene",
            Self::Graphics => "Graphics",
            Self::Overlays => "Overlays",
        }
    }
}
