//! Native project persistence and replacement state machine. UI code emits choices; this
//! module owns saved revision, pending diffs and save/quit/replacement transitions.
mod save;
#[cfg(test)]
mod tests;
use super::ProjectRuntime;
use crate::app::edit_runtime::EditRuntime;
use crate::session::RegionDiff;
use osrs_cache::source::CacheSource;
use osrs_core::coords::RegionCoord;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::mpsc::Receiver,
};
#[derive(Default)]
pub(super) struct ProjectPersistence {
    /// Diffs from an opened project whose region has not been applied and projected yet.
    pending: BTreeMap<RegionCoord, RegionDiff>,
    /// The file Save writes to.
    path: Option<PathBuf>,
    save: Option<Receiver<Result<(PathBuf, usize), String>>>,
    /// `EditorSession::edit_revision` as of the last successful save or project application.
    saved_edit_revision: u64,
    /// The edit revision the running save snapshotted.
    saving_revision: u64,
    /// The unsaved-changes dialog is showing for application close.
    quit_prompt: bool,
    /// Close the window once the running save finishes.
    close_when_saved: bool,
    /// The user chose to quit without saving.
    discard_confirmed: bool,
    /// A cache/project replacement waiting for Save / Discard / Cancel.
    transition: Option<LifecycleTransition>,
    transition_prompt: bool,
    /// Execute `transition` after the save currently requested by the transition prompt succeeds.
    transition_after_save: bool,
}

pub(in crate::app) enum LifecycleTransition {
    Source {
        source: CacheSource,
        label: PathBuf,
    },
    Project {
        path: PathBuf,
        diffs: Vec<RegionDiff>,
    },
}

impl LifecycleTransition {
    fn description(&self) -> &'static str {
        match self {
            Self::Source { .. } => "open another cache",
            Self::Project { .. } => "open another project",
        }
    }
}

#[derive(Clone, Copy)]
pub(in crate::app) enum LifecycleChoice {
    Save,
    Discard,
    Cancel,
}

impl ProjectRuntime {
    pub(in crate::app) fn has_pending_exports(&self) -> bool {
        !self.persistence.pending.is_empty()
    }
    pub(in crate::app) fn export_pending(&self) -> Vec<RegionDiff> {
        self.persistence.pending.values().cloned().collect()
    }
    pub(in crate::app) fn is_unsaved(&self, revision: u64) -> bool {
        revision != self.persistence.saved_edit_revision
    }
    pub(in crate::app) fn has_changes(&self, dirty: bool) -> bool {
        dirty || self.has_pending_exports()
    }
    pub(in crate::app) fn save_running(&self) -> bool {
        self.persistence.save.is_some()
    }
    pub(in crate::app) fn project_path(&self) -> Option<&Path> {
        self.persistence.path.as_deref()
    }
    pub(in crate::app) fn reset_for_source_change(&mut self) {
        debug_assert!(!self.save_running());
        self.persistence = ProjectPersistence::default();
    }
    pub(in crate::app) fn prepare_replacement(
        &mut self,
        session: &mut EditRuntime,
        path: PathBuf,
        diffs: Vec<RegionDiff>,
    ) {
        self.reset_for_source_change();
        session.reset();
        self.persistence.path = Some(path);
        self.persistence.pending = diffs.into_iter().map(|diff| (diff.region, diff)).collect();
        self.persistence.saved_edit_revision = session.edit_revision();
    }
    pub(in crate::app) fn apply_pending_semantics(
        &mut self,
        session: &mut EditRuntime,
        region: RegionCoord,
    ) -> Result<Option<RegionDiff>, osrs_edit::PatchApplyError> {
        let Some(diff) = self.persistence.pending.get(&region).cloned() else {
            return Ok(None);
        };
        let was_saved = !self.is_unsaved(session.edit_revision());
        let changed = session.apply_untracked(&diff.to_patch(session.document()))?;
        if changed && was_saved {
            self.persistence.saved_edit_revision = session.edit_revision();
        }
        Ok(Some(diff))
    }
    /// Only successful projection consumes a pending project region, so failed builds can retry.
    pub(in crate::app) fn complete_region_projection(&mut self, region: RegionCoord) {
        self.persistence.pending.remove(&region);
    }
    pub(in crate::app) fn stage_transition(
        &mut self,
        transition: LifecycleTransition,
        unsaved: bool,
    ) -> Option<LifecycleTransition> {
        if unsaved {
            self.persistence.transition = Some(transition);
            self.persistence.transition_prompt = true;
            None
        } else {
            Some(transition)
        }
    }
    pub(in crate::app) fn ready_transition(
        &mut self,
        unsaved: bool,
    ) -> Option<LifecycleTransition> {
        if self.persistence.transition_after_save && !self.save_running() {
            self.persistence.transition_after_save = false;
            self.persistence.transition_prompt = unsaved;
            if !unsaved {
                return self.persistence.transition.take();
            }
        }
        None
    }
    pub(in crate::app) fn transition_description(&self) -> Option<&'static str> {
        self.persistence.transition_prompt.then(|| {
            self.persistence
                .transition
                .as_ref()
                .map(LifecycleTransition::description)
                .unwrap_or("replace the current session")
        })
    }
    pub(in crate::app) fn choose_transition(
        &mut self,
        choice: LifecycleChoice,
    ) -> Option<LifecycleTransition> {
        self.persistence.transition_prompt = false;
        match choice {
            LifecycleChoice::Discard => self.persistence.transition.take(),
            LifecycleChoice::Cancel => {
                self.persistence.transition_after_save = false;
                self.persistence.transition = None;
                None
            }
            LifecycleChoice::Save => None,
        }
    }
    pub(in crate::app) fn wait_transition_save(&mut self, started: bool) {
        self.persistence.transition_after_save = started;
        self.persistence.transition_prompt = !started;
    }
    pub(in crate::app) fn discard_confirmed(&self) -> bool {
        self.persistence.discard_confirmed
    }
    pub(in crate::app) fn request_quit(&mut self) {
        self.choose_transition(LifecycleChoice::Cancel);
        self.persistence.quit_prompt = true;
    }
    pub(in crate::app) fn quit_prompt(&self) -> bool {
        self.persistence.quit_prompt
    }
    pub(in crate::app) fn choose_quit(&mut self, choice: LifecycleChoice) -> bool {
        self.persistence.quit_prompt = false;
        if matches!(choice, LifecycleChoice::Discard) {
            self.persistence.discard_confirmed = true;
            true
        } else {
            false
        }
    }
    pub(in crate::app) fn wait_quit_save(&mut self, started: bool) {
        self.persistence.close_when_saved = started;
    }
    pub(in crate::app) fn ready_to_close(&mut self, unsaved: bool) -> bool {
        if self.persistence.close_when_saved && !self.save_running() {
            self.persistence.close_when_saved = false;
            if !unsaved {
                self.persistence.discard_confirmed = true;
                return true;
            }
        }
        false
    }
}
