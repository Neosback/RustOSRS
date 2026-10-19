//! A folder of JSON files beside the settings: one kind of file per folder suffix, a trash
//! folder instead of deleting, and atomic saves. Used for path lessons and path models.

use std::path::PathBuf;

pub(super) struct FileStore<T> {
    directory: Option<PathBuf>,
    suffix: &'static str,
    parse: fn(&str) -> Result<T, String>,
    render: fn(&T) -> Result<String, String>,
    /// Newest first: the key to sort by, descending.
    order: fn(&T) -> String,
    pub(super) entries: Vec<(String, T)>,
    pub(super) problems: Vec<String>,
    loaded: bool,
}

/// A file-system safe stem for a name.
pub(super) fn slug(name: &str, fallback: &str) -> String {
    let mut stem: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    while stem.contains("--") {
        stem = stem.replace("--", "-");
    }
    let stem = stem.trim_matches('-');
    if stem.is_empty() {
        fallback.to_owned()
    } else {
        stem.chars().take(48).collect()
    }
}

impl<T> FileStore<T> {
    pub(super) fn new(
        folder: &str,
        suffix: &'static str,
        parse: fn(&str) -> Result<T, String>,
        render: fn(&T) -> Result<String, String>,
        order: fn(&T) -> String,
    ) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let directory =
            crate::settings::path().and_then(|file| file.parent().map(|p| p.join(folder)));
        #[cfg(target_arch = "wasm32")]
        let directory = {
            let _ = folder;
            None
        };
        Self {
            directory,
            suffix,
            parse,
            render,
            order,
            entries: Vec::new(),
            problems: Vec::new(),
            loaded: false,
        }
    }

    #[cfg(test)]
    pub(super) fn in_directory(mut self, directory: PathBuf) -> Self {
        self.directory = Some(directory);
        self
    }

    pub(super) fn ensure_loaded(&mut self) {
        if !self.loaded {
            self.reload();
        }
    }

    pub(super) fn reload(&mut self) {
        self.loaded = true;
        self.entries.clear();
        self.problems.clear();
        let Some(directory) = &self.directory else {
            return;
        };
        let Ok(read) = std::fs::read_dir(directory) else {
            return;
        };
        let mut files: Vec<String> = read
            .filter_map(Result::ok)
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| n.ends_with(self.suffix))
            .collect();
        files.sort();
        for file in files {
            match std::fs::read_to_string(directory.join(&file))
                .map_err(|e| e.to_string())
                .and_then(|text| (self.parse)(&text))
            {
                Ok(item) => self.entries.push((file, item)),
                Err(error) => self.problems.push(format!("{file}: {error}")),
            }
        }
        let order = self.order;
        self.entries
            .sort_by(|a, b| order(&b.1).cmp(&order(&a.1)).then(a.0.cmp(&b.0)));
    }

    /// A stem no file uses yet.
    pub(super) fn free_id(&mut self, name: &str, fallback: &str) -> String {
        self.ensure_loaded();
        let stem = slug(name, fallback);
        let taken = |id: &str| {
            self.entries
                .iter()
                .any(|(f, _)| f == &format!("{id}{}", self.suffix))
        };
        if !taken(&stem) {
            return stem;
        }
        (2..)
            .map(|n| format!("{stem}-{n}"))
            .find(|id| !taken(id))
            .unwrap_or(stem)
    }

    /// Save under `id` (atomically) and list it again. Returns the file name.
    pub(super) fn save(&mut self, id: &str, item: &T) -> Result<String, String> {
        let file = format!("{id}{}", self.suffix);
        if let Some(directory) = &self.directory {
            std::fs::create_dir_all(directory)
                .map_err(|e| format!("{}: {e}", directory.display()))?;
            let temporary = directory.join(format!("{file}.tmp"));
            std::fs::write(&temporary, (self.render)(item)?).map_err(|e| e.to_string())?;
            std::fs::rename(&temporary, directory.join(&file)).map_err(|e| e.to_string())?;
            self.reload();
        } else if let Some(at) = self.entries.iter().position(|(f, _)| f == &file) {
            // No folder (the web build): keep the list in memory.
            let _ = at;
        }
        Ok(file)
    }

    /// Change one entry and write it back to its own file.
    pub(super) fn edit(&mut self, file: &str, change: impl FnOnce(&mut T)) -> Result<(), String> {
        let at = self
            .entries
            .iter()
            .position(|(f, _)| f == file)
            .ok_or("that file is not in the list")?;
        change(&mut self.entries[at].1);
        if let Some(directory) = &self.directory {
            let text = (self.render)(&self.entries[at].1)?;
            let temporary = directory.join(format!("{file}.tmp"));
            std::fs::write(&temporary, text).map_err(|e| e.to_string())?;
            std::fs::rename(&temporary, directory.join(file)).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Add to the list without a file (when there is no folder).
    pub(super) fn keep(&mut self, file: String, item: T) {
        if self.directory.is_none() {
            self.entries.retain(|(f, _)| *f != file);
            self.entries.insert(0, (file, item));
        }
    }

    /// Move a file to the trash folder; it stays on disk.
    pub(super) fn trash(&mut self, file: &str) -> Result<(), String> {
        if let Some(directory) = &self.directory {
            let bin = directory.join(".trash");
            std::fs::create_dir_all(&bin).map_err(|e| e.to_string())?;
            std::fs::rename(directory.join(file), bin.join(file)).map_err(|e| e.to_string())?;
            self.reload();
        } else {
            self.entries.retain(|(f, _)| f != file);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_are_saved_listed_trashed_and_numbered() {
        let dir = std::env::temp_dir().join(format!("rustosrs-store-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut store: FileStore<String> = FileStore::new(
            "x",
            ".note.json",
            |t| Ok(t.to_owned()),
            |t| Ok(t.clone()),
            |t| t.clone(),
        )
        .in_directory(dir.clone());
        assert_eq!(store.free_id("Hello there!", "item"), "hello-there");
        store
            .save("hello-there", &"a".to_owned())
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(store.free_id("Hello there!", "item"), "hello-there-2");
        store
            .save("other", &"b".to_owned())
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(store.entries.len(), 2);
        assert_eq!(store.entries[0].1, "b", "newest (largest key) first");
        store
            .trash("hello-there.note.json")
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(store.entries.len(), 1);
        assert!(dir.join(".trash").join("hello-there.note.json").exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
