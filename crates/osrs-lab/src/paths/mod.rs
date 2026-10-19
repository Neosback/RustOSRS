//! Learning to lay paths: lessons (real map rectangles with their paths), what a learner sees of
//! a place, the network that says where paths go, the router that draws them, and the checks.

mod attempt;
mod eval;
mod grid;
mod learn;
mod lesson;
mod model;
mod net;
mod route;
mod sense;
mod task;

pub use attempt::{Attempt, Mode, Why};
pub use eval::{Draft, endpoints_of};
pub use learn::{PathProgress, PathTrainRequest, train_paths};
pub use lesson::{LESSON_FORMAT, ObjectEntry, PathLesson};
pub use model::{PATH_MODEL_FORMAT, PathMetrics, PathModel, Reason, Score};
pub use route::{Tile, pieces, waypoints};
pub use sense::{FEATURE_INFO, Scene};
pub use task::{lay_actions, path_task};

#[cfg(test)]
mod tests;
