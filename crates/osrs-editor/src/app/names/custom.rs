//! The name book: names people give objects, kept per object id (`osrs_core::object_names`) in
//! `names/objects.names.json` beside the settings, so a name given in one building shows in all.

use osrs_core::object_names::{ObjectNames, Resolved, Witness};
use std::path::PathBuf;

#[derive(Default)]
pub(in crate::app) struct CustomNames {
    book: ObjectNames,
    path: Option<PathBuf>,
    /// Bumped on every change, so background work can tell its names are out of date.
    generation: u64,
    problem: Option<String>,
}

impl CustomNames {
    /// The default file, beside the settings (native); the browser build keeps names for the
    /// session only.
    pub(in crate::app) fn open(build: u32) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let path = crate::settings::path()
            .and_then(|file| file.parent().map(|p| p.join("names/objects.names.json")));
        #[cfg(target_arch = "wasm32")]
        let path = None;
        Self::at(path, build)
    }

    fn at(path: Option<PathBuf>, build: u32) -> Self {
        let mut names = Self {
            book: ObjectNames::new(build),
            path,
            generation: 0,
            problem: None,
        };
        names.read();
        names
    }

    #[cfg(test)]
    pub(in crate::app) fn in_directory(directory: PathBuf, build: u32) -> Self {
        Self::at(Some(directory.join("objects.names.json")), build)
    }

    fn read(&mut self) {
        let Some(path) = &self.path else { return };
        let Ok(text) = std::fs::read_to_string(path) else {
            return;
        };
        match osrs_edit::name_book_from_json(&text) {
            Ok(book) => self.book = book,
            // A file that cannot be read is kept as it is; names added now go to a fresh book.
            Err(error) => {
                self.problem = Some(format!("The saved names could not be read ({error})."));
            }
        }
    }

    pub(in crate::app) fn book(&self) -> &ObjectNames {
        &self.book
    }

    pub(in crate::app) fn generation(&self) -> u64 {
        self.generation
    }

    pub(in crate::app) fn problem(&self) -> Option<&str> {
        self.problem.as_deref()
    }

    pub(in crate::app) fn resolve(
        &self,
        id: u32,
        cache_name: Option<&str>,
        symbol: Option<&str>,
    ) -> Resolved {
        self.book.resolve(id, cache_name, symbol)
    }

    /// Change the book and write it. A failed write is shown, never lost silently.
    pub(in crate::app) fn change(&mut self, edit: impl FnOnce(&mut ObjectNames)) {
        edit(&mut self.book);
        self.generation += 1;
        self.save();
    }

    pub(in crate::app) fn name_object(&mut self, id: u32, name: &str, witness: Witness) {
        self.change(|book| book.set_name(id, name, witness));
    }

    fn save(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let Some(path) = &self.path else { return };
            let result = osrs_edit::name_book_to_json(&self.book).and_then(|text| {
                if let Some(folder) = path.parent() {
                    std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
                }
                // Write beside the file and rename, so a crash never leaves half a book.
                let temporary = path.with_extension("json.tmp");
                std::fs::write(&temporary, text).map_err(|e| e.to_string())?;
                std::fs::rename(&temporary, path).map_err(|e| e.to_string())
            });
            self.problem = result
                .err()
                .map(|e| format!("The names could not be saved ({e})."));
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use osrs_core::object_names::NameSource;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("rustosrs-names-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn witness() -> Witness {
        Witness {
            cache_name: "null".into(),
            symbol: "loc.00".into(),
        }
    }

    #[test]
    fn a_name_survives_a_restart_and_shows_up_for_every_use_of_the_object() {
        let dir = scratch("restart");
        let mut names = CustomNames::in_directory(dir.clone(), 240);
        names.name_object(1902, "Brick wall", witness());
        assert_eq!(names.generation(), 1);
        let again = CustomNames::in_directory(dir.clone(), 240);
        let resolved = again.resolve(1902, Some("null"), Some("loc.00"));
        assert_eq!(
            (resolved.text.as_str(), resolved.source),
            ("Brick wall", NameSource::You)
        );
        // Another building using the same object id gets the same name.
        assert_eq!(again.resolve(1902, None, None).text, "Brick wall");
        assert_eq!(again.resolve(1903, None, None).source, NameSource::Unnamed);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_damaged_file_is_reported_and_replaced_by_the_next_save_without_a_crash() {
        let dir = scratch("damaged");
        std::fs::create_dir_all(&dir).unwrap_or_default();
        std::fs::write(dir.join("objects.names.json"), "{ not json").unwrap_or_default();
        let mut names = CustomNames::in_directory(dir.clone(), 240);
        assert!(names.problem().is_some());
        names.name_object(5, "Door", witness());
        assert!(names.problem().is_none(), "the save worked");
        assert_eq!(
            CustomNames::in_directory(dir.clone(), 240)
                .resolve(5, None, None)
                .text,
            "Door"
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
