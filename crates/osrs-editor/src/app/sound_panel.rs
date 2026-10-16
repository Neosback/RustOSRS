//! The Sound dock panel: which sounds the loaded map plays, which objects play them, and what
//! each one does, with a Play button on every sound. Data and arithmetic live in `crate::sound`,
//! rendered audio in `audio_runtime`; this draws pages over them and keeps the panel's own state.
//! It never changes the map.
//!
//! Pages: **Sounds** (one row per sound, with the objects that use it), **Objects** (one row per
//! object) and **Object** (everything about the object picked in either list or in the world).

mod pages;
pub(in crate::app) mod script;
mod world;

use super::EditorApp;
use crate::sound::{self, AreaSound, Scope};
use eframe::egui;

pub(super) use world::{draw_world, marker_click};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Page {
    #[default]
    Sounds,
    Objects,
    Object,
}

#[derive(Default)]
pub(super) struct SoundPanel {
    page: Page,
    filter: String,
    whole_map: bool,
    all_planes: bool,
    hide_in_world: bool,
    loop_playback: bool,
    /// Sounds whose objects are listed open on the Sounds page.
    expanded: std::collections::BTreeSet<u32>,
    /// The selected object: its region and loc index.
    selected: Option<((i32, i32), usize)>,
    cache: Option<Cache>,
}

struct Cache {
    key: (u64, Scope, Option<u8>),
    rows: Vec<AreaSound>,
    unresolved: usize,
}

impl SoundPanel {
    /// Open the Object page for one object (a click on its marker in the world does this).
    pub(super) fn open_object(&mut self, key: ((i32, i32), usize)) {
        self.selected = Some(key);
        self.page = Page::Object;
    }
}

/// Recompute the list when the loaded map, the scope or the plane changed.
pub(super) fn refresh(app: &mut EditorApp) {
    let scope = if app.ui.sound.whole_map {
        Scope::Loaded
    } else {
        Scope::Region(app.camera_region())
    };
    let plane = (!app.ui.sound.all_planes).then(|| app.interaction.view_plane());
    let key = (app.world.infos_version(), scope, plane);
    if app.ui.sound.cache.as_ref().is_some_and(|c| c.key == key) {
        return;
    }
    let (rows, unresolved) = sound::collect(app.world.infos(), scope, plane);
    app.ui.sound.cache = Some(Cache {
        key,
        rows,
        unresolved,
    });
}

/// "#1234 waterfall" or "#1234": RuneLite's name, else the installation's, else only the ID.
fn sound_label(app: &EditorApp, id: u32) -> String {
    match sound::name(id).or_else(|| app.names.book().and_then(|b| b.sound(id))) {
        Some(name) => format!("#{id} {name}"),
        None => format!("#{id}"),
    }
}

/// The cache's name for an object, else the installation's, else "object 123".
fn object_name(app: &EditorApp, row: &AreaSound) -> String {
    if !row.name.is_empty() {
        return row.name.clone();
    }
    match app.names.book().and_then(|b| b.object(row.object)) {
        Some(name) => name.to_owned(),
        None => format!("object {}", row.object),
    }
}

/// Whether `needle` (lower case) is in the object's or a sound's name or ID, counting the
/// installation's names too.
fn row_matches(app: &EditorApp, row: &AreaSound, needle: &str) -> bool {
    row.matches(needle)
        || object_name(app, row).to_lowercase().contains(needle)
        || row.sound.sounds().any(|id| {
            app.names
                .book()
                .and_then(|b| b.sound(id.get()))
                .is_some_and(|n| n.to_lowercase().contains(needle))
        })
}

fn plural(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

pub(super) fn show(app: &mut EditorApp, ui: &mut egui::Ui) {
    refresh(app);
    pages::show(app, ui);
}
