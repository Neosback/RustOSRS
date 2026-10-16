//! Small value types of the NPC owner: what is armed, what to show, where loading stands.
use osrs_core::ids::NpcId;

/// The open cache the scenario is bound to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::app) struct SourceBinding {
    pub fingerprint: String,
    pub build: u32,
}

/// A pending viewport gesture armed from the panel.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(in crate::app) enum Arm {
    #[default]
    None,
    /// Place an NPC of this definition on the next tile click.
    Place { npc: NpcId, symbol: String },
    /// Send the selected actor to the next clicked tile (`route`: full routing, else step rules).
    Destination { route: bool },
}

/// How much of the spawn layer to show, from the settings.
#[derive(Debug, Clone, Copy)]
pub(in crate::app) struct NpcDisplay {
    /// Master switch: off hides every NPC and pauses the simulation.
    pub enabled: bool,
    pub distance_tiles: i32,
    pub max_drawn: usize,
    pub models: bool,
}

/// Where loading the OpenRune spawns stands (the panel shows this as plain words).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(in crate::app) enum AutoLoad {
    #[default]
    Idle,
    Reading,
    Parsing,
    Loaded,
    Failed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum OpenKind {
    Scenario,
}

/// A reviewed OpenRune export; files are written only after this is shown.
pub(in crate::app) struct ExportPreview {
    pub spawns_toml: String,
    pub behaviors_toml: String,
    pub error: Option<String>,
}
