//! Terrain path generation: the planner of Terraini's path tool, ported.
//!
//! A path is a network of edges between nodes; the planner turns it into the overlay tiles to
//! paint (an overlay id, a shape and an orientation per tile). See `planner.rs` for the steps.

mod atlas;
mod grammar;
mod model;
mod noise;
mod planner;
mod resample;
mod router;
mod scene;
mod settings;
mod shapes;
pub mod walls;

pub use model::{
    CostGrid, Occupancy, PathEdge, PathEnvironment, PathNetwork, PathNode, PathPlan, PathPoint,
    PathWarning, Severity, TileBounds, TilePaint,
};
pub use planner::{PathPlanner, fnv};
pub use resample::smooth_and_resample as smooth_line;
pub use scene::SceneGrid;
pub use settings::{AutoConnectPolicy, MaterialVariant, ObstaclePolicy, PathProfile, PathSettings};
pub use shapes::{overlay_triangles, shape_triangles};

#[cfg(test)]
mod golden;
