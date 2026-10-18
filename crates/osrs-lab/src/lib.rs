//! Headless tool-only reconstruction environments. Targets remain private to each environment.
mod action;
mod critic;
mod model;
pub use model::{
    Built, Candidate, MODEL_FORMAT, MODEL_VERSION, Model, ModelMetrics, ModelName, Structure,
    StructureBucket, TrainedOn, build_with_model, features_of, fingerprint,
};
mod train;
pub use critic::{Critic, CriticMetrics, train_critic};
pub use train::{Corpus, Progress, TrainRequest, train_model};
mod env;
mod generate;
pub use generate::{
    Brief, Generated, Verdict, archetype_of, census_buildings, generate, generate_named,
    generated_prefab, judge_snapshot, kit_objects,
};
mod reconstruction;
mod relocation;
pub use reconstruction::reconstruction_task;
mod score;
mod style;
pub use relocation::relocation_task;
pub use style::{StyleReport, learn_style, prefab_buildings};
mod task;
pub use action::{Action, PathSmoothSpec};
pub use env::{Env, LabError, LegalActions, Observation, ObservationMode, StepResult};
pub use score::Score;
pub use task::{
    LabTask, PaletteEntry, Snapshot, generated_paths, generated_tavern, prefab_paths_task,
};
#[cfg(test)]
mod tests;

#[cfg(test)]
mod reconstruction_tests;
