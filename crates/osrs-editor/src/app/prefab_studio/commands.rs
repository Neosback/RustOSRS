//! What the building window can be asked to do from outside its own frame: open a building,
//! switch to a tab, or run a scene command (scripts and panels send these).
use super::*;

pub(in crate::app) enum SceneCommand {
    Save,
    SaveAsNew,
    Close,
    Revert,
    Report,
    Demonstrate,
    PlayReplay,
    #[cfg(not(target_arch = "wasm32"))]
    PolicyReplay(Box<(osrs_lab::LabTask, Vec<osrs_lab::Action>)>),
    /// Open the Workshop (in the main window) on a tab, with this scene's building picked.
    Workshop(crate::app::workshop::Tab),
}

/// The former gallery entry opens a real editing workspace, including for an empty library.
pub(in crate::app) fn open_library(app: &mut EditorApp) {
    let file = app
        .prefabs
        .selected
        .clone()
        .filter(|f| app.prefabs.entry(f).is_some())
        .or_else(|| app.prefabs.entries().first().map(|e| e.file.clone()));
    let result = file.map_or_else(|| app.prefabs.create_blank_scene(), Ok);
    match result {
        Ok(file) => request(app, file),
        Err(error) => app.prefabs.say(error),
    }
}

pub(in crate::app) fn request(app: &mut EditorApp, file: String) {
    app.prefabs.studio_request = Some(file);
}
