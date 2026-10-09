//! Deterministic semantic OSRS scene construction and mutation boundary.
//!
//! Scene code consumes canonical semantic inputs and must not import concrete cache
//! implementation types. M6 begins the real semantic scene with terrain and bounded
//! storage while keeping renderer/editor policy outside this crate.

pub mod scene;
pub mod terrain;

pub use scene::{SceneGrid, SceneGridError, SemanticTile};
pub use terrain::{
    FlatTerrainSurface, ShapedTerrainInput, ShapedTerrainSurface, TerrainBuildError,
    TerrainColorSource, TerrainCorner, TerrainCorners, TerrainFace, TerrainSurface, TerrainVertex,
};
