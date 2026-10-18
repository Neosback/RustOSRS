//! The prefab library and the state of placing one: the saved prefabs, their files, the search
//! text, the selection and the armed placement gesture.
//!
//! Plain data and file IO only. The panel, the viewport gesture and the edit commit live
//! elsewhere and talk to this owner through narrow operations; nothing here touches the map.

mod build;
mod store;

use crate::app::npc_runtime::{OpenTask, SaveTask, open_text, save_text};
pub(in crate::app) use build::{ARCHETYPES, BuildState, MAX_SITE, STYLES};
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
    pub(in crate::app) name: String,
    pub(in crate::app) planes: u8,
    pub(in crate::app) terrain: bool,
    pub(in crate::app) heights: bool,
    pub(in crate::app) objects: bool,
}

impl Default for CaptureForm {
    fn default() -> Self {
        Self {
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
    pub(in crate::app) gallery: crate::app::prefab_gallery::Gallery,
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
    pub(in crate::app) fn add(&mut self, prefab: Prefab) -> Result<String, String> {
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
        edited.validate().map_err(|error| error.to_string())?;
        self.store.write(file, &edited)?;
        entry.prefab = edited;
        Ok(())
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
