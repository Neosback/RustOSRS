//! Deterministic semantic OSRS scene construction and mutation boundary.
//!
//! Scene code consumes canonical semantic inputs and must not import concrete cache
//! implementation types. M6 builds the real semantic scene with terrain, bounded
//! storage, exact location placement, plane relinking, and queryable scene state
//! while keeping renderer/editor policy outside this crate.

pub mod dynamic_placement;
pub mod normal_finalization;
pub mod pending_replacement;
pub mod placement;
pub mod placement_height;
pub mod planes;
pub mod reference_finalization;
pub mod scene;
pub mod side_effects;
pub mod terrain;
pub mod terrain_contract;

pub use dynamic_placement::{
    DynamicPlacementError, DynamicPlacementInput, ObjectDefinitionLookup, ResolvedDynamicPlacement,
    resolve_dynamic_placement,
};
pub use normal_finalization::{
    BoundaryModelData, FloorDecorationModelData, GameObjectModelData, SceneModelDataGrid,
    SceneModelDataId, SceneNormalError, SceneNormalMergeReport,
};
pub use pending_replacement::{
    PendingInsertion, PendingRemoval, PendingReplacementPlan, PendingReplacementPlanError,
    PendingReplacementReport, PendingSceneCategory, PendingSceneMutation,
    apply_pending_replacement, plan_pending_replacement,
};
pub use placement::{
    BoundaryPlan, CARDINAL_OFFSET_X, CARDINAL_OFFSET_Z, DIAGONAL_OFFSET_X, DIAGONAL_OFFSET_Z,
    DIAGONAL_WALL_FLAGS, FloorDecorationPlan, Footprint, GameObjectPlan, ModelRequest,
    PlacementError, PlacementInput, PlacementKind, PlacementPlan, STRAIGHT_WALL_FLAGS, SceneLayer,
    WallDecorationPlan, plan_placement,
};
pub use placement_height::{PlacementHeightError, PlacementHeightInput, sample_placement_height};
pub use planes::{PlacementPlanes, collision_plane};
pub use reference_finalization::{SceneReferenceFinalizationError, SceneReferenceFinalizer};
pub use scene::{SceneGameObject, SceneGrid, SceneGridError, ScenePlacedLoc, SemanticTile};
pub use side_effects::{
    CollisionSideEffect, DefinitionSideEffectInputs, SceneSideEffectPlan,
    plan_definition_side_effects, plan_side_effects,
};
pub use terrain::{
    FlatTerrainSurface, ShapedTerrainInput, ShapedTerrainSurface, TerrainBuildError,
    TerrainColorSource, TerrainCorner, TerrainCorners, TerrainFace, TerrainSurface, TerrainVertex,
};
pub use terrain_contract::{
    REFERENCE_TERRAIN_SKIP_COLOR, flat_paint_is_reference_skipped, shaped_face_is_reference_skipped,
};
