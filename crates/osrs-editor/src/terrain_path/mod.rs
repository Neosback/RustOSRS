//! The path tool's tile fitting and preview colours.
//!
//! The path tool follows the EikenB00m map editor's: click points, optionally smooth them into a
//! Catmull-Rom curve, and fit the real OSRS overlay shapes to a band of the chosen width
//! ([`fit()`]). There is deliberately no obstacle routing, per-point settings or map-specific
//! tuning. This module only plans; committing is `osrs-edit`'s `path` and `path_height`.

mod appearance;

pub use appearance::{PathAppearance, path_appearance, reference_palette};
#[cfg(test)]
pub use osrs_scene::terrain_path::fit;
pub use osrs_scene::terrain_path::{
    FittedTile, fit_bounded, overlay_triangles, shape_triangles, smooth,
};
