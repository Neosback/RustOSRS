//! Cache-to-scene world assembly.
//!
//! `osrs-world` composes `osrs-cache` (definitions, map squares) with `osrs-scene` (terrain,
//! placement, finalization) the way the reference client's scene loader does. It owns no new
//! semantics: each stage delegates to a pinned-source port in `osrs-scene`/`osrs-core` and adds
//! only orchestration and cache access.

mod animation;
mod animation_worker;
mod assets;
mod definitions;
mod error;
mod extract;
mod floor_palette;
pub mod hd;
mod info;
mod loc_edits;
mod loc_stage;
pub mod minimap;
mod object_catalog;
pub mod oracle_dump;
mod scene_builder;
mod source_squares;
mod streaming;
mod terrain_edits;
mod terrain_stage;
mod window;
pub mod worldmap;

pub use animation::{AnimatedContour, AnimatedInstance, AnimatedModel, AnimationSystem};
pub use animation_worker::{AnimationUpdate, AnimationWorker};
pub use assets::{AssetEvent, AssetWorker, SpriteImage, WorldMapRegionImage};
pub use definitions::{
    FloorTable, RegionMap, TEXTURE_SIZE, TextureImage, TextureTable, WorldDefinitions,
    region_tiles_from_decoded,
};
pub use error::WorldError;
pub use extract::{
    OwnedTiles, extract_animated_instances, extract_owned_geometry, extract_scene_geometry,
    texture_layers,
};
pub use floor_palette::{FloorPalette, OverlaySwatch, UnderlaySwatch, load_floor_palette};
pub use info::{
    LocInfo, RegionInfo, SlotInfo, SurfaceKind, TileInfo, extract_region_info,
    loc_outline_triangles,
};
pub use loc_edits::{LocEdit, LocEdits, LocSpec};
pub use loc_stage::{LocRenderable, LocStageOutput, LocStageStats, WorldLoc, place_window_locs};
pub use object_catalog::{ObjectCatalog, ObjectEntry, SCENERY_LOC_TYPE, load_object_catalog};
pub use scene_builder::{WorldScene, build_world_scene};
pub use source_squares::{SourceSquare, SourceSquares};
pub use streaming::{
    HdRegionStats, REGION_WINDOW_MARGIN, RegionGeometry, RegionRenderPath, RegionStreamer,
    StreamEvent, build_region_geometry, build_region_geometry_with, region_window,
};
pub use terrain_edits::{TerrainEdits, TileEdit};
pub use terrain_stage::{
    DEFAULT_BRIGHTNESS, LoadedWindow, TerrainPresentation, apply_terrain, load_window,
    load_window_terrain,
};
pub use window::SceneWindow;
