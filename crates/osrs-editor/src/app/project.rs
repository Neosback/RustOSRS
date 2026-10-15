//! File > Open / Save project: persist an editing session's changes between runs.
//!
//! A project file stores only the differences against the source cache (see
//! `session::project_file`). Saving diffs the authoritative document against freshly read source
//! squares on a worker thread. Opening validates the file against the open cache's profile, then
//! restarts that cache from its pristine source before applying each project region as it streams.
//! This keeps Open Project replacement semantics distinct from a future explicit merge/import.

mod lifecycle;

use super::EditorApp;
use super::editing::TileKey;
use super::object_tool::{
    ObjectHistoryRoute, build_object_history_projection, spec_from_placement,
};
use crate::session::{PROJECT_FORMAT, ProjectFile, RegionDiff};
use eframe::egui;
use osrs_core::coords::RegionCoord;
use std::path::{Path, PathBuf};

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
        self.project.is_unsaved(self.session.edit_revision())
    }

    fn has_project_changes(&self) -> bool {
        self.project
            .has_changes(self.session.dirty_regions().next().is_some())
    }

    /// File menu entries.
    pub(super) fn project_menu(&mut self, ui: &mut egui::Ui) {
        let cache_open = self.project.cache_build().is_some();
        let idle = !self.project.save_running();
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
                    self.settings.keymap.describe("project.save")
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
        if ctx.input(|i| self.settings.keymap.pressed(i, "project.save"))
            && self.project.cache_build().is_some()
            && !self.project.save_running()
            && self.has_project_changes()
        {
            let _ = self.save_project(false);
        }
    }

    /// Returns whether a save job was actually started (the user may cancel the file dialog).
    fn save_project(&mut self, ask: bool) -> bool {
        let path = match (self.project.project_path(), ask) {
            (Some(path), false) => path.to_path_buf(),
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
        self.project.start_save(path, &self.session);
        self.ui.notify("Saving project...".to_owned());
    }

    /// `project save|open PATH` from the script language (no dialogs).
    pub(super) fn script_project(&mut self, action: &str, path: &str) -> Result<String, String> {
        if self.project.cache_build().is_none() {
            return Err("project needs an open cache".to_owned());
        }
        match action {
            "save" if !self.project.save_running() => {
                self.start_save(PathBuf::from(path));
                Ok(format!("Saving project to {path}"))
            }
            "save" => Err("a save is already running".to_owned()),
            "open" if self.project.save_running() => {
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
                self.ui.notify(format!("Could not open project: {error}"));
                return;
            }
        };
        self.request_transition(
            super::project_runtime::LifecycleTransition::Project { path, diffs },
            ctx,
        );
    }

    /// Read and validate a project against the currently open cache.
    fn load_project(&self, path: &Path) -> Result<Vec<RegionDiff>, String> {
        let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
        let file = ProjectFile::from_json(&text).map_err(|error| format!("{error:?}"))?;
        let profile = self
            .project
            .cache_build()
            .as_ref()
            .map(|(_, id)| id.clone());
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
        self.project
            .prepare_replacement(&mut self.session, path, diffs);
        // Restarting the source clears worker-side edit projections as well as all resident legacy
        // projections. Project diffs are then applied only to fresh source regions as they arrive.
        self.restart_current_source();
    }

    /// Apply the opened project's diff for `region` if one is waiting. Called whenever a region's
    /// semantic state has just been loaded. The diff remains queued until semantic application and
    /// every synchronous projection step succeeds, so failures are retryable instead of lossy.
    pub(super) fn apply_project_region(&mut self, region: RegionCoord) {
        let applied = match self
            .project
            .apply_pending_semantics(&mut self.session, region)
        {
            Ok(applied) => applied,
            Err(error) => {
                self.ui.notify(format!(
                    "Project region {},{} could not be applied: {error:?}",
                    region.x, region.y
                ));
                return;
            }
        };
        let Some(diff) = applied else {
            return;
        };
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
            self.ui
                .notify(format!("Project tiles could not be projected: {error}"));
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
                    self.ui
                        .notify(format!("Project objects could not be projected: {error:?}"));
                    return;
                }
            }
            self.clear_selection();
        }

        self.project.complete_region_projection(region);
    }

    /// Report a finished save once per frame.
    pub(super) fn poll_project(&mut self) {
        let Some(result) = self.project.poll_save() else {
            return;
        };
        let message = match result {
            Ok((path, count)) => format!(
                "Saved project: {count} edited region{} to {}",
                plural(count),
                path.display()
            ),
            Err(error) => format!("Save failed: {error}"),
        };
        self.ui.notify(message);
    }
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}
