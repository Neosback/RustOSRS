//! The training process is native-only; the task and action semantics remain portable.
use osrs_lab::{Action, LabTask};
pub(super) struct Demonstration {
    pub(super) actions: Vec<Action>,
    pub(super) score: f64,
    pub(super) directory: std::path::PathBuf,
}
pub(super) struct Job {
    pub(super) phase: &'static str,
}
impl Job {
    pub(super) fn start(_task: LabTask, _seed: u64) -> Result<Self, String> {
        Err("Local demonstration jobs require the desktop app".into())
    }
    pub(super) fn poll(&mut self) -> Option<Result<Demonstration, String>> {
        None
    }
}
