//! The three pages of the Sound panel.
use super::{Page, object_name, plural, row_matches, sound_label};
use crate::{
    app::{EditorApp, audio_runtime::SoundState},
    hud::Destination,
    sound::{self, AreaSound},
};
use eframe::egui::{self, Color32, RichText};

/// Most rows drawn at once; the filter narrows a longer list.
const MAX_ROWS: usize = 300;

/// What a click on this frame asked for; applied after drawing so the lists stay borrowed.
#[derive(Default)]
struct Actions {
    play: Option<u32>,
    stop: bool,
    select: Option<((i32, i32), usize)>,
    go_to: Option<((i32, i32), u8)>,
    filter: Option<String>,
    page: Option<Page>,
}

pub(super) fn show(app: &mut EditorApp, ui: &mut egui::Ui) {
    let mut state = std::mem::take(&mut app.ui.sound);
    let mut actions = Actions::default();
    let playing = app.audio.playing();
    if playing.is_some() || app.audio.waiting().is_some() {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(150));
    }
    if state.selected.is_none() && state.page == Page::Object {
        state.page = Page::Sounds;
    }
    ui.horizontal_wrapped(|ui| {
        ui.selectable_value(&mut state.page, Page::Sounds, "Sounds");
        ui.selectable_value(&mut state.page, Page::Objects, "Objects");
        ui.add_enabled_ui(state.selected.is_some(), |ui| {
            ui.selectable_value(&mut state.page, Page::Object, "Object");
        });
    });
    ui.separator();
    if state.page != Page::Object {
        filters(&mut state, ui, app.camera_region());
        if let Some(note) = app.names.note() {
            ui.weak(format!("No installation names: {note}"));
        }
    }
    let rows: &[AreaSound] = state.cache.as_ref().map_or(&[], |c| &c.rows[..]);
    let unresolved = state.cache.as_ref().map_or(0, |c| c.unresolved);
    let needle = state.filter.trim().to_lowercase();
    let shown: Vec<&AreaSound> = rows
        .iter()
        .filter(|r| row_matches(app, r, &needle))
        .collect();
    match state.page {
        Page::Sounds => sounds_page(
            ui,
            app,
            &shown,
            rows.len(),
            unresolved,
            &mut state.expanded,
            &mut actions,
        ),
        Page::Objects => objects_page(ui, app, &shown, state.selected, &mut actions),
        Page::Object => {
            let selected = state
                .selected
                .and_then(|key| rows.iter().find(|r| (r.region, r.index) == key));
            match selected {
                Some(row) => object_page(ui, app, row, &mut state.loop_playback, &mut actions),
                None => {
                    ui.weak("That object is no longer loaded.");
                }
            }
        }
    }
    if let Some(error) = app.audio.error() {
        ui.colored_label(Color32::LIGHT_RED, error);
    }
    if let Some(key) = actions.select {
        state.selected = Some(key);
        state.page = Page::Object;
    }
    if let Some(page) = actions.page {
        state.page = page;
    }
    if let Some(filter) = actions.filter {
        state.filter = filter;
        state.page = Page::Sounds;
    }
    let looped = state.loop_playback;
    app.ui.sound = state;
    if actions.stop {
        app.audio.stop();
    }
    if let Some(id) = actions.play {
        app.audio.play(&app.world, id, looped);
    }
    if let Some((tile, plane)) = actions.go_to {
        app.teleport_to(Destination::Tile {
            x: tile.0,
            y: tile.1,
            plane,
        });
    }
}

fn filters(state: &mut super::SoundPanel, ui: &mut egui::Ui, region: (i32, i32)) {
    let scope = if state.whole_map {
        "All loaded regions".to_owned()
    } else {
        format!(
            "Region {} ({}, {})",
            crate::hud::region_id(region.0, region.1),
            region.0,
            region.1
        )
    };
    ui.label(RichText::new(scope).strong());
    ui.horizontal_wrapped(|ui| {
        ui.selectable_value(&mut state.whole_map, false, "This region");
        ui.selectable_value(&mut state.whole_map, true, "Loaded map");
        ui.checkbox(&mut state.all_planes, "All planes")
            .on_hover_text("The game only plays sounds from the plane you are on");
    });
    ui.checkbox(&mut state.hide_in_world, "Hide markers in the world")
        .on_hover_text("Markers and the audible range of each sound are drawn in the viewport");
    ui.add(
        egui::TextEdit::singleline(&mut state.filter)
            .hint_text("Filter by object, sound name or id")
            .desired_width(f32::INFINITY),
    );
}

/// A Play / Stop button for sound `id`.
fn play_button(ui: &mut egui::Ui, app: &EditorApp, id: u32, actions: &mut Actions) {
    let playing = app.audio.playing() == Some(id);
    let waiting = app.audio.waiting() == Some(id);
    let (icon, tip) = if playing {
        (egui_phosphor::regular::STOP, "Stop")
    } else if waiting {
        (egui_phosphor::regular::HOURGLASS, "Rendering the sound...")
    } else if let Some(SoundState::Failed(reason)) = app.audio.state(id) {
        (egui_phosphor::regular::WARNING, reason.as_str())
    } else {
        (egui_phosphor::regular::PLAY, "Play")
    };
    if ui.small_button(icon).on_hover_text(tip).clicked() {
        if playing {
            actions.stop = true;
        } else {
            actions.play = Some(id);
        }
    }
}

/// "1.4 s" once the sound has been rendered.
fn length_text(app: &EditorApp, id: u32) -> Option<String> {
    match app.audio.state(id)? {
        SoundState::Ready(sound) if sound.sample_rate > 0 => Some(format!(
            "{:.1} s",
            sound.samples.len() as f32 / sound.sample_rate as f32
        )),
        _ => None,
    }
}

/// One row per distinct sound, with the objects that use it.
fn sounds_page(
    ui: &mut egui::Ui,
    app: &EditorApp,
    shown: &[&AreaSound],
    total: usize,
    unresolved: usize,
    expanded: &mut std::collections::BTreeSet<u32>,
    actions: &mut Actions,
) {
    let order = sound::by_sound(shown);
    ui.weak(format!(
        "{} making {}{}",
        plural(total, "object", "objects"),
        plural(order.len(), "different sound", "different sounds"),
        if shown.len() == total {
            String::new()
        } else {
            format!(" \u{b7} {} shown", shown.len())
        }
    ));
    if unresolved > 0 {
        ui.weak(format!(
            "{} change with a variable; the sound of what they turn into is not listed",
            plural(unresolved, "object", "objects")
        ));
    }
    if order.is_empty() {
        ui.add_space(6.0);
        ui.label("No object here plays a sound.");
        return;
    }
    let camera = app.camera_tile();
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (id, objects) in order.iter().take(MAX_ROWS) {
                let id = *id;
                let open = expanded.contains(&id);
                ui.horizontal(|ui| {
                    play_button(ui, app, id, actions);
                    let caret = if open {
                        egui_phosphor::regular::CARET_DOWN
                    } else {
                        egui_phosphor::regular::CARET_RIGHT
                    };
                    let mut names: Vec<String> =
                        objects.iter().map(|o| object_name(app, o)).collect();
                    names.sort();
                    names.dedup();
                    let mut title = format!("{caret} {}", sound_label(app, id));
                    if let Some(length) = length_text(app, id) {
                        title.push_str(&format!("  \u{b7} {length}"));
                    }
                    let summary = format!(
                        "{} \u{b7} {}",
                        plural(objects.len(), "object", "objects"),
                        names.iter().take(2).cloned().collect::<Vec<_>>().join(", ")
                    );
                    if ui
                        .selectable_label(open, RichText::new(title).strong())
                        .on_hover_text(summary.clone())
                        .clicked()
                    {
                        if open {
                            expanded.remove(&id);
                        } else {
                            expanded.insert(id);
                        }
                    }
                    ui.weak(summary);
                });
                if open {
                    let mut near: Vec<&&AreaSound> = objects.iter().collect();
                    near.sort_by_key(|o| {
                        (o.tile.0 - camera.0).abs().max((o.tile.1 - camera.1).abs())
                    });
                    for row in near.into_iter().take(40) {
                        let distance = (row.tile.0 - camera.0)
                            .abs()
                            .max((row.tile.1 - camera.1).abs());
                        let response = ui
                            .selectable_label(
                                false,
                                format!(
                                    "      {} \u{b7} ({}, {}) \u{b7} {distance} tiles away",
                                    object_name(app, row),
                                    row.tile.0,
                                    row.tile.1
                                ),
                            )
                            .on_hover_text("Open this object");
                        if response.clicked() {
                            actions.select = Some((row.region, row.index));
                        }
                        if response.double_clicked() {
                            actions.go_to = Some((row.tile, row.plane));
                        }
                    }
                }
            }
        });
}

/// One row per object that plays a sound.
fn objects_page(
    ui: &mut egui::Ui,
    app: &EditorApp,
    shown: &[&AreaSound],
    selected: Option<((i32, i32), usize)>,
    actions: &mut Actions,
) {
    ui.weak(plural(shown.len(), "object", "objects"));
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for row in shown.iter().take(MAX_ROWS) {
                let key = (row.region, row.index);
                ui.horizontal(|ui| {
                    if let Some(id) = row.main_sound() {
                        play_button(ui, app, id, actions);
                    }
                    let main = row
                        .main_sound()
                        .map_or_else(|| "-".to_owned(), |id| sound_label(app, id));
                    let kind = if row.sound.random.is_empty() {
                        String::new()
                    } else {
                        format!("  {}", egui_phosphor::regular::SHUFFLE)
                    };
                    let response = ui
                        .selectable_label(
                            selected == Some(key),
                            format!("{} \u{b7} {main}{kind}", object_name(app, row)),
                        )
                        .on_hover_text(format!(
                            "at ({}, {}), plane {}. Hears out to {} tiles.",
                            row.tile.0, row.tile.1, row.plane, row.sound.distance
                        ));
                    if response.clicked() {
                        actions.select = Some(key);
                    }
                    if response.double_clicked() {
                        actions.go_to = Some((row.tile, row.plane));
                    }
                });
            }
            if shown.len() > MAX_ROWS {
                ui.weak(format!(
                    "{} more; narrow the filter to see them",
                    shown.len() - MAX_ROWS
                ));
            }
        });
}

/// Everything about one object's sound.
fn object_page(
    ui: &mut egui::Ui,
    app: &EditorApp,
    row: &AreaSound,
    loop_playback: &mut bool,
    actions: &mut Actions,
) {
    let sound = &row.sound;
    let (inner, outer) = sound::reach(sound);
    if ui
        .button(format!("{} Sounds", egui_phosphor::regular::CARET_LEFT))
        .clicked()
    {
        actions.page = Some(Page::Sounds);
    }
    ui.add_space(4.0);
    ui.label(RichText::new(object_name(app, row)).strong().size(16.0));
    ui.weak(format!(
        "object {} \u{b7} tile ({}, {}) plane {} \u{b7} {}\u{d7}{} tiles",
        row.object, row.tile.0, row.tile.1, row.plane, row.size.0, row.size.1
    ));
    crate::theme::section_heading(ui, "Plays");
    let sound_row = |ui: &mut egui::Ui, id: u32, actions: &mut Actions, note: &str| {
        ui.horizontal(|ui| {
            play_button(ui, app, id, actions);
            let mut text = sound_label(app, id);
            if let Some(length) = length_text(app, id) {
                text.push_str(&format!("  \u{b7} {length}"));
            }
            if ui
                .link(text)
                .on_hover_text("List every object that plays this sound")
                .clicked()
            {
                actions.filter = Some(id.to_string());
            }
            if !note.is_empty() {
                ui.weak(note);
            }
        });
    };
    if let Some(looping) = sound.looping {
        sound_row(ui, looping.get(), actions, "loops");
    }
    if !sound.random.is_empty() {
        for id in &sound.random {
            sound_row(ui, id.get(), actions, "");
        }
        ui.weak(format!(
            "One is picked at random every {} to {} client cycles (about 20 ms each)",
            sound.change_ticks.0, sound.change_ticks.1
        ));
    }
    ui.checkbox(loop_playback, "Repeat when played here")
        .on_hover_text("Previews the sound over and over until stopped");
    crate::theme::section_heading(ui, "How far it carries");
    ui.label(format!(
        "Full volume within {inner:.1} tiles of the object, silent beyond {outer:.1}"
    ));
    let who = match sound.visibility {
        0 => "everyone",
        1 => "only inside the object's own world view",
        _ => "the default (same world view or the main map)",
    };
    ui.weak(format!(
        "Heard by {who} \u{b7} fade in {} / out {} cycles \u{b7} curves {} / {} / {}",
        sound.fade_in.1,
        sound.fade_out.1,
        sound.fade_in.0,
        sound.fade_out.0,
        sound.distance_fade_curve
    ));
    ui.add_space(6.0);
    if ui
        .button(format!("{}  Go to", crate::icons::TELEPORT))
        .clicked()
    {
        actions.go_to = Some((row.tile, row.plane));
    }
}
