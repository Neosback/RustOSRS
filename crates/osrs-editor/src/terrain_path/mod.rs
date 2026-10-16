//! Deterministic terrain-path overlay fitting for the Spline tool.
//!
//! The path tool intentionally has no obstacle routing, network pathfinding, walls, noise, or
//! auto-connect behavior. It smooths the user's control points (each with its own width and an
//! optional corner) and fits the exact OSRS overlay shape geometry to that band, preferring what
//! the real map does (`census`).

mod appearance;
mod census;
mod census_data;
mod fitter;
#[cfg(test)]
mod quality;
mod shapes;
mod smooth;

pub use appearance::{PathAppearance, path_appearance, reference_palette};
pub use census::is_path_overlay;
pub use fitter::{Fit, FitOptions, FittedTile, fit_path_overlay};
pub use shapes::{overlay_triangles, shape_triangles};
pub use smooth::{ControlPoint, PathSample, smooth_path};
