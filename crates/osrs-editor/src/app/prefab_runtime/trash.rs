//! Deleting prefabs without losing them: files move to a `.trash` folder inside the library and
//! the last deletion can be taken back. Nothing is erased until the folder is emptied by hand.

use super::PrefabRuntime;
use std::collections::BTreeSet;

/// What one deletion moved: `(file in the library, file in the trash)`.
pub(in crate::app) type Trashed = Vec<(String, String)>;

#[derive(Default)]
pub(in crate::app) struct TrashState {
    /// Files ticked in the list (survives switching tabs).
    pub(in crate::app) checked: BTreeSet<String>,
    /// The deletion waiting for a yes: the files it would move. One at a time, so an old
    /// confirmation can never be answered by mistake.
    pub(in crate::app) asking: Option<Vec<String>>,
    /// Deletions that can still be undone, newest last.
    undo: Vec<Trashed>,
}

impl TrashState {
    pub(in crate::app) fn can_restore(&self) -> bool {
        !self.undo.is_empty()
    }
}

impl PrefabRuntime {
    /// Ask before deleting `files` (the question is shown by the list).
    pub(in crate::app) fn ask_to_delete(&mut self, files: Vec<String>) {
        self.trash.asking = (!files.is_empty()).then_some(files);
    }

    /// Move `files` to the trash. Returns how many moved; the first problem stops it.
    pub(in crate::app) fn move_to_trash(&mut self, files: &[String]) -> Result<usize, String> {
        let mut moved: Trashed = Vec::new();
        let mut result = Ok(());
        for file in files {
            match self.store.trash(file) {
                Ok(name) => moved.push((file.clone(), name)),
                Err(error) => {
                    result = Err(error);
                    break;
                }
            }
        }
        let count = moved.len();
        for (file, _) in &moved {
            self.entries.retain(|entry| &entry.file != file);
            self.trash.checked.remove(file);
            if self.selected.as_deref() == Some(file.as_str()) {
                self.selected = None;
            }
            if self.armed.as_ref().is_some_and(|armed| &armed.file == file) {
                self.armed = None;
            }
        }
        if !moved.is_empty() {
            self.trash.undo.push(moved);
        }
        result.map(|()| count)
    }

    /// Put back what the last deletion moved. Returns the files restored.
    pub(in crate::app) fn restore_last(&mut self) -> Result<Vec<String>, String> {
        let Some(batch) = self.trash.undo.pop() else {
            return Err("Nothing to bring back.".into());
        };
        let mut restored = Vec::new();
        for (file, name) in batch {
            restored.push(self.store.restore(&name, &file)?);
        }
        self.load();
        if let Some(first) = restored.first() {
            self.selected = Some(first.clone());
        }
        Ok(restored)
    }
}
