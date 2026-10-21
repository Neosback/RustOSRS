//! The prefab library and the state of placing one: the saved prefabs, their files, the search
//! text, the selection and the armed placement gesture.
//!
//! Plain data and file IO only. The panel, the viewport gesture and the edit commit live
//! elsewhere and talk to this owner through narrow operations; nothing here touches the map.

mod build;
#[cfg(not(target_arch = "wasm32"))]
pub(in crate::app) use build::reproduction::Source as ReproductionSource;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use build::reproduction::run_cli as reproduce_cli;
mod store;
mod trash;
pub(in crate::app) use trash::TrashState;

use crate::app::npc_runtime::{OpenTask, SaveTask, open_text, save_text};
pub(in crate::app) use build::starter_kit;
pub(in crate::app) use build::{
    ARCHETYPES, BuildForm, BuildState, GenerationMode, MAX_SITE, STYLES,
};
use osrs_edit::{CaptureSpec, Prefab, PrefabError, PrefabSource, capture_prefab};
use std::path::PathBuf;
use store::Store;

/// What the next click places, and how.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::app) struct Armed {
    /// File name of the prefab in the library (stable while the list reorders).
    pub(in crate::app) file: String,
    pub(in crate::app) turns: u8,
    pub(in crate::app) terrain: bool,
    pub(in crate::app) heights: bool,
    pub(in crate::app) objects: bool,
    pub(in crate::app) replace_objects: bool,
}

/// One entry of the library: the prefab and the file it lives in.
#[derive(Debug, Clone)]
pub(in crate::app) struct Entry {
    pub(in crate::app) file: String,
    pub(in crate::app) prefab: Prefab,
}

/// What the capture form holds.
#[derive(Debug, Clone)]
pub(in crate::app) struct CaptureForm {
    pub(in crate::app) building_only: bool,
    pub(in crate::app) name: String,
    pub(in crate::app) planes: u8,
    pub(in crate::app) terrain: bool,
    pub(in crate::app) heights: bool,
    pub(in crate::app) objects: bool,
}

impl Default for CaptureForm {
    fn default() -> Self {
        Self {
            building_only: true,
            name: String::new(),
            planes: 1,
            terrain: true,
            heights: true,
            objects: true,
        }
    }
}

/// Which page of the panel is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::app) enum Page {
    #[default]
    Library,
    Build,
}

#[derive(Default)]
pub(in crate::app) struct PrefabRuntime {
    entries: Vec<Entry>,
    store: Store,
    /// File name of the selected entry.
    pub(in crate::app) selected: Option<String>,
    pub(in crate::app) filter: String,
    pub(in crate::app) form: CaptureForm,
    pub(in crate::app) page: Page,
    pub(in crate::app) build: BuildState,
    pub(in crate::app) armed: Option<Armed>,
    pub(in crate::app) placement_review:
        Option<crate::app::prefab_panel::placement_review::PlacementReview>,
    pub(in crate::app) gallery: crate::app::prefab_gallery::Gallery,
    pub(in crate::app) workshop: crate::app::workshop::Workshop,
    pub(in crate::app) capture_review: crate::app::prefab_panel::capture::CaptureReview,
    pub(in crate::app) studio: Option<Box<crate::app::prefab_studio::Studio>>,
    pub(in crate::app) studio_overlay: Option<crate::app::prefab_studio::overlays::Overlay>,
    pub(in crate::app) stage_region: Option<osrs_core::coords::RegionCoord>,
    pub(in crate::app) studio_request: Option<String>,
    pub(in crate::app) studio_command: Option<crate::app::prefab_studio::SceneCommand>,
    pub(in crate::app) trash: TrashState,
    notice: Option<String>,
    import_task: Option<OpenTask>,
    export_task: Option<SaveTask>,
}

impl PrefabRuntime {
    /// The library in its usual place beside the settings (native), read from disk.
    pub(in crate::app) fn open() -> Self {
        let mut runtime = Self::default();
        #[cfg(not(target_arch = "wasm32"))]
        {
            runtime.store = Store::beside_settings();
        }
        runtime.load();
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(directory) = runtime.directory() {
            runtime.build.load_kits(&directory.with_file_name("kits"));
        }
        runtime
    }

    /// A library in `directory`, read from disk.
    #[cfg(test)]
    fn in_directory(directory: PathBuf) -> Self {
        let mut runtime = Self {
            store: Store::in_directory(directory),
            ..Self::default()
        };
        runtime.load();
        runtime
    }

    /// Read the library from disk (native); the browser build starts empty.
    pub(in crate::app) fn load(&mut self) {
        let (entries, problems) = self.store.read_all();
        self.entries = entries;
        if !problems.is_empty() {
            self.notice = Some(format!(
                "{} prefab file{} could not be read: {}",
                problems.len(),
                if problems.len() == 1 { "" } else { "s" },
                problems.join("; ")
            ));
        }
    }

    pub(in crate::app) fn directory(&self) -> Option<PathBuf> {
        self.store.directory()
    }

    pub(in crate::app) fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub(in crate::app) fn entry(&self, file: &str) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.file == file)
    }

    /// Extract from the saved source asset, never an editor or GPU projection.
    pub(in crate::app) fn extract_assembly(
        &mut self,
        file: &str,
        member: usize,
        name: &str,
    ) -> Result<String, String> {
        let prefab = self
            .entry(file)
            .ok_or("The source prefab no longer exists")?
            .prefab
            .extract_assembly(member, name)
            .map_err(|e| e.to_string())?;
        self.add(prefab)
    }

    pub(in crate::app) fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    pub(in crate::app) fn say(&mut self, message: impl Into<String>) {
        self.notice = Some(message.into());
    }

    pub(in crate::app) fn clear_notice(&mut self) {
        self.notice = None;
    }

    /// Entries whose name, tags or notes contain every word of the filter, by name.
    pub(in crate::app) fn visible(&self) -> Vec<&Entry> {
        let words: Vec<String> = self
            .filter
            .split_whitespace()
            .map(str::to_lowercase)
            .collect();
        let mut shown: Vec<&Entry> = self
            .entries
            .iter()
            .filter(|entry| {
                let haystack = format!(
                    "{} {} {}",
                    entry.prefab.name,
                    entry.prefab.tags.join(" "),
                    entry.prefab.notes
                )
                .to_lowercase();
                words.iter().all(|word| haystack.contains(word))
            })
            .collect();
        shown.sort_by_key(|entry| entry.prefab.name.to_lowercase());
        shown
    }

    /// Save a new prefab in the library and select it.
    pub(in crate::app) fn add(&mut self, mut prefab: Prefab) -> Result<String, String> {
        prefab.version = osrs_edit::PREFAB_VERSION;
        prefab.validate().map_err(|error| error.to_string())?;
        let taken: Vec<&str> = self.entries.iter().map(|e| e.file.as_str()).collect();
        let file = self.store.unique_file_name(&prefab.name, &taken);
        self.store.write(&file, &prefab)?;
        self.entries.push(Entry {
            file: file.clone(),
            prefab,
        });
        self.selected = Some(file.clone());
        Ok(file)
    }

    pub(in crate::app) fn sync_entries_from(&mut self, other: &Self) {
        self.entries = other.entries.clone();
    }

    pub(in crate::app) fn create_blank_scene(&mut self) -> Result<String, String> {
        let floors: Vec<_> = (0..12)
            .flat_map(|y| (0..12).map(move |x| osrs_edit::PrefabCell { x, y, plane: 0 }))
            .collect();
        let support: Vec<_> = (0..=12)
            .flat_map(|y| (0..=12).map(move |x| osrs_edit::PrefabCell { x, y, plane: 0 }))
            .collect();
        let tiles = floors
            .iter()
            .map(|c| osrs_edit::PrefabTile {
                x: c.x,
                y: c.y,
                plane: c.plane,
                underlay: Some(50),
                overlay: None,
                flags: 0,
            })
            .collect();
        let heights = support
            .iter()
            .map(|c| osrs_edit::PrefabHeight {
                x: c.x,
                y: c.y,
                plane: c.plane,
                value: 0,
            })
            .collect();
        self.add(Prefab {
            format: osrs_edit::PREFAB_FORMAT.into(),
            version: osrs_edit::PREFAB_VERSION,
            name: "Blank scene".into(),
            tags: vec!["scene".into()],
            notes: "Authored in Prefab Studio".into(),
            source: PrefabSource::Authored,
            width: 12,
            height: 12,
            planes: 1,
            tiles,
            heights,
            objects: vec![],
            building: Some(osrs_edit::PrefabBuilding {
                semantics: Default::default(),
                floors,
                support,
                source_datum: 0,
                object_sizes: Default::default(),
                provenance: "authored".into(),
                relations: vec![],
                // A blank scene is not a reviewed building and must not teach anything.
                reviewed: false,
                labels: Default::default(),
            }),
        })
    }

    /// Change a prefab's name, tags or notes and save it.
    pub(in crate::app) fn edit(
        &mut self,
        file: &str,
        change: impl FnOnce(&mut Prefab),
    ) -> Result<(), String> {
        let Some(entry) = self.entries.iter_mut().find(|entry| entry.file == file) else {
            return Err("that prefab is no longer in the library".into());
        };
        let mut edited = entry.prefab.clone();
        change(&mut edited);
        edited.version = osrs_edit::PREFAB_VERSION;
        edited.validate().map_err(|error| error.to_string())?;
        self.store.write(file, &edited)?;
        entry.prefab = edited;
        Ok(())
    }

    /// Detached authoring cannot overwrite a newer library edit from the world or another window.
    pub(in crate::app) fn replace_if_unchanged(
        &mut self,
        file: &str,
        baseline: &Prefab,
        replacement: Prefab,
    ) -> Result<(), String> {
        #[cfg(not(target_arch = "wasm32"))]
        self.load();
        if self.entry(file).map(|e| &e.prefab) != Some(baseline) {
            return Err("This prefab changed in the library while the workspace was open. Revert to reload it, or Save as new to keep your work.".into());
        }
        self.edit(file, |p| *p = replacement)
    }

    pub(in crate::app) fn remove(&mut self, file: &str) -> Result<(), String> {
        self.store.delete(file)?;
        self.entries.retain(|entry| entry.file != file);
        if self.selected.as_deref() == Some(file) {
            self.selected = None;
        }
        if self.armed.as_ref().is_some_and(|armed| armed.file == file) {
            self.armed = None;
        }
        Ok(())
    }

    pub(in crate::app) fn duplicate(&mut self, file: &str) -> Result<String, String> {
        let Some(entry) = self.entry(file) else {
            return Err("that prefab is no longer in the library".into());
        };
        let mut copy = entry.prefab.clone();
        copy.name = format!("{} copy", copy.name);
        self.add(copy)
    }

    /// Add prefabs from a file's text (one prefab per file).
    pub(in crate::app) fn import_text(&mut self, text: &str) -> Result<String, String> {
        let prefab: Prefab = serde_json::from_str(text).map_err(|error| error.to_string())?;
        prefab.validate().map_err(|error| error.to_string())?;
        self.add(prefab)
    }

    pub(in crate::app) fn export_text(&self, file: &str) -> Option<(String, String)> {
        let entry = self.entry(file)?;
        let text = serde_json::to_string_pretty(&entry.prefab).ok()?;
        Some((format!("{}.prefab.json", self.store.stem_of(file)), text))
    }

    /// Capture a rectangle of the map into the library.
    pub(in crate::app) fn capture(
        &mut self,
        document: &osrs_edit::MapDocument,
        spec: CaptureSpec,
        name: &str,
    ) -> Result<String, String> {
        let name = if name.trim().is_empty() {
            format!("Capture {}x{}", spec.width, spec.height)
        } else {
            name.trim().to_owned()
        };
        let source = PrefabSource::Captured {
            tile_x: spec.origin.x,
            tile_y: spec.origin.y,
            plane: spec.base_plane.index().get(),
        };
        let prefab = capture_prefab(document, spec, name, source).map_err(describe)?;
        self.add(prefab)
    }

    /// Ask for a prefab file to add to the library.
    pub(in crate::app) fn start_import(&mut self) {
        self.import_task = Some(open_text("Import a prefab", "json"));
    }

    /// Ask where to save the prefab as a file.
    pub(in crate::app) fn start_export(&mut self, file: &str) {
        if let Some((name, text)) = self.export_text(file) {
            self.export_task = Some(save_text(&name, text));
        }
    }

    /// Finish file dialogs that have answered. True when the list changed.
    pub(in crate::app) fn poll_files(&mut self) -> bool {
        let mut changed = false;
        if let Some(task) = &mut self.import_task
            && let Some(result) = task.poll()
        {
            self.import_task = None;
            match result {
                Ok(Some(picked)) => match self.import_text(&picked.text) {
                    Ok(_) => {
                        self.notice = Some(format!("Imported {}", picked.name));
                        changed = true;
                    }
                    Err(error) => self.notice = Some(format!("{}: {error}", picked.name)),
                },
                Ok(None) => {}
                Err(error) => self.notice = Some(error),
            }
        }
        if let Some(task) = &mut self.export_task
            && let Some(result) = task.poll()
        {
            self.export_task = None;
            match result {
                Ok(Some(name)) => self.notice = Some(format!("Saved {name}")),
                Ok(None) => {}
                Err(error) => self.notice = Some(error),
            }
        }
        changed
    }

    pub(in crate::app) fn disarm(&mut self) -> bool {
        self.armed.take().is_some()
    }

    pub(in crate::app) fn arm(&mut self, file: &str) {
        let (turns, options) = self
            .armed
            .as_ref()
            .map_or((0, None), |armed| (armed.turns, Some(armed.clone())));
        self.armed = Some(Armed {
            file: file.to_owned(),
            turns,
            terrain: options.as_ref().is_none_or(|o| o.terrain),
            heights: options.as_ref().is_none_or(|o| o.heights),
            objects: options.as_ref().is_none_or(|o| o.objects),
            replace_objects: options.as_ref().is_none_or(|o| o.replace_objects),
        });
    }
}

fn describe(error: PrefabError) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests;
