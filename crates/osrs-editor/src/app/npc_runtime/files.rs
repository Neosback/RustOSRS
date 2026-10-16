//! Text files in and out: native dialogs resolve immediately, browser pickers later, both
//! through one pollable task so the panel never branches on the platform.
use std::{cell::RefCell, rc::Rc};

pub(in crate::app) struct FileTask<T>(Rc<RefCell<Option<T>>>);

impl<T> FileTask<T> {
    pub fn poll(&mut self) -> Option<T> {
        self.0.borrow_mut().take()
    }
}

/// A chosen text file: its name and contents.
pub(in crate::app) struct PickedText {
    pub name: String,
    pub text: String,
}

pub(in crate::app) type OpenTask = FileTask<Result<Option<PickedText>, String>>;
pub(in crate::app) type SaveTask = FileTask<Result<Option<String>, String>>;

#[cfg(not(target_arch = "wasm32"))]
pub(in crate::app) fn open_text(title: &str, extension: &str) -> OpenTask {
    let result = rfd::FileDialog::new()
        .set_title(title)
        .add_filter(extension, &[extension])
        .pick_file()
        .map(|path| {
            std::fs::read_to_string(&path)
                .map(|text| PickedText {
                    name: path
                        .file_name()
                        .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
                    text,
                })
                .map_err(|e| format!("{}: {e}", path.display()))
        });
    FileTask(Rc::new(RefCell::new(Some(result.transpose()))))
}

#[cfg(not(target_arch = "wasm32"))]
pub(in crate::app) fn save_text(name: &str, contents: String) -> SaveTask {
    let result = rfd::FileDialog::new()
        .set_file_name(name)
        .save_file()
        .map(|path| {
            std::fs::write(&path, contents)
                .map(|()| path.display().to_string())
                .map_err(|e| format!("{}: {e}", path.display()))
        });
    FileTask(Rc::new(RefCell::new(Some(result.transpose()))))
}

#[cfg(target_arch = "wasm32")]
pub(in crate::app) fn open_text(title: &str, extension: &str) -> OpenTask {
    let slot = Rc::new(RefCell::new(None));
    let out = Rc::clone(&slot);
    let (title, extension) = (title.to_owned(), extension.to_owned());
    wasm_bindgen_futures::spawn_local(async move {
        let handle = rfd::AsyncFileDialog::new()
            .set_title(&title)
            .add_filter(&extension, &[extension.as_str()])
            .pick_file()
            .await;
        let result = match handle {
            None => Ok(None),
            Some(handle) => {
                let bytes = handle.read().await;
                String::from_utf8(bytes)
                    .map(|text| {
                        Some(PickedText {
                            name: handle.file_name(),
                            text,
                        })
                    })
                    .map_err(|e| format!("{}: {e}", handle.file_name()))
            }
        };
        *out.borrow_mut() = Some(result);
    });
    FileTask(slot)
}

#[cfg(target_arch = "wasm32")]
pub(in crate::app) fn save_text(name: &str, contents: String) -> SaveTask {
    let slot = Rc::new(RefCell::new(None));
    let out = Rc::clone(&slot);
    let name = name.to_owned();
    wasm_bindgen_futures::spawn_local(async move {
        let handle = rfd::AsyncFileDialog::new()
            .set_file_name(&name)
            .save_file()
            .await;
        let result = match handle {
            None => Ok(None),
            Some(handle) => handle
                .write(contents.as_bytes())
                .await
                .map(|()| Some(handle.file_name()))
                .map_err(|e| e.to_string()),
        };
        *out.borrow_mut() = Some(result);
    });
    FileTask(slot)
}
