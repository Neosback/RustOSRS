//! Renderer-owned CPU extraction and headless structural rendering core.
//!
//! M10 keeps this crate free of wgpu/device state. It consumes authoritative
//! semantic data, produces disposable immutable render snapshots, and preserves
//! enough provenance/metadata for exact structural verification before M11.

mod coordinates;
mod generation;
mod mesh;
mod snapshot;

pub use coordinates::{RenderCoordinateError, RenderOrigin, RenderPoint};
pub use generation::SemanticGeneration;
pub use mesh::{RenderExtractionError, RenderMesh, RenderPlacement};
pub use snapshot::RenderSnapshot;
