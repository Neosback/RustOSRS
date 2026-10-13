//! File > Open / Save project: persist an editing session's changes between runs.
//!
//! A project file stores only the differences against the source cache (see
//! `session::project_file`). Saving diffs the authoritative document against freshly read source
//! squares on a worker thread. Opening validates the file against the open cache's profile, clears
//! undo history, and applies each region's diff to the session the moment that region is loaded
//! (immediately for resident regions, on streaming arrival for the rest), projecting the result
//! into the worker/picker mirrors exactly like an interactive edit would.

use super::EditorApp;
use super::editing::TileKey;
use super::object_tool::{
    ObjectHistoryRoute, build_object_history_projection, spec_from_placement,
};
use crate::session::{PROJECT_FORMAT, ProjectFile, RegionDiff, diff_region};
use eframe::egui;
use osrs_core::coords::RegionCoord;
use osrs_edit::{MapDocument, MapOp};
use osrs_world::SourceSquares;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError};
use web_time::Instant;

#[derive(Default)]
pub(super) struct ProjectState {
    /// Diffs from an opened project whose region has not been applied yet.
    pending: BTreeMap<RegionCoord, RegionDiff>,
    /// The file Save writes to.
    path: Option<PathBuf>,
    save: Option<Receiver<Result<(PathBuf, usize), String>>>,
}

/// Build the project file contents for `document`, keeping unapplied `pending` diffs for regions
/// that are not being re-diffed. Pure apart from reading the source cache.
fn build_project(
    source: osrs_cache::source::CacheSource,
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
    fn has_project_changes(&self) -> bool {
        self.session.dirty_regions().next().is_some() || !self.project.pending.is_empty()
    }

    /// File menu entries.
    pub(super) fn project_menu(&mut self, ui: &mut egui::Ui) {
        let cache_open = self.cache_build.is_some();
        let idle = self.project.save.is_none();
        if ui
            .add_enabled(
                cache_open,
                egui::Button::new(format!(
                    "{}  Open project...",
                    egui_phosphor::regular::FOLDER_OPEN
                )),
            )
            .clicked()
        {
            ui.close();
            self.open_project();
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
            self.save_project(false);
        }
        if ui
            .add_enabled(can_save, egui::Button::new("Save project as..."))
            .clicked()
        {
            ui.close();
            self.save_project(true);
        }
    }

    /// The Save shortcut.
    pub(super) fn project_actions(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| self.keymap.pressed(i, "project.save"))
            && self.cache_build.is_some()
            && self.project.save.is_none()
            && self.has_project_changes()
        {
            self.save_project(false);
        }
    }

    fn save_project(&mut self, ask: bool) {
        let path = match (&self.project.path, ask) {
            (Some(path), false) => path.clone(),
            _ => {
                let Some(path) = rfd::FileDialog::new()
                    .set_title("Save project")
                    .add_filter("RustOSRS project", &["json"])
                    .set_file_name("map-edits.rustosrs.json")
                    .save_file()
                else {
                    return;
                };
                path
            }
        };
        self.start_save(path);
    }

    fn start_save(&mut self, path: PathBuf) {
        self.project.path = Some(path.clone());
        let source = osrs_cache::source::CacheSource::Dir(self.cache_dir.clone());
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
            "open" => self
                .begin_project(Path::new(path))
                .map(|count| format!("Opened project: {count} edited region{}", plural(count))),
            other => Err(format!("unknown project action '{other}'")),
        }
    }

    fn open_project(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Open project")
            .add_filter("RustOSRS project", &["json"])
            .pick_file()
        else {
            return;
        };
        let message = match self.begin_project(&path) {
            Ok(count) => {
                self.project.path = Some(path);
                format!("Opened project: {count} edited region{}", plural(count))
            }
            Err(error) => format!("Could not open project: {error}"),
        };
        self.notice = Some((message, Instant::now()));
    }

    /// Validate `path` against the open cache and queue its regions. Returns the region count.
    fn begin_project(&mut self, path: &Path) -> Result<usize, String> {
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
        let diffs = file.into_diffs().map_err(|error| format!("{error:?}"))?;
        let count = diffs.len();
        // The opened project replaces the undo history: its edits are not undoable steps.
        self.session.clear_history();
        self.history.clear();
        self.redo_stack.clear();
        self.project.pending = diffs.into_iter().map(|diff| (diff.region, diff)).collect();
        let resident: Vec<RegionCoord> = self
            .project
            .pending
            .keys()
            .copied()
            .filter(|region| self.session.is_loaded(*region))
            .collect();
        for region in resident {
            self.apply_project_region(region);
        }
        Ok(count)
    }

    /// Apply the opened project's diff for `region` if one is waiting. Called whenever a region's
    /// semantic state has just been loaded (and right after opening for resident regions).
    pub(super) fn apply_project_region(&mut self, region: RegionCoord) {
        let Some(diff) = self.project.pending.remove(&region) else {
            return;
        };
        let patch = diff.to_patch(self.session.document());
        match self.session.apply_untracked(&patch) {
            Ok(false) => return,
            Ok(true) => {}
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
        let (mut terrain_keys, mut height_keys) = (Vec::new(), Vec::new());
        let (mut removed, mut added) = (Vec::new(), Vec::new());
        for op in patch.ops() {
            match op {
                MapOp::SetTerrain { coord, .. } => terrain_keys.push(tile_key(*coord)),
                MapOp::SetHeight { coord, .. } => {
                    height_keys.push((coord.plane.index().get(), coord.corner.x, coord.corner.y));
                }
                MapOp::RemoveObject { placement } => {
                    removed.push(spec_from_placement(*placement));
                }
                MapOp::AddObject { placement } => {
                    added.push(spec_from_placement(*placement));
                }
            }
        }
        terrain_keys.retain(|key| !height_keys.contains(key));
        if let Err(error) = self.project_document_tiles(&terrain_keys, &height_keys) {
            self.notice = Some((
                format!("Project tiles could not be projected: {error}"),
                Instant::now(),
            ));
        }
        if !removed.is_empty() || !added.is_empty() {
            let route = ObjectHistoryRoute::new(removed, added);
            match build_object_history_projection(self.session.document(), &route, true) {
                Ok(projection) => self.send_loc_edits(projection.edits),
                Err(error) => {
                    self.notice = Some((
                        format!("Project objects could not be projected: {error:?}"),
                        Instant::now(),
                    ));
                }
            }
            self.clear_selection();
        }
    }

    /// Report a finished save once per frame.
    pub(super) fn poll_project(&mut self) {
        let Some(receiver) = &self.project.save else {
            return;
        };
        let message = match receiver.try_recv() {
            Ok(Ok((path, count))) => format!(
                "Saved project: {count} edited region{} to {}",
                plural(count),
                path.display()
            ),
            Ok(Err(error)) => format!("Save failed: {error}"),
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => "Save stopped unexpectedly".to_owned(),
        };
        self.project.save = None;
        self.notice = Some((message, Instant::now()));
    }
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}
