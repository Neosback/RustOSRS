//! File > Open / Save project: persist an editing session's changes between runs.
//!
//! A project file stores only the differences against the source cache (see
//! `session::project_file`). Saving diffs the authoritative document against freshly read source
//! squares on a worker thread. Opening validates the file against the open cache's profile, then
//! restarts that cache from its pristine source before applying each project region as it streams.
//! This keeps Open Project replacement semantics distinct from a future explicit merge/import.

use super::EditorApp;
use super::editing::TileKey;
use super::object_tool::{
    ObjectHistoryRoute, build_object_history_projection, spec_from_placement,
};
use crate::session::{PROJECT_FORMAT, ProjectFile, RegionDiff, diff_region};
use eframe::egui;
use osrs_cache::source::CacheSource;
use osrs_core::coords::RegionCoord;
use osrs_edit::MapDocument;
use osrs_world::SourceSquares;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError};
use web_time::Instant;

enum LifecycleTransition {
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

#[derive(Default)]
pub(super) struct ProjectState {
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

impl ProjectState {
    /// Clear every piece of state whose meaning is tied to the current cache/source.
    ///
    /// Source replacement is deliberately blocked while a save is running, so dropping the
    /// receiver here would be a lifecycle bug rather than a cancellation mechanism.
    fn reset_for_source_change(&mut self) {
        debug_assert!(self.save.is_none());
        *self = Self::default();
    }
}

/// Build the project file contents for `document`, keeping unapplied `pending` diffs for regions
/// that are not being re-diffed. Pure apart from reading the source cache.
fn build_project(
    source: CacheSource,
    document: &MapDocument,
    dirty: &[RegionCoord],
    pending: &[RegionDiff],
) -> Result<ProjectFile, String> {
    let squares = SourceSquares::open(source).map_err(|error| error.to_string())?;
    let mut diffs = Vec::new();
    for &region in dirty {
        let Some(square) = squares.read(region).map_err(|error| error.to_string())? else {
            continue;
        };
        let diff = diff_region(&square.terrain, &square.locations, document)
            .map_err(|error| format!("region {},{}: {error:?}", region.x, region.y))?;
        diffs.push(diff);
    }
    let covered: BTreeSet<RegionCoord> = dirty.iter().copied().collect();
    diffs.extend(
        pending
            .iter()
            .filter(|diff| !covered.contains(&diff.region))
            .cloned(),
    );
    diffs.sort_by_key(|diff| (diff.region.x, diff.region.y));
    Ok(ProjectFile::from_diffs(squares.profile_id(), &diffs))
}

/// Write `text` next to `path` first, so a crash never leaves a half-written project.
fn write_atomically(path: &Path, text: &str) -> Result<(), String> {
    let temp = path.with_extension("tmp");
    std::fs::write(&temp, text).map_err(|error| format!("writing {}: {error}", temp.display()))?;
    std::fs::rename(&temp, path).map_err(|error| format!("saving {}: {error}", path.display()))
}

fn tile_key(position: osrs_edit::MapPosition) -> TileKey {
    (
        position.plane.index().get(),
        position.tile.x,
        position.tile.y,
    )
}

impl EditorApp {
    /// Whether edits exist that no save or project open has captured.
    fn is_unsaved(&self) -> bool {
        self.session.edit_revision() != self.project.saved_edit_revision
    }

    fn has_project_changes(&self) -> bool {
        self.session.dirty_regions().next().is_some() || !self.project.pending.is_empty()
    }

    /// Route a cache/source replacement through the same unsaved-work lifecycle as project open.
    pub(super) fn request_source_change(
        &mut self,
        source: CacheSource,
        label: PathBuf,
        ctx: &egui::Context,
    ) {
        if self.project.save.is_some() {
            self.notice = Some((
                "Finish the current project save before switching caches.".to_owned(),
                Instant::now(),
            ));
            return;
        }
        self.request_transition(LifecycleTransition::Source { source, label }, ctx);
    }

    fn request_transition(&mut self, transition: LifecycleTransition, ctx: &egui::Context) {
        if self.is_unsaved() {
            self.project.transition = Some(transition);
            self.project.transition_prompt = true;
        } else {
            self.execute_transition(transition, ctx);
        }
    }

    fn execute_transition(&mut self, transition: LifecycleTransition, ctx: &egui::Context) {
        match transition {
            LifecycleTransition::Source { source, label } => {
                self.project.reset_for_source_change();
                self.session.reset();
                self.open_source(source, label, ctx);
            }
            LifecycleTransition::Project { path, diffs } => {
                let count = diffs.len();
                self.activate_project(path, diffs);
                self.notice = Some((
                    format!("Opening project: {count} edited region{}", plural(count)),
                    Instant::now(),
                ));
            }
        }
    }

    /// File menu entries.
    pub(super) fn project_menu(&mut self, ui: &mut egui::Ui) {
        let cache_open = self.cache_build.is_some();
        let idle = self.project.save.is_none();
        if ui
            .add_enabled(
                cache_open && idle,
                egui::Button::new(format!(
                    "{}  Open project...",
                    egui_phosphor::regular::FOLDER_OPEN
                )),
            )
            .clicked()
        {
            ui.close();
            let ctx = ui.ctx().clone();
            self.open_project(&ctx);
        }
        let can_save = cache_open && idle && self.has_project_changes();
        if ui
            .add_enabled(
                can_save,
                egui::Button::new(format!(
                    "{}  Save project   {}",
                    egui_phosphor::regular::FLOPPY_DISK,
                    self.keymap.describe("project.save")
                )),
            )
            .on_disabled_hover_text("Nothing to save yet")
            .clicked()
        {
            ui.close();
            let _ = self.save_project(false);
        }
        if ui
            .add_enabled(can_save, egui::Button::new("Save project as..."))
            .clicked()
        {
            ui.close();
            let _ = self.save_project(true);
        }
    }

    /// The Save shortcut.
    pub(super) fn project_actions(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| self.keymap.pressed(i, "project.save"))
            && self.cache_build.is_some()
            && self.project.save.is_none()
            && self.has_project_changes()
        {
            let _ = self.save_project(false);
        }
    }

    /// Returns whether a save job was actually started (the user may cancel the file dialog).
    fn save_project(&mut self, ask: bool) -> bool {
        let path = match (&self.project.path, ask) {
            (Some(path), false) => path.clone(),
            _ => {
                let Some(path) = rfd::FileDialog::new()
                    .set_title("Save project")
                    .add_filter("RustOSRS project", &["json"])
                    .set_file_name("map-edits.rustosrs.json")
                    .save_file()
                else {
                    return false;
                };
                path
            }
        };
        self.start_save(path);
        true
    }

    fn start_save(&mut self, path: PathBuf) {
        self.project.path = Some(path.clone());
        self.project.saving_revision = self.session.edit_revision();
        let source = CacheSource::Dir(self.cache_dir.clone());
        let document = self.session.document().clone();
        let dirty: Vec<RegionCoord> = self.session.dirty_regions().collect();
        let pending: Vec<RegionDiff> = self.project.pending.values().cloned().collect();
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let result = build_project(source, &document, &dirty, &pending).and_then(|file| {
                write_atomically(&path, &file.to_json())?;
                Ok((path, file.regions.len()))
            });
            let _ = sender.send(result);
        });
        self.project.save = Some(receiver);
        self.notice = Some(("Saving project...".to_owned(), Instant::now()));
    }

    /// `project save|open PATH` from the script language (no dialogs).
    pub(super) fn script_project(&mut self, action: &str, path: &str) -> Result<String, String> {
        if self.cache_build.is_none() {
            return Err("project needs an open cache".to_owned());
        }
        match action {
            "save" if self.project.save.is_none() => {
                self.start_save(PathBuf::from(path));
                Ok(format!("Saving project to {path}"))
            }
            "save" => Err("a save is already running".to_owned()),
            "open" if self.project.save.is_some() => {
                Err("finish the current project save before opening another project".to_owned())
            }
            "open" if self.is_unsaved() => Err(
                "unsaved changes exist; save or discard them before scripted project open"
                    .to_owned(),
            ),
            "open" => {
                let diffs = self.load_project(Path::new(path))?;
                let count = diffs.len();
                self.activate_project(PathBuf::from(path), diffs);
                Ok(format!(
                    "Opening project: {count} edited region{}",
                    plural(count)
                ))
            }
            other => Err(format!("unknown project action '{other}'")),
        }
    }

    fn open_project(&mut self, ctx: &egui::Context) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Open project")
            .add_filter("RustOSRS project", &["json"])
            .pick_file()
        else {
            return;
        };
        let diffs = match self.load_project(&path) {
            Ok(diffs) => diffs,
            Err(error) => {
                self.notice = Some((format!("Could not open project: {error}"), Instant::now()));
                return;
            }
        };
        self.request_transition(LifecycleTransition::Project { path, diffs }, ctx);
    }

    /// Read and validate a project against the currently open cache.
    fn load_project(&self, path: &Path) -> Result<Vec<RegionDiff>, String> {
        let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
        let file = ProjectFile::from_json(&text).map_err(|error| format!("{error:?}"))?;
        let profile = self.cache_build.as_ref().map(|(_, id)| id.clone());
        if profile.as_deref() != Some(file.profile.as_str()) {
            return Err(format!(
                "it was made for cache {}, but {} is open",
                file.profile,
                profile.as_deref().unwrap_or("no cache")
            ));
        }
        debug_assert_eq!(file.format, PROJECT_FORMAT);
        file.into_diffs().map_err(|error| format!("{error:?}"))
    }

    /// Replace the current editing session with `diffs` from the pristine current cache source.
    fn activate_project(&mut self, path: PathBuf, diffs: Vec<RegionDiff>) {
        debug_assert!(self.project.save.is_none());
        self.project.reset_for_source_change();
        self.session.reset();
        self.project.path = Some(path);
        self.project.pending = diffs.into_iter().map(|diff| (diff.region, diff)).collect();
        self.project.saved_edit_revision = self.session.edit_revision();
        // Restarting the source clears worker-side edit projections as well as all resident legacy
        // projections. Project diffs are then applied only to fresh source regions as they arrive.
        self.restart_current_source();
    }

    /// Apply the opened project's diff for `region` if one is waiting. Called whenever a region's
    /// semantic state has just been loaded. The diff remains queued until semantic application and
    /// every synchronous projection step succeeds, so failures are retryable instead of lossy.
    pub(super) fn apply_project_region(&mut self, region: RegionCoord) {
        let Some(diff) = self.project.pending.get(&region).cloned() else {
            return;
        };
        let patch = diff.to_patch(self.session.document());
        let was_saved = !self.is_unsaved();
        match self.session.apply_untracked(&patch) {
            Ok(false) => {}
            Ok(true) => {
                // Applying a project's own edits does not make the session "unsaved". Projection
                // failure is a presentation/runtime failure, not a new semantic user edit.
                if was_saved {
                    self.project.saved_edit_revision = self.session.edit_revision();
                }
            }
            Err(error) => {
                self.notice = Some((
                    format!(
                        "Project region {},{} could not be applied: {error:?}",
                        region.x, region.y
                    ),
                    Instant::now(),
                ));
                return;
            }
        }

        // Derive projection work from the project diff, not only the just-applied patch. If a prior
        // projection attempt failed after semantic application, the next retry still re-projects the
        // desired state even though `to_patch` is now empty.
        let mut terrain_keys: Vec<TileKey> = diff
            .tiles
            .iter()
            .map(|(position, _)| tile_key(*position))
            .collect();
        let height_keys: Vec<TileKey> = diff
            .heights
            .iter()
            .map(|(coord, _)| {
                (
                    coord.plane.index().get(),
                    coord.corner.x,
                    coord.corner.y,
                )
            })
            .collect();
        terrain_keys.retain(|key| !height_keys.contains(key));
        if let Err(error) = self.project_document_tiles(&terrain_keys, &height_keys) {
            self.notice = Some((
                format!("Project tiles could not be projected: {error}"),
                Instant::now(),
            ));
            return;
        }

        if !diff.objects_removed.is_empty() || !diff.objects_added.is_empty() {
            let route = ObjectHistoryRoute::new(
                diff.objects_removed
                    .iter()
                    .copied()
                    .map(spec_from_placement)
                    .collect(),
                diff.objects_added
                    .iter()
                    .copied()
                    .map(spec_from_placement)
                    .collect(),
            );
            match build_object_history_projection(self.session.document(), &route, true) {
                Ok(projection) => self.send_loc_edits(projection.edits),
                Err(error) => {
                    self.notice = Some((
                        format!("Project objects could not be projected: {error:?}"),
                        Instant::now(),
                    ));
                    return;
                }
            }
            self.clear_selection();
        }

        self.project.pending.remove(&region);
    }

    /// Show and resolve Save / Discard / Cancel for cache/project replacement.
    /// Returns true while the lifecycle prompt owns the frame's modal interaction.
    fn transition_guard(&mut self, ctx: &egui::Context) -> bool {
        if self.project.transition_after_save && self.project.save.is_none() {
            self.project.transition_after_save = false;
            if !self.is_unsaved() {
                if let Some(transition) = self.project.transition.take() {
                    self.execute_transition(transition, ctx);
                }
                self.project.transition_prompt = false;
                return false;
            }
            self.project.transition_prompt = true;
        }

        if !self.project.transition_prompt {
            return false;
        }
        let action = self
            .project
            .transition
            .as_ref()
            .map(LifecycleTransition::description)
            .unwrap_or("replace the current session");
        let (choice, closed) = super::modal::Modal::new("replace-unsaved", "Unsaved changes")
            .size(420.0, 0.0)
            .show(ctx, |ui| {
                ui.label(format!(
                    "Your map edits have not been saved. Save them before you {action}?"
                ));
                ui.add_space(8.0);
                let mut choice = None;
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        choice = Some(TransitionChoice::Save);
                    }
                    if ui.button("Discard").clicked() {
                        choice = Some(TransitionChoice::Discard);
                    }
                    if ui.button("Cancel").clicked() {
                        choice = Some(TransitionChoice::Cancel);
                    }
                });
                choice
            });
        match choice {
            Some(TransitionChoice::Save) => {
                self.project.transition_prompt = false;
                if self.save_project(false) {
                    self.project.transition_after_save = true;
                } else {
                    self.project.transition_prompt = true;
                }
            }
            Some(TransitionChoice::Discard) => {
                self.project.transition_prompt = false;
                if let Some(transition) = self.project.transition.take() {
                    self.execute_transition(transition, ctx);
                }
            }
            Some(TransitionChoice::Cancel) => {
                self.project.transition_prompt = false;
                self.project.transition_after_save = false;
                self.project.transition = None;
            }
            None if closed => {
                self.project.transition_prompt = false;
                self.project.transition_after_save = false;
                self.project.transition = None;
            }
            None => {}
        }
        true
    }

    /// Intercept closing the window while edits are unsaved, and finish a save-then-quit.
    pub(super) fn quit_guard(&mut self, ctx: &egui::Context) {
        let smoke = self.exit_after_seconds.is_some() || self.screenshot.is_some();
        if !smoke
            && ctx.input(|i| i.viewport().close_requested())
            && self.is_unsaved()
            && !self.project.discard_confirmed
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            // Closing the application supersedes an in-progress source/project replacement choice.
            self.project.transition = None;
            self.project.transition_prompt = false;
            self.project.transition_after_save = false;
            self.project.quit_prompt = true;
        }
        if self.project.close_when_saved && self.project.save.is_none() {
            self.project.close_when_saved = false;
            if !self.is_unsaved() {
                self.project.discard_confirmed = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        if !self.project.quit_prompt && self.transition_guard(ctx) {
            return;
        }
        if !self.project.quit_prompt {
            return;
        }
        let (choice, closed) = super::modal::Modal::new("quit-unsaved", "Unsaved changes")
            .size(380.0, 0.0)
            .show(ctx, |ui| {
                ui.label("Your map edits have not been saved to a project.");
                ui.add_space(8.0);
                let mut choice = None;
                ui.horizontal(|ui| {
                    if ui.button("Save and quit").clicked() {
                        choice = Some(QuitChoice::Save);
                    }
                    if ui.button("Quit without saving").clicked() {
                        choice = Some(QuitChoice::Discard);
                    }
                    if ui.button("Cancel").clicked() {
                        choice = Some(QuitChoice::Cancel);
                    }
                });
                choice
            });
        match choice {
            Some(QuitChoice::Save) => {
                self.project.quit_prompt = false;
                if self.cache_build.is_some()
                    && self.project.save.is_none()
                    && self.save_project(false)
                {
                    self.project.close_when_saved = true;
                }
            }
            Some(QuitChoice::Discard) => {
                self.project.quit_prompt = false;
                self.project.discard_confirmed = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Some(QuitChoice::Cancel) => self.project.quit_prompt = false,
            None if closed => self.project.quit_prompt = false,
            None => {}
        }
    }

    /// Report a finished save once per frame.
    pub(super) fn poll_project(&mut self) {
        let Some(receiver) = &self.project.save else {
            return;
        };
        let message = match receiver.try_recv() {
            Ok(Ok((path, count))) => {
                self.project.saved_edit_revision = self.project.saving_revision;
                format!(
                    "Saved project: {count} edited region{} to {}",
                    plural(count),
                    path.display()
                )
            }
            Ok(Err(error)) => {
                self.project.close_when_saved = false;
                self.project.transition_after_save = false;
                self.project.transition_prompt = self.project.transition.is_some();
                format!("Save failed: {error}")
            }
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => {
                self.project.close_when_saved = false;
                self.project.transition_after_save = false;
                self.project.transition_prompt = self.project.transition.is_some();
                "Save stopped unexpectedly".to_owned()
            }
        };
        self.project.save = None;
        self.notice = Some((message, Instant::now()));
    }
}

#[derive(Clone, Copy)]
enum QuitChoice {
    Save,
    Discard,
    Cancel,
}

#[derive(Clone, Copy)]
enum TransitionChoice {
    Save,
    Discard,
    Cancel,
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_change_reset_clears_source_bound_project_state() {
        let region = RegionCoord::new(50, 51);
        let mut state = ProjectState::default();
        state.pending.insert(
            region,
            RegionDiff {
                region,
                tiles: Vec::new(),
                heights: Vec::new(),
                objects_added: Vec::new(),
                objects_removed: Vec::new(),
            },
        );
        state.path = Some(PathBuf::from("old-project.json"));
        state.saved_edit_revision = 7;
        state.saving_revision = 6;
        state.quit_prompt = true;
        state.close_when_saved = true;
        state.discard_confirmed = true;
        state.transition_prompt = true;
        state.transition_after_save = true;
        state.transition = Some(LifecycleTransition::Project {
            path: PathBuf::from("next-project.json"),
            diffs: Vec::new(),
        });

        state.reset_for_source_change();

        assert!(state.pending.is_empty());
        assert!(state.path.is_none());
        assert!(state.save.is_none());
        assert_eq!(state.saved_edit_revision, 0);
        assert_eq!(state.saving_revision, 0);
        assert!(!state.quit_prompt);
        assert!(!state.close_when_saved);
        assert!(!state.discard_confirmed);
        assert!(state.transition.is_none());
        assert!(!state.transition_prompt);
        assert!(!state.transition_after_save);
    }
}
