//! Headless tool-only reconstruction environments. Targets remain private to each environment.
mod action;
mod env;
mod relocation;
mod score;
pub use relocation::relocation_task;
mod task;
pub use action::{Action, PathSmoothSpec};
pub use env::{Env, LabError, LegalActions, Observation, ObservationMode, StepResult};
pub use score::Score;
pub use task::{
    LabTask, PaletteEntry, Snapshot, generated_paths, generated_tavern, prefab_paths_task,
};
#[cfg(test)]
mod tests;
