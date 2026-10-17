//! Headless semantic map-editing domain for RustOSRS.
//!
//! This crate owns editor-facing map values and invariants. It deliberately has no
//! dependency on egui, rendering, GPU resources, cache codecs, scene workers, or
//! ECS/runtime state. Presentation and infrastructure layers adapt to and from the
//! types defined here.

mod building;
mod building_program;
mod building_realization;
mod building_refinement;
mod building_refinement_policy;
mod building_routing;
mod building_topology;
mod building_validation;
mod building_wfc;
mod document;
mod history;
mod object;
mod object_edit;
mod patch;
mod position;
mod query;
mod region;
mod scenario;
mod terrain;
mod workspace;

pub use building::{
    BuildingArchetype, BuildingConstraints, BuildingPlan, BuildingPlanError, BuildingRequest,
    BuildingStyle, OccupancyCell, OccupancyGrid, PortalKind, RectSite, RoomConnection, RoomId,
    RoomKind, RoomRect, RoomSpec, plan_basic_house,
};
pub use building_program::{
    BspCorridor, BspLayout, BspLayoutError, BspRoomPlacement, BuildingProgram, BuildingRoomRole,
    ProgramConnection, ProgramRoom, building_program, pack_building_program,
};
pub use building_realization::{
    BuildingArchitecture, BuildingArchitectureError, BuildingPalette, BuildingRealizationError,
    CardinalDirection, EdgeAsset, EdgeFeature, FloorMaterial, SemanticEdge, build_building_patch,
    derive_building_architecture,
};
pub use building_refinement::{
    FacadeRefinementError, FacadeWfcConfig, RefinedBuildingArchitecture,
    build_wfc_refined_building_patch, refine_exterior_walls_with_wfc,
};
pub use building_refinement_policy::{
    FacadeRefinementPolicy, build_wfc_building_variation_patch, facade_variation_seed,
    refine_exterior_walls_for_variation,
};
pub use building_routing::{ProgramRoute, ProgramRoutingError, route_program_connections};
pub use building_topology::{
    BspBuildingPlan, BspTopologyError, ProgramRoomRole, build_bsp_building_plan,
};
pub use building_validation::{
    BuildingDiagnostic, BuildingValidationReport, GenerationSnapshot, OccupiedFootprint,
    capture_generation_snapshot, validate_building_plan,
};
pub use building_wfc::{
    MAX_WFC_PATTERNS, PatternDomain, WfcCatalog, WfcCatalogError, WfcConstraint, WfcCoord,
    WfcPattern, WfcPatternId, WfcPatternSpec, WfcSocket, WfcSolution, WfcSolveError, solve_wfc,
};
pub use document::{MapDocument, PatchApplyError};
pub use history::{
    EditHistory, EditHistoryTransaction, HistoryDocument, HistoryTransactionError, PatchHistory,
    ReversiblePatch,
};
pub use object::{LocKind, ObjectOrientation, ObjectPlacement};
pub use object_edit::{ObjectEditError, build_object_patch};
pub use patch::{ChangeImpact, MapOp, MapPatch};
pub use position::{HeightVertexCoord, MapPosition};
pub use query::MapQuery;
pub use region::{MapRegionSnapshot, RegionSnapshotError};
pub use scenario::{
    ScenarioDocument, ScenarioPatch, ScenarioPlanError, SpawnChange, SpawnDraft, plan_add_spawn,
    plan_remove_spawns, plan_replace_scenario, plan_update_spawn,
};
pub use terrain::{OverlayRotation, OverlayShape, TerrainOverlay, TerrainTile};
pub use workspace::{WorkspaceDocument, WorkspaceHistory, WorkspacePatch};

mod brush;
pub mod height;
mod paint;
mod path;
mod path_height;

pub use brush::{BrushShape, footprint};
pub use height::{HeightCommitError, HeightStrokeSpec, build_terrain_height_patch};
pub use paint::{PaintCommitError, TerrainPaintIntent, build_terrain_paint_patch};
pub use path::{PathCommitError, TerrainPathIntent, build_terrain_path_patch};
pub use path_height::{
    PathHeightSpec, build_path_height_patch, distance_to_line, plan_path_height_smoothing,
    smoothing_vertices,
};
