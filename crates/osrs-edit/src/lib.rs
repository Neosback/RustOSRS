//! Headless semantic map-editing domain for RustOSRS.
//!
//! This crate owns editor-facing map values and invariants. It deliberately has no
//! dependency on egui, rendering, GPU resources, cache codecs, scene workers, or
//! ECS/runtime state. Presentation and infrastructure layers adapt to and from the
//! types defined here.

mod document;
mod history;
mod object;
mod object_edit;
mod patch;
mod position;
mod query;
mod region;
mod terrain;

pub use document::{MapDocument, PatchApplyError};
pub use history::{EditHistory, EditHistoryTransaction, HistoryTransactionError};
pub use object::{LocKind, ObjectOrientation, ObjectPlacement};
pub use object_edit::{ObjectEditError, build_object_patch};
pub use patch::{ChangeImpact, MapOp, MapPatch};
pub use position::{HeightVertexCoord, MapPosition};
pub use query::MapQuery;
pub use region::{MapRegionSnapshot, RegionSnapshotError};
pub use terrain::{OverlayRotation, OverlayShape, TerrainOverlay, TerrainTile};

mod brush;
pub mod height;
mod paint;
mod path;

pub use brush::{BrushShape, footprint};
pub use height::{HeightCommitError, HeightStrokeSpec, build_terrain_height_patch};
pub use paint::{PaintCommitError, TerrainPaintIntent, build_terrain_paint_patch};
pub use path::{PathCommitError, TerrainPathIntent, build_terrain_path_patch};
