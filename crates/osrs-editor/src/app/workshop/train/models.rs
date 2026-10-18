//! The trained models on disk: `models/<name>.model.json` beside the settings, with a trash folder
//! so a deleted model can be brought back by hand, and a feedback log of ratings.

use osrs_lab::Model;
use std::path::PathBuf;

const SUFFIX: &str = ".model.json";

#[derive(Default)]
pub(super) struct ModelList {
    directory: Option<PathBuf>,
    /// `(file name, model)`, newest first.
    pub(super) entries: Vec<(String, Model)>,
    pub(super) problems: Vec<String>,
    loaded: bool,
}

/// A file-system safe name for a model.
pub(super) fn slug(name: &str) -> String {
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
        "model".to_owned()
    } else {
        stem.chars().take(48).collect()
    }
}

impl ModelList {
    pub(super) fn beside_settings() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let directory = crate::settings::path()
            .and_then(|file| file.parent().map(|parent| parent.join("models")));
        #[cfg(target_arch = "wasm32")]
        let directory = None;
        Self {
            directory,
            ..Self::default()
        }
    }

    #[cfg(test)]
    pub(super) fn in_directory(directory: PathBuf) -> Self {
        Self {
            directory: Some(directory),
            ..Self::default()
        }
    }

    /// Read the folder (once; call [`Self::reload`] to read again).
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
            .filter(|n| n.ends_with(SUFFIX))
            .collect();
        files.sort();
        for file in files {
            match std::fs::read_to_string(directory.join(&file))
                .map_err(|e| e.to_string())
                .and_then(|text| Model::from_json(&text))
            {
                Ok(model) => self.entries.push((file, model)),
                Err(error) => self.problems.push(format!("{file}: {error}")),
            }
        }
        self.entries
            .sort_by(|a, b| b.1.created.cmp(&a.1.created).then(a.0.cmp(&b.0)));
    }

    /// A model id and file stem that no saved model uses.
    pub(super) fn free_id(&mut self, name: &str) -> String {
        self.ensure_loaded();
        let stem = slug(name);
        let taken = |id: &str| {
            self.entries
                .iter()
                .any(|(f, _)| f == &format!("{id}{SUFFIX}"))
        };
        if !taken(&stem) {
            return stem;
        }
        (2..)
            .map(|n| format!("{stem}-{n}"))
            .find(|id| !taken(id))
            .unwrap_or(stem)
    }

    /// Save a model (atomically) and list it.
    pub(super) fn save(&mut self, model: &Model) -> Result<String, String> {
        let file = format!("{}{SUFFIX}", model.id);
        if let Some(directory) = &self.directory {
            std::fs::create_dir_all(directory)
                .map_err(|e| format!("{}: {e}", directory.display()))?;
            let temporary = directory.join(format!("{file}.tmp"));
            std::fs::write(&temporary, model.to_json()?).map_err(|e| e.to_string())?;
            std::fs::rename(&temporary, directory.join(&file)).map_err(|e| e.to_string())?;
        }
        self.reload();
        Ok(file)
    }

    /// Move a model to the trash folder; the file stays on disk.
    pub(super) fn trash(&mut self, file: &str) -> Result<(), String> {
        if let Some(directory) = &self.directory {
            let bin = directory.join(".trash");
            std::fs::create_dir_all(&bin).map_err(|e| e.to_string())?;
            std::fs::rename(directory.join(file), bin.join(file)).map_err(|e| e.to_string())?;
        }
        self.reload();
        Ok(())
    }

    /// Add one line to the model's ratings file.
    pub(super) fn rate(&self, model_id: &str, line: &str) -> Result<(), String> {
        use std::io::Write;
        let Some(directory) = &self.directory else {
            return Ok(());
        };
        std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(directory.join(format!("{model_id}.feedback.jsonl")))
            .map_err(|e| e.to_string())?;
        writeln!(file, "{line}").map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_become_safe_ids_and_taken_ones_get_a_number() {
        assert_eq!(slug("Brick style!"), "brick-style");
        assert_eq!(slug("../etc"), "etc");
        assert_eq!(slug("???"), "model");
        let dir = std::env::temp_dir().join(format!("rustosrs-models-{}", std::process::id()));
        let mut list = ModelList::in_directory(dir.clone());
        assert_eq!(list.free_id("My style"), "my-style");
        let _ = std::fs::remove_dir_all(dir);
    }
}
