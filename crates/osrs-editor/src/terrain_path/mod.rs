//! Deterministic terrain-path overlay fitting for the Spline tool.
//!
//! The current path tool intentionally has no obstacle routing, network pathfinding, walls, noise,
//! or auto-connect behavior. It smooths the user's control polyline and fits the exact OSRS overlay
//! shape geometry to that band.

mod fitter;
mod shapes;

pub use fitter::{FittedTile, fit_path_overlay, smooth_catmull_rom};
pub use shapes::{overlay_triangles, shape_triangles};
