//! The Sound dock panel: every object in the camera's map square that makes a sound, what it
//! plays, and how far it carries. Data and arithmetic live in `crate::sound`; this draws them,
//! keeps the panel's own widget state, and shows the selected sound's reach in the world. It never
//! changes the map.

use super::EditorApp;
use crate::hud::Destination;
use crate::sound::{self, AreaSound, Scope};
use eframe::egui::{self, Color32, Pos2, Rect, Stroke};

/// Most rows drawn at once; the filter narrows a longer list.
const MAX_ROWS: usize = 300;

#[derive(Default)]
pub(super) struct SoundPanel {
    filter: String,
    whole_map: bool,
    all_planes: bool,
    hide_in_world: bool,
    /// The selected row: its region and loc index.
    selected: Option<((i32, i32), usize)>,
    cache: Option<Cache>,
}

struct Cache {
    key: (u64, Scope, Option<u8>),
    rows: Vec<AreaSound>,
    unresolved: usize,
}

/// Recompute the list when the loaded map, the scope or the plane changed.
fn refresh(app: &mut EditorApp) {
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

/// "Sound #1234 (waterfall)".
fn sound_label(id: u32) -> String {
    match sound::name(id) {
        Some(name) => format!("#{id} {name}"),
        None => format!("#{id}"),
    }
}

fn plural(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

pub(super) fn show(app: &mut EditorApp, ui: &mut egui::Ui) {
    refresh(app);
    let mut go_to = None;
    let mut state = std::mem::take(&mut app.ui.sound);
    let rows = state.cache.as_ref().map_or(&[][..], |c| &c.rows[..]);
    let unresolved = state.cache.as_ref().map_or(0, |c| c.unresolved);

    let scope_label = match state.cache.as_ref().map(|c| c.key.1) {
        Some(Scope::Region((x, y))) => format!("Region {} ({x}, {y})", crate::hud::region_id(x, y)),
        _ => "All loaded regions".to_owned(),
    };
    ui.label(egui::RichText::new(scope_label).strong());
    ui.horizontal_wrapped(|ui| {
        ui.selectable_value(&mut state.whole_map, false, "This region");
        ui.selectable_value(&mut state.whole_map, true, "Loaded map");
    });
    ui.checkbox(&mut state.all_planes, "All planes")
        .on_hover_text("The game only plays sounds from the plane you are on");
    ui.checkbox(&mut state.hide_in_world, "Hide in the world")
        .on_hover_text("Markers and the audible range of each sound are drawn in the viewport");
    ui.add(
        egui::TextEdit::singleline(&mut state.filter)
            .hint_text("Filter by object, sound name or id")
            .desired_width(f32::INFINITY),
    );
    let needle = state.filter.trim().to_lowercase();
    let shown: Vec<&AreaSound> = rows.iter().filter(|r| r.matches(&needle)).collect();

    ui.add_space(4.0);
    let counts = sound::by_sound(rows);
    ui.weak(format!(
        "{} making {}{}",
        plural(rows.len(), "object", "objects"),
        plural(counts.len(), "different sound", "different sounds"),
        if needle.is_empty() {
            String::new()
        } else {
            format!(" \u{b7} {} shown", shown.len())
        }
    ));
    if unresolved > 0 {
        ui.weak(format!(
            "{} change with a variable; the sound of what they turn into is not listed",
            plural(unresolved, "object", "objects")
        ))
        .on_hover_text(
            "The Player panel's variables will decide these once object changes are resolved",
        );
    }
    if rows.is_empty() {
        ui.add_space(6.0);
        ui.label("No object here plays a sound.");
    }

    if !counts.is_empty() {
        egui::CollapsingHeader::new("Sounds used")
            .id_salt("sound_counts")
            .show(ui, |ui| {
                for (id, count) in counts.iter().take(40) {
                    if ui
                        .selectable_label(false, format!("{}  \u{d7} {count}", sound_label(*id)))
                        .on_hover_text("Show only this sound")
                        .clicked()
                    {
                        state.filter = id.to_string();
                    }
                }
            });
    }

    if let Some(selected) = state
        .selected
        .and_then(|key| rows.iter().find(|r| (r.region, r.index) == key))
    {
        details(ui, selected, &mut go_to);
    }

    crate::theme::section_heading(ui, "Objects");
    for row in shown.iter().take(MAX_ROWS) {
        let key = (row.region, row.index);
        let main = row.main_sound().map_or_else(|| "-".to_owned(), sound_label);
        let object = if row.name.is_empty() {
            format!("object {}", row.object)
        } else {
            row.name.clone()
        };
        let kind = if row.sound.random.is_empty() {
            String::new()
        } else {
            format!("  {}", egui_phosphor::regular::SHUFFLE)
        };
        let response = ui
            .selectable_label(
                state.selected == Some(key),
                format!("{main} \u{b7} {object}{kind}"),
            )
            .on_hover_text(format!(
                "{} at ({}, {}), plane {}. Hears out to {} tiles.",
                object, row.tile.0, row.tile.1, row.plane, row.sound.distance
            ));
        if response.clicked() {
            state.selected = Some(key);
        }
        if response.double_clicked() {
            go_to = Some((row.tile, row.plane));
        }
    }
    if shown.len() > MAX_ROWS {
        ui.weak(format!(
            "{} more; narrow the filter to see them",
            shown.len() - MAX_ROWS
        ));
    }
    app.ui.sound = state;
    if let Some((tile, plane)) = go_to {
        app.teleport_to(Destination::Tile {
            x: tile.0,
            y: tile.1,
            plane,
        });
    }
}

/// What the selected object's sound does, as the client treats it.
fn details(ui: &mut egui::Ui, row: &AreaSound, go_to: &mut Option<((i32, i32), u8)>) {
    let sound = &row.sound;
    let (inner, outer) = sound::reach(sound);
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(
            egui::RichText::new(if row.name.is_empty() {
                "Object"
            } else {
                &row.name
            })
            .strong(),
        );
        ui.weak(format!(
            "object {} \u{b7} tile ({}, {}) plane {} \u{b7} {}\u{d7}{} tiles",
            row.object, row.tile.0, row.tile.1, row.plane, row.size.0, row.size.1
        ));
        if let Some(looping) = sound.looping {
            ui.label(format!("Loops {}", sound_label(looping.get())));
        }
        if !sound.random.is_empty() {
            let list: Vec<String> = sound
                .random
                .iter()
                .map(|id| sound_label(id.get()))
                .collect();
            ui.label(format!("Plays one at random: {}", list.join(", ")));
            ui.weak(format!(
                "every {} to {} client cycles (about 20 ms each)",
                sound.change_ticks.0, sound.change_ticks.1
            ));
        }
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
        if ui
            .button(format!("{}  Go to", crate::icons::TELEPORT))
            .clicked()
        {
            *go_to = Some((row.tile, row.plane));
        }
    });
}

/// The sounds in the viewport: a marker on each, and the full-volume and audible ranges of the
/// selected one.
pub(super) fn draw_world(
    app: &EditorApp,
    painter: &egui::Painter,
    viewport: Rect,
    pixels_per_point: f32,
    width: u32,
    height: u32,
) {
    use crate::settings::RailTab;
    let prefs = &app.settings.prefs;
    let state = &app.ui.sound;
    if !prefs.properties_panel || prefs.rail_tab != RailTab::Sound || state.hide_in_world {
        return;
    }
    let Some(cache) = &state.cache else {
        return;
    };
    let plane = app.interaction.view_plane();
    let project = |x: f32, z: f32, plane: u8, lift: f64| -> Option<Pos2> {
        let tile = (x.floor() as i32, z.floor() as i32);
        let corners = crate::interact::tile_corners(app.world.infos(), tile, plane)?;
        let ground = corners.iter().map(|c| c[1]).sum::<f64>() / 4.0;
        app.screen_point(
            [f64::from(x) * 128.0, ground - lift, f64::from(z) * 128.0],
            viewport,
            pixels_per_point,
            width,
            height,
        )
    };
    let accent = Color32::from_rgb(120, 200, 255);
    let needle = state.filter.trim().to_lowercase();
    for row in cache
        .rows
        .iter()
        .filter(|r| r.plane == plane && r.matches(&needle))
    {
        let selected = state.selected == Some((row.region, row.index));
        let (min, max) = row.rect();
        let centre = ((min.0 + max.0) / 2.0, (min.1 + max.1) / 2.0);
        if let Some(at) = project(centre.0, centre.1, row.plane, 24.0)
            && viewport.contains(at)
        {
            let colour = if selected { Color32::WHITE } else { accent };
            painter.circle_filled(
                at,
                if selected { 6.0 } else { 4.0 },
                colour.gamma_multiply(0.9),
            );
            painter.circle_stroke(
                at,
                7.0,
                Stroke::new(1.5, Color32::BLACK.gamma_multiply(0.6)),
            );
        }
        if !selected {
            continue;
        }
        let (inner, outer) = sound::reach(&row.sound);
        for (distance, colour) in [(outer, accent), (inner, Color32::from_rgb(120, 255, 170))] {
            let points: Vec<Pos2> = sound::ring(min, max, distance)
                .into_iter()
                .filter_map(|(x, z)| project(x, z, row.plane, 6.0))
                .collect();
            if points.len() > 2 {
                painter.add(egui::Shape::closed_line(points, Stroke::new(2.0, colour)));
            }
        }
    }
}
