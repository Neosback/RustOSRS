//! Pure spline-to-tile overlay shape fitting and overlay triangle geometry.
//!
//! Ports the EikenB00m `PathOverlayFitter` against the canonical `osrs-scene` terrain tables so
//! both the interactive editor (`osrs-editor`) and the headless procedural/AI lab (`osrs-lab`)
//! share the exact same path fitting semantics.

mod fit;
mod java_order;
mod shapes;

pub use fit::{FittedTile, fit, fit_bounded, smooth};
pub use shapes::{overlay_triangles, shape_triangles};
