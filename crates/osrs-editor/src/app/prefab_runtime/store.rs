//! Where prefab files live: one `<name>.prefab.json` per prefab in a `prefabs` folder next to the
//! editor's settings (native). The browser build has no folder, so its library lasts the session
//! and moves in and out through import and export.

use super::Entry;
use osrs_edit::Prefab;
use std::path::PathBuf;

const SUFFIX: &str = ".prefab.json";

#[derive(Default)]
pub(super) struct Store {
    #[cfg(not(target_arch = "wasm32"))]
    directory: Option<PathBuf>,
}

/// A file-system safe stem for a prefab name.
fn slug(name: &str) -> String {
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
        "prefab".to_owned()
    } else {
        stem.chars().take(60).collect()
    }
}

impl Store {
    /// A store in `directory` (created when the first prefab is saved).
    #[cfg(all(test, not(target_arch = "wasm32")))]
    pub(super) fn in_directory(directory: PathBuf) -> Self {
        Self {
            directory: Some(directory),
        }
    }

    /// The default location: `prefabs/` beside the settings file.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn beside_settings() -> Self {
        Self {
            directory: crate::settings::path()
                .and_then(|file| file.parent().map(|parent| parent.join("prefabs"))),
        }
    }

    pub(super) fn directory(&self) -> Option<PathBuf> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.directory.clone()
        }
        #[cfg(target_arch = "wasm32")]
        {
            None
        }
    }

    pub(super) fn stem_of(&self, file: &str) -> String {
        file.strip_suffix(SUFFIX).unwrap_or(file).to_owned()
    }

    /// A file name for `name` that no entry in `taken` uses.
    pub(super) fn unique_file_name(&self, name: &str, taken: &[&str]) -> String {
        let stem = slug(name);
        let mut candidate = format!("{stem}{SUFFIX}");
        let mut counter = 2;
        while taken.contains(&candidate.as_str()) || self.exists(&candidate) {
            candidate = format!("{stem}-{counter}{SUFFIX}");
            counter += 1;
        }
        candidate
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn exists(&self, file: &str) -> bool {
        self.directory
            .as_ref()
            .is_some_and(|directory| directory.join(file).exists())
    }

    #[cfg(target_arch = "wasm32")]
    fn exists(&self, _file: &str) -> bool {
        false
    }

    /// Every readable prefab file, and a line for each that was not.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn read_all(&self) -> (Vec<Entry>, Vec<String>) {
        let mut entries = Vec::new();
        let mut problems = Vec::new();
        let Some(directory) = &self.directory else {
            return (entries, problems);
        };
        let Ok(read) = std::fs::read_dir(directory) else {
            return (entries, problems);
        };
        let mut files: Vec<_> = read
            .filter_map(Result::ok)
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|name| name.ends_with(SUFFIX))
            .collect();
        files.sort();
        for file in files {
            let parsed = std::fs::read_to_string(directory.join(&file))
                .map_err(|error| error.to_string())
                .and_then(|text| {
                    serde_json::from_str::<Prefab>(&text).map_err(|error| error.to_string())
                })
                .and_then(|prefab| {
                    prefab.validate().map_err(|error| error.to_string())?;
                    Ok(prefab)
                });
            match parsed {
                Ok(prefab) => entries.push(Entry { file, prefab }),
                Err(error) => problems.push(format!("{file}: {error}")),
            }
        }
        (entries, problems)
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) fn read_all(&self) -> (Vec<Entry>, Vec<String>) {
        (Vec::new(), Vec::new())
    }

    /// Write one prefab (atomically: a temporary file, then a rename).
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn write(&self, file: &str, prefab: &Prefab) -> Result<(), String> {
        let Some(directory) = &self.directory else {
            return Ok(());
        };
        std::fs::create_dir_all(directory).map_err(|e| format!("{}: {e}", directory.display()))?;
        let text = serde_json::to_string_pretty(prefab).map_err(|e| e.to_string())?;
        let target = directory.join(file);
        let temporary = directory.join(format!("{file}.tmp"));
        std::fs::write(&temporary, text).map_err(|e| format!("{}: {e}", temporary.display()))?;
        std::fs::rename(&temporary, &target).map_err(|e| format!("{}: {e}", target.display()))
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) fn write(&self, _file: &str, _prefab: &Prefab) -> Result<(), String> {
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn delete(&self, file: &str) -> Result<(), String> {
        let Some(directory) = &self.directory else {
            return Ok(());
        };
        match std::fs::remove_file(directory.join(file)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("{file}: {error}")),
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) fn delete(&self, _file: &str) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::slug;

    #[test]
    fn names_become_safe_file_stems() {
        assert_eq!(
            slug("Lumbridge Castle (north wing)"),
            "lumbridge-castle-north-wing"
        );
        assert_eq!(slug("  ../../etc/passwd "), "etc-passwd");
        assert_eq!(slug("???"), "prefab");
        assert_eq!(slug(&"a".repeat(200)).len(), 60);
    }
}
