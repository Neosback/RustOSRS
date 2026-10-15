//! File > Open / Save project: persist an editing session's changes between runs.
//!
//! A project file stores only the differences against the source cache (see
//! `session::project_file`). Saving diffs the authoritative document against freshly read source
//! squares on a worker thread. Opening validates the file against the open cache's profile, then
//! restarts that cache from its pristine source before applying each project region as it streams.
//! This keeps Open Project replacement semantics distinct from a future explicit merge/import.

mod lifecycle;
#[cfg(test)]
mod tests;

use super::EditorApp;
use super::editing::TileKey;
use super::object_tool::{
    ObjectHistoryRoute, build_object_history_projection, spec_from_placement,
};
use crate::session::{EditorSession, PROJECT_FORMAT, ProjectFile, RegionDiff, diff_region};
use eframe::egui;
use osrs_cache::source::CacheSource;
use osrs_core::coords::RegionCoord;
use osrs_edit::MapDocument;
use osrs_world::SourceSquares;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError};
use web_time::Instant;

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
    transition: Option<lifecycle::LifecycleTransition>,
    transition_prompt: bool,
    /// Execute `transition` after the save currently requested by the transition prompt succeeds.
    transition_after_save: bool,
}

impl ProjectState {
    pub(in crate::app) fn has_pending_exports(&self) -> bool {
        !self.pending.is_empty()
    }
    pub(in crate::app) fn export_pending(&self) -> Vec<RegionDiff> {
        self.pending.values().cloned().collect()
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
        let square = squares
            .read(region)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| {
                format!(
                    "Cannot save authored region {},{}: its original map data is missing.",
                    region.x, region.y
                )
            })?;
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

/// Establish a clean semantic/project state before the current source is restarted.
fn prepare_project_replacement(
    project: &mut ProjectState,
    session: &mut EditorSession,
    path: PathBuf,
    diffs: Vec<RegionDiff>,
) {
    debug_assert!(project.save.is_none());
    project.reset_for_source_change();
    session.reset();
    project.path = Some(path);
    project.pending = diffs.into_iter().map(|diff| (diff.region, diff)).collect();
    project.saved_edit_revision = session.edit_revision();
}

/// Apply the queued semantic diff without consuming it. Projection success owns final removal.
fn apply_pending_project_semantics(
    project: &ProjectState,
    session: &mut EditorSession,
    region: RegionCoord,
) -> Result<Option<(RegionDiff, bool)>, osrs_edit::PatchApplyError> {
    let Some(diff) = project.pending.get(&region).cloned() else {
        return Ok(None);
    };
    let patch = diff.to_patch(session.document());
    let changed = session.apply_untracked(&patch)?;
    Ok(Some((diff, changed)))
}

impl EditorApp {
    /// Whether edits exist that no save or project open has captured.
    fn is_unsaved(&self) -> bool {
        self.session.edit_revision() != self.project.saved_edit_revision
    }

    fn has_project_changes(&self) -> bool {
        self.session.dirty_regions().next().is_some() || !self.project.pending.is_empty()
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
        self.request_transition(lifecycle::LifecycleTransition::Project { path, diffs }, ctx);
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
        prepare_project_replacement(&mut self.project, &mut self.session, path, diffs);
        // Restarting the source clears worker-side edit projections as well as all resident legacy
        // projections. Project diffs are then applied only to fresh source regions as they arrive.
        self.restart_current_source();
    }

    /// Apply the opened project's diff for `region` if one is waiting. Called whenever a region's
    /// semantic state has just been loaded. The diff remains queued until semantic application and
    /// every synchronous projection step succeeds, so failures are retryable instead of lossy.
    pub(super) fn apply_project_region(&mut self, region: RegionCoord) {
        let was_saved = !self.is_unsaved();
        let applied =
            match apply_pending_project_semantics(&self.project, &mut self.session, region) {
                Ok(applied) => applied,
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
            };
        let Some((diff, changed)) = applied else {
            return;
        };
        if changed && was_saved {
            // Applying a project's own edits does not make the session "unsaved". Projection
            // failure is a presentation/runtime failure, not a new semantic user edit.
            self.project.saved_edit_revision = self.session.edit_revision();
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
            .map(|(coord, _)| (coord.plane.index().get(), coord.corner.x, coord.corner.y))
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

fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}
