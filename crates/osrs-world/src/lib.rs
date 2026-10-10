//! Cache-to-scene world assembly.
//!
//! `osrs-world` composes `osrs-cache` (definitions, map squares) with `osrs-scene` (terrain,
//! placement, finalization) the way the reference client's scene loader does. It owns no new
//! semantics: each stage delegates to a pinned-source port in `osrs-scene`/`osrs-core` and adds
//! only orchestration and cache access.

mod animation;
mod definitions;
mod error;
mod extract;
mod loc_stage;
pub mod oracle_dump;
mod scene_builder;
mod streaming;
mod terrain_stage;
mod window;

pub use animation::{AnimatedContour, AnimatedInstance, AnimatedModel, AnimationSystem};
pub use definitions::{
    FloorTable, RegionMap, TEXTURE_SIZE, TextureImage, TextureTable, WorldDefinitions,
};
pub use error::WorldError;
pub use extract::{
    OwnedTiles, extract_animated_instances, extract_owned_geometry, extract_scene_geometry,
    texture_layers,
};
pub use loc_stage::{LocRenderable, LocStageOutput, LocStageStats, WorldLoc, place_window_locs};
pub use scene_builder::{WorldScene, build_world_scene};
pub use streaming::{
    REGION_WINDOW_MARGIN, RegionGeometry, RegionStreamer, StreamEvent, build_region_geometry,
    region_window,
};
pub use terrain_stage::{
    DEFAULT_BRIGHTNESS, LoadedWindow, TerrainPresentation, apply_terrain, load_window,
    load_window_terrain,
};
pub use window::SceneWindow;
