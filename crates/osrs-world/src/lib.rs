//! Cache-to-scene world assembly.
//!
//! `osrs-world` composes `osrs-cache` (definitions, map squares) with `osrs-scene` (terrain,
//! placement, finalization) the way the reference client's scene loader does. It owns no new
//! semantics: each stage delegates to a pinned-source port in `osrs-scene`/`osrs-core` and adds
//! only orchestration and cache access.

mod definitions;
mod error;
pub mod oracle_dump;
mod terrain_stage;
mod window;

pub use definitions::{FloorTable, RegionMap, WorldDefinitions};
pub use error::WorldError;
pub use terrain_stage::{
    DEFAULT_BRIGHTNESS, TerrainPresentation, TerrainScene, build_terrain_scene, load_window_terrain,
};
pub use window::SceneWindow;
