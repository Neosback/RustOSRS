//! The path tool's tile fitting and preview colours.
//!
//! The path tool follows the EikenB00m map editor's: click points, optionally smooth them into a
//! Catmull-Rom curve, and fit the real OSRS overlay shapes to a band of the chosen width
//! ([`fit()`]). There is deliberately no obstacle routing, per-point settings or map-specific
//! tuning. This module only plans; committing is `osrs-edit`'s `path` and `path_height`.

mod appearance;
mod fit;
mod java_order;
mod shapes;

pub use appearance::{PathAppearance, path_appearance, reference_palette};
#[cfg(test)]
pub use fit::fit;
pub use fit::{FittedTile, fit_bounded, smooth};
pub use shapes::{overlay_triangles, shape_triangles};
