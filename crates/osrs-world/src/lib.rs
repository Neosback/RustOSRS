//! Cache-to-scene world assembly.
//!
//! `osrs-world` composes `osrs-cache` (definitions, map squares) with `osrs-scene` (terrain,
//! placement, finalization) the way the reference client's scene loader does. It owns no new
//! semantics: each stage delegates to a pinned-source port in `osrs-scene`/`osrs-core` and adds
//! only orchestration and cache access.

mod definitions;
mod error;
mod extract;
mod loc_stage;
pub mod oracle_dump;
mod scene_builder;
mod terrain_stage;
mod window;

pub use definitions::{FloorTable, RegionMap, WorldDefinitions};
pub use error::WorldError;
pub use extract::extract_scene_geometry;
pub use loc_stage::{LocRenderable, LocStageOutput, LocStageStats, WorldLoc, place_window_locs};
pub use scene_builder::{WorldScene, build_world_scene};
pub use terrain_stage::{
    DEFAULT_BRIGHTNESS, LoadedWindow, TerrainPresentation, apply_terrain, load_window,
    load_window_terrain,
};
pub use window::SceneWindow;
