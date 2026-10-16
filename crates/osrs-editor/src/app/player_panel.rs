//! The Player dock panel: edit the mock player (see `crate::player`) and stand it in the world.
//! Drawing edits the player in the settings; nothing here touches the map or its history.

use super::EditorApp;
use crate::hud::Destination;
use crate::player::{
    COLOUR_SLOTS, EQUIPMENT_SLOTS, KIT_SLOTS, MAX_LEVEL, PlayerSandbox, SKILLS, Stand,
};
use eframe::egui::{self, Color32, Stroke};
use std::collections::BTreeMap;

/// A drag value that shows "none" for -1 and stores `None`.
fn optional_id(ui: &mut egui::Ui, value: &mut Option<u16>) {
    let mut raw = value.map_or(-1, i32::from);
    ui.add(
        egui::DragValue::new(&mut raw)
            .range(-1..=u16::MAX as i32)
            .custom_formatter(|n, _| {
                if n < 0.0 {
                    "none".to_owned()
                } else {
                    format!("{n}")
                }
            }),
    );
    *value = u16::try_from(raw).ok();
}

/// The draft a variable table adds from, kept in egui's temporary memory.
#[derive(Clone, Copy, Default)]
struct Draft {
    id: u32,
    value: i32,
}

fn variables(ui: &mut egui::Ui, title: &str, hint: &str, table: &mut BTreeMap<u32, i32>) {
    egui::CollapsingHeader::new(format!("{title} ({})", table.len()))
        .id_salt(title)
        .show(ui, |ui| {
            ui.weak(hint);
            let key = ui.make_persistent_id((title, "draft"));
            let mut draft = ui
                .data_mut(|d| d.get_temp::<Draft>(key))
                .unwrap_or_default();
            ui.horizontal_wrapped(|ui| {
                ui.add(egui::DragValue::new(&mut draft.id).prefix("id "));
                ui.add(egui::DragValue::new(&mut draft.value).prefix("= "));
                if ui
                    .small_button(crate::icons::ADD)
                    .on_hover_text("Set this variable")
                    .clicked()
                {
                    table.insert(draft.id, draft.value);
                }
            });
            ui.data_mut(|d| d.insert_temp(key, draft));
            let mut remove = None;
            for (id, value) in table.iter_mut() {
                ui.horizontal(|ui| {
                    ui.label(format!("{id}"));
                    ui.add(egui::DragValue::new(value));
                    if ui
                        .small_button(egui_phosphor::regular::TRASH)
                        .on_hover_text("Remove")
                        .clicked()
                    {
                        remove = Some(*id);
                    }
                });
            }
            if let Some(id) = remove {
                table.remove(&id);
            }
        });
}

pub(super) fn show(app: &mut EditorApp, ui: &mut egui::Ui) {
    let hover = app
        .interaction
        .hover()
        .tile
        .map(|pick| pick.tile)
        .or(app.interaction.last_hover_tile());
    let (here, plane) = (
        hover.unwrap_or_else(|| app.camera_tile()),
        app.interaction.view_plane(),
    );
    let mut go_to = None;
    let player = &mut app.settings.prefs.player;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Name");
                ui.add(egui::TextEdit::singleline(&mut player.name).desired_width(f32::INFINITY));
            });
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut player.female, false, "Male");
                ui.selectable_value(&mut player.female, true, "Female");
                ui.weak(format!(
                    "Combat {} \u{b7} Total {}",
                    player.combat_level(),
                    player.total_level()
                ));
            });

            crate::theme::section_heading(ui, "Location");
            ui.horizontal_wrapped(|ui| {
                if ui
                    .button("Stand here")
                    .on_hover_text("On the tile under the cursor, else the camera's tile")
                    .clicked()
                {
                    player.stand = Some(Stand {
                        x: here.0,
                        y: here.1,
                        plane,
                    });
                }
                if player.stand.is_some() && ui.button("Remove").clicked() {
                    player.stand = None;
                }
            });
            if let Some(stand) = &mut player.stand {
                ui.horizontal_wrapped(|ui| {
                    ui.add(egui::DragValue::new(&mut stand.x).prefix("X "));
                    ui.add(egui::DragValue::new(&mut stand.y).prefix("Z "));
                    ui.add(
                        egui::DragValue::new(&mut stand.plane)
                            .range(0..=3)
                            .prefix("P "),
                    );
                    if ui
                        .button(format!("{}  Go to", crate::icons::TELEPORT))
                        .clicked()
                    {
                        go_to = Some(*stand);
                    }
                });
            } else {
                ui.weak("Not in the world.");
            }

            egui::CollapsingHeader::new("Skills")
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        if ui.small_button("All 99").clicked() {
                            player.max_levels();
                        }
                        if ui.small_button("Reset").clicked() {
                            let keep = std::mem::take(player);
                            *player = PlayerSandbox {
                                name: keep.name,
                                stand: keep.stand,
                                ..PlayerSandbox::default()
                            };
                        }
                    });
                    egui::Grid::new("player_skills")
                        .num_columns(4)
                        .show(ui, |ui| {
                            for (skill, name) in SKILLS.iter().enumerate() {
                                ui.label(*name);
                                let mut level = player.levels[skill];
                                if ui
                                    .add(egui::DragValue::new(&mut level).range(1..=MAX_LEVEL))
                                    .changed()
                                {
                                    player.set_level(skill, level);
                                }
                                if skill % 2 == 1 {
                                    ui.end_row();
                                }
                            }
                        });
                });

            egui::CollapsingHeader::new("Status").show(ui, |ui| {
                let hitpoints = u16::from(player.levels[3]);
                let prayer = u16::from(player.levels[5]);
                ui.horizontal_wrapped(|ui| {
                    ui.add(
                        egui::DragValue::new(&mut player.hitpoints)
                            .range(0..=hitpoints)
                            .prefix("HP "),
                    );
                    ui.add(
                        egui::DragValue::new(&mut player.prayer)
                            .range(0..=prayer)
                            .prefix("Prayer "),
                    );
                });
                ui.add(egui::Slider::new(&mut player.run_energy, 0..=100).text("Run energy %"));
                ui.checkbox(&mut player.running, "Running");
                ui.add(
                    egui::DragValue::new(&mut player.weight_kg)
                        .range(-30..=64)
                        .suffix(" kg"),
                );
            });

            egui::CollapsingHeader::new("Look").show(ui, |ui| {
                egui::Grid::new("player_kits")
                    .num_columns(2)
                    .show(ui, |ui| {
                        for (slot, name) in KIT_SLOTS.iter().enumerate() {
                            ui.label(*name);
                            optional_id(ui, &mut player.kits[slot]);
                            ui.end_row();
                        }
                        for (slot, name) in COLOUR_SLOTS.iter().enumerate() {
                            ui.label(format!("{name} colour"));
                            ui.add(egui::DragValue::new(&mut player.colours[slot]));
                            ui.end_row();
                        }
                    });
                ui.weak(
                    "Kit and colour ids as the game stores them; they show once players are drawn.",
                );
            });

            egui::CollapsingHeader::new(format!("Worn ({})", player.equipment.len()))
                .id_salt("player_worn")
                .show(ui, |ui| {
                    egui::Grid::new("player_equipment")
                        .num_columns(2)
                        .show(ui, |ui| {
                            for (slot, name) in EQUIPMENT_SLOTS {
                                ui.label(name);
                                let mut item = player.equipment.get(&slot).copied().unwrap_or(0);
                                if ui
                                    .add(egui::DragValue::new(&mut item).custom_formatter(
                                        |n, _| {
                                            if n == 0.0 {
                                                "empty".to_owned()
                                            } else {
                                                format!("{n}")
                                            }
                                        },
                                    ))
                                    .changed()
                                {
                                    player.wear(slot, (item != 0).then_some(item));
                                }
                                ui.end_row();
                            }
                        });
                    ui.weak("Item ids (0 is empty).");
                });

            crate::theme::section_heading(ui, "Variables");
            variables(
                ui,
                "Varps",
                "Player variables (config values).",
                &mut player.varps,
            );
            variables(
                ui,
                "Varbits",
                "Bit ranges of a varp, set directly by varbit id.",
                &mut player.varbits,
            );
            variables(
                ui,
                "Varcs",
                "Client variables used by interfaces and scripts.",
                &mut player.varcs,
            );

            ui.add_space(8.0);
            if ui.button("Copy as JSON").clicked() {
                ui.ctx().copy_text(player.to_json());
            }
        });
    if let Some(stand) = go_to {
        app.teleport_to(Destination::Tile {
            x: stand.x,
            y: stand.y,
            plane: stand.plane,
        });
    }
}

/// The mock player in the viewport: its tile and name, until real players are drawn.
pub(super) fn draw_world(
    app: &EditorApp,
    painter: &egui::Painter,
    viewport: egui::Rect,
    pixels_per_point: f32,
    width: u32,
    height: u32,
) {
    let player = &app.settings.prefs.player;
    let Some(stand) = player.stand else {
        return;
    };
    if stand.plane != app.interaction.view_plane() {
        return;
    }
    let Some(corners) =
        crate::interact::tile_corners(app.world.infos(), (stand.x, stand.y), stand.plane)
    else {
        return;
    };
    let project = |p: [f64; 3], lift: f64| {
        app.screen_point(
            [p[0], p[1] - lift, p[2]],
            viewport,
            pixels_per_point,
            width,
            height,
        )
    };
    let outline: Vec<egui::Pos2> = corners.iter().filter_map(|c| project(*c, 4.0)).collect();
    let colour = Color32::from_rgb(255, 214, 90);
    if outline.len() == 4 {
        painter.add(egui::Shape::closed_line(outline, Stroke::new(2.0, colour)));
    }
    let centre = [
        corners.iter().map(|c| c[0]).sum::<f64>() / 4.0,
        corners.iter().map(|c| c[1]).sum::<f64>() / 4.0,
        corners.iter().map(|c| c[2]).sum::<f64>() / 4.0,
    ];
    if let (Some(foot), Some(head)) = (project(centre, 0.0), project(centre, 200.0)) {
        painter.line_segment([foot, head], Stroke::new(3.0, colour));
        painter.circle_filled(head, 5.0, colour);
        painter.text(
            head + egui::vec2(0.0, -10.0),
            egui::Align2::CENTER_BOTTOM,
            format!("{} (level {})", player.name, player.combat_level()),
            egui::FontId::proportional(12.0),
            Color32::WHITE,
        );
    }
}
