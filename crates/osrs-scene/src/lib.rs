//! Deterministic semantic OSRS scene construction and mutation boundary.
//!
//! Scene code consumes canonical semantic inputs and must not import concrete cache
//! implementation types. M6 begins the real semantic scene with terrain, bounded
//! storage, and exact location-placement planning while keeping renderer/editor
//! policy outside this crate.

pub mod placement;
pub mod scene;
pub mod terrain;

pub use placement::{
    BoundaryPlan, CARDINAL_OFFSET_X, CARDINAL_OFFSET_Z, DIAGONAL_OFFSET_X, DIAGONAL_OFFSET_Z,
    DIAGONAL_WALL_FLAGS, FloorDecorationPlan, Footprint, GameObjectPlan, ModelRequest,
    PlacementError, PlacementInput, PlacementKind, PlacementPlan, STRAIGHT_WALL_FLAGS, SceneLayer,
    WallDecorationPlan, plan_placement,
};
pub use scene::{SceneGrid, SceneGridError, SemanticTile};
pub use terrain::{
    FlatTerrainSurface, ShapedTerrainInput, ShapedTerrainSurface, TerrainBuildError,
    TerrainColorSource, TerrainCorner, TerrainCorners, TerrainFace, TerrainSurface, TerrainVertex,
};
