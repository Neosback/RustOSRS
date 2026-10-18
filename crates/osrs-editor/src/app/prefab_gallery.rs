//! The prefab gallery: a window of its own for browsing every saved prefab and seeing the selected
//! one in 3D, with the real renderer, from every side.
//!
//! The list side filters and sorts the library the Prefabs panel already owns (`PrefabRuntime`);
//! the view side is the [`stage`]: an isolated editor over an empty map square that the prefab is
//! placed on. Nothing here edits the map being worked on; "Place on the map" arms the normal
//! placement gesture.

mod list;
pub(in crate::app) mod stage;

use crate::app::{EditorApp, prefab_runtime::Page};
use crate::settings::RoofChoice;
use eframe::egui;
use osrs_core::ids::ObjectId;
use osrs_edit::{LocKind, Prefab};
use stage::Stage;
use std::collections::HashMap;

/// Size of a prefab's still, in pixels.
const STILL: (u32, u32) = (320, 240);

/// A prefab's 3D still and what it was made from (a changed prefab needs a new one).
pub(super) struct Thumb {
    key: u64,
    pub(super) texture: egui::TextureHandle,
}

/// How the list is ordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Sort {
    #[default]
    Name,
    /// Largest footprint first.
    Size,
    /// Most objects first.
    Objects,
}

/// Which prefabs the list shows besides the search text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum SourceFilter {
    #[default]
    All,
    Captured,
    Generated,
    Authored,
}

pub(in crate::app) struct Gallery {
    pub(in crate::app) open: bool,
    stage: Option<Box<Stage>>,
    /// Set when the stage could not be made (shown instead of the view).
    stage_error: Option<String>,
    tag: Option<String>,
    source: SourceFilter,
    sort: Sort,
    turns: u8,
    roofs: RoofChoice,
    spin: bool,
    /// Stills by file name.
    thumbs: HashMap<String, Thumb>,
    /// Files whose stills are being rendered one after the other (shown on the stage meanwhile).
    queue: Vec<String>,
}

impl Default for Gallery {
    fn default() -> Self {
        Self {
            open: false,
            stage: None,
            stage_error: None,
            tag: None,
            source: SourceFilter::default(),
            sort: Sort::default(),
            turns: 0,
            roofs: RoofChoice::Visible,
            spin: true,
            thumbs: HashMap::new(),
            queue: Vec::new(),
        }
    }
}

const LIST_WIDTH: f32 = 330.0;

/// Draw the window when it is open. Called once per frame.
pub(super) fn window(app: &mut EditorApp, ctx: &egui::Context) {
    if !app.prefabs.gallery.open {
        // Closing frees the stage's world and GPU resources.
        app.prefabs.gallery.stage = None;
        app.prefabs.gallery.stage_error = None;
        return;
    }
    // The stage is taken out so it and the editor can both be borrowed while drawing.
    let mut stage = app.prefabs.gallery.stage.take();
    if stage.is_none() && app.prefabs.gallery.stage_error.is_none() && app.world.is_ready() {
        match Stage::new(app, ctx) {
            Ok(made) => stage = Some(Box::new(made)),
            Err(error) => app.prefabs.gallery.stage_error = Some(error),
        }
    }
    if let Some(stage) = stage.as_mut() {
        stage.step(ctx);
        sync(app, stage);
        capture(app, stage, ctx);
    }
    let mut open = true;
    egui::Window::new("Prefab gallery")
        .id(egui::Id::new("prefab-gallery"))
        .open(&mut open)
        .default_size([1100.0, 700.0])
        .min_size([760.0, 420.0])
        .resizable(true)
        .show(ctx, |ui| {
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(LIST_WIDTH, ui.available_height()),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| list::show(app, ui),
                );
                ui.separator();
                ui.vertical(|ui| view(app, stage.as_mut(), ui, ctx));
            });
        });
    app.prefabs.gallery.open = open;
    app.prefabs.gallery.stage = stage;
    if open {
        ctx.request_repaint();
    }
}

/// What a prefab's still depends on.
fn still_key(prefab: &Prefab) -> u64 {
    let mut key = u64::from(prefab.width) << 32 | u64::from(prefab.height) << 16;
    key ^=
        (prefab.objects.len() as u64).rotate_left(7) ^ (prefab.tiles.len() as u64).rotate_left(21);
    for object in &prefab.objects {
        key = key.wrapping_mul(0x100_0000_01b3).wrapping_add(
            u64::from(object.id) << 8 ^ u64::from(object.x) << 24 ^ u64::from(object.y),
        );
    }
    key
}

/// Whether `file` has a still that matches its prefab.
pub(super) fn has_still(app: &EditorApp, file: &str) -> bool {
    app.prefabs.entry(file).is_some_and(|entry| {
        app.prefabs
            .gallery
            .thumbs
            .get(file)
            .is_some_and(|thumb| thumb.key == still_key(&entry.prefab))
    })
}

/// The 3D still of `file`, when it has a current one.
pub(in crate::app) fn still_of(app: &EditorApp, file: &str) -> Option<egui::TextureHandle> {
    has_still(app, file)
        .then(|| {
            app.prefabs
                .gallery
                .thumbs
                .get(file)
                .map(|t| t.texture.clone())
        })
        .flatten()
}

/// Render a still of every prefab that has none, one after the other.
pub(in crate::app) fn queue_missing_stills(app: &mut EditorApp) {
    let missing: Vec<String> = app
        .prefabs
        .entries()
        .iter()
        .filter(|entry| !has_still(app, &entry.file))
        .map(|entry| entry.file.clone())
        .collect();
    app.prefabs.gallery.queue = missing;
}

/// Keep the stage showing the selected prefab (or the next one waiting for its still).
fn sync(app: &mut EditorApp, stage: &mut Stage) {
    while let Some(first) = app.prefabs.gallery.queue.first().cloned() {
        if app.prefabs.entry(&first).is_none() || has_still(app, &first) {
            app.prefabs.gallery.queue.remove(0);
        } else {
            return show_file(app, stage, &first, 0);
        }
    }
    let Some(file) = app.prefabs.selected.clone() else {
        return;
    };
    let turns = app.prefabs.gallery.turns;
    show_file(app, stage, &file, turns);
}

fn show_file(app: &mut EditorApp, stage: &mut Stage, file: &str, turns: u8) {
    let file = file.to_owned();
    let Some(prefab) = app.prefabs.entry(&file).map(|entry| entry.prefab.clone()) else {
        return;
    };
    let size_of = |id: ObjectId, _: LocKind| app.placer.object_size(id.get());
    stage.show(&file, &prefab, turns, &size_of);
}

/// Take the still of the prefab on the stage when it is complete and does not have one yet.
fn capture(app: &mut EditorApp, stage: &Stage, ctx: &egui::Context) {
    if !stage.settled() {
        return;
    }
    let Some((file, turns)) = stage.shown() else {
        return;
    };
    let file = file.to_owned();
    // A turned prefab is not what the still stands for.
    if turns != 0 || has_still(app, &file) {
        return;
    }
    let Some(entry) = app.prefabs.entry(&file) else {
        return;
    };
    let key = still_key(&entry.prefab);
    let Some(rgba) = stage.still(STILL.0, STILL.1) else {
        return;
    };
    let image =
        egui::ColorImage::from_rgba_unmultiplied([STILL.0 as usize, STILL.1 as usize], &rgba);
    let texture = ctx.load_texture(
        format!("prefab-still-{file}"),
        image,
        egui::TextureOptions::LINEAR,
    );
    app.prefabs
        .gallery
        .thumbs
        .insert(file, Thumb { key, texture });
}

/// The right side: the 3D view and what can be done with it.
fn view(
    app: &mut EditorApp,
    stage: Option<&mut Box<Stage>>,
    ui: &mut egui::Ui,
    ctx: &egui::Context,
) {
    if let Some(error) = app.prefabs.gallery.stage_error.clone() {
        ui.colored_label(ui.visuals().error_fg_color, error);
        return;
    }
    let Some(stage) = stage else {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.weak("Opening the stage...");
        });
        return;
    };
    let selected = app.prefabs.selected.clone();
    let Some(entry) = selected.as_deref().and_then(|file| app.prefabs.entry(file)) else {
        ui.weak("Pick a prefab on the left.");
        return;
    };
    let (name, summary) = (entry.prefab.name.clone(), entry.prefab.summary());
    let file = entry.file.clone();
    let planes = entry.prefab.planes;
    ui.horizontal(|ui| {
        ui.heading(&name);
        ui.weak(summary);
    });
    controls(app, stage, ui, planes);
    let side = ui.available_size() - egui::vec2(0.0, 36.0);
    let (rect, response) = ui.allocate_exact_size(side, egui::Sense::click_and_drag());
    draw_stage(app, stage, ui, ctx, rect, &response);
    ui.horizontal(|ui| {
        if let Some(problem) = &stage.problem {
            ui.colored_label(ui.visuals().error_fg_color, problem);
        } else if !stage.ready() {
            ui.spinner();
            ui.weak("Laying the stage...");
        } else {
            ui.weak("Right drag orbits, Shift + right drag pans, scroll zooms.");
        }
        if ui
            .button(format!(
                "{} Place on the map",
                egui_phosphor::regular::CURSOR_CLICK
            ))
            .on_hover_text("Then click the map. R turns it, Esc stops")
            .clicked()
        {
            crate::app::prefab_panel::arm(app, &file);
            app.prefabs.page = Page::Library;
        }
    });
}

fn controls(app: &mut EditorApp, stage: &mut Stage, ui: &mut egui::Ui, planes: u8) {
    let gallery = &mut app.prefabs.gallery;
    ui.horizontal_wrapped(|ui| {
        if ui
            .button(egui_phosphor::regular::ARROW_COUNTER_CLOCKWISE)
            .on_hover_text("Turn the prefab left")
            .clicked()
        {
            gallery.turns = (gallery.turns + 3) & 3;
        }
        if ui
            .button(egui_phosphor::regular::ARROW_CLOCKWISE)
            .on_hover_text("Turn the prefab right")
            .clicked()
        {
            gallery.turns = (gallery.turns + 1) & 3;
        }
        ui.separator();
        ui.label("Roofs");
        for choice in [RoofChoice::Hidden, RoofChoice::Visible] {
            ui.selectable_value(&mut gallery.roofs, choice, choice.label());
        }
        if planes > 1 {
            ui.separator();
            ui.label("Floor");
            let viewer = stage.viewer();
            let mut plane = viewer.interaction.view_plane();
            for p in 0..planes {
                ui.selectable_value(&mut plane, p, p.to_string());
            }
            viewer.interaction.set_view_plane(plane);
        }
        ui.separator();
        crate::theme::toggle(ui, &mut gallery.spin, "Turntable");
    });
}

fn draw_stage(
    app: &mut EditorApp,
    stage: &mut Stage,
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    rect: egui::Rect,
    response: &egui::Response,
) {
    let (spin, roofs) = (app.prefabs.gallery.spin, app.prefabs.gallery.roofs);
    let density = ctx.pixels_per_point() * app.settings.prefs.render_scale;
    let width = (rect.width() * density).max(16.0) as u32;
    let height = (rect.height() * density).max(16.0) as u32;
    let has_still = app
        .prefabs
        .selected
        .as_deref()
        .is_some_and(|file| has_still(app, file));
    if spin && has_still && !response.dragged() {
        let dt = ctx.input(|i| i.stable_dt).clamp(0.0, 0.1);
        stage.spin(90.0 * dt);
    }
    let viewer = stage.viewer();
    viewer.settings.prefs.roofs = roofs;
    // The stage keeps the main editor's render profile and look.
    viewer.settings.prefs.render_profile = app.settings.prefs.render_profile;
    if response.hovered() || response.dragged() {
        viewer.handle_input(ctx, response);
    }
    viewer.render_viewport_image(ui, rect, width, height);
}

pub(in crate::app) fn take_stage(app: &mut EditorApp, file: &str) -> Option<Box<Stage>> {
    if app
        .prefabs
        .gallery
        .stage
        .as_ref()
        .is_some_and(|s| s.shown() == Some((file, 0)))
    {
        app.prefabs.gallery.stage.take()
    } else {
        None
    }
}
