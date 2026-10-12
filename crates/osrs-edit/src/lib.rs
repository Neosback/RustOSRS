//! Headless semantic map-editing domain for RustOSRS.
//!
//! This crate owns editor-facing map values and invariants. It deliberately has no
//! dependency on egui, rendering, GPU resources, cache codecs, scene workers, or
//! ECS/runtime state. Presentation and infrastructure layers adapt to and from the
//! types defined here.

mod object;
mod position;
mod terrain;

pub use object::{LocKind, ObjectOrientation, ObjectPlacement};
pub use position::{HeightVertexCoord, MapPosition};
pub use terrain::{OverlayRotation, OverlayShape, TerrainOverlay, TerrainTile};
