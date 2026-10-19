//! The Player dock panel: quick player controls (stand, run, overlay) and Character Studio launch.
//! Detailed appearance, equipment paperdoll, skills, and named variables live in `player_window`.

use super::{
    EditorApp,
    player_runtime::{PlayerHost, PlayerSpec},
};
use crate::hud::Destination;
use crate::player::Stand;
use eframe::egui::{self, Color32, Stroke};

/// Keep the player's model in step with the settings (called once per frame).
pub(super) fn update_model(app: &mut EditorApp, ctx: &egui::Context) {
    let mode = app.interaction.camera().mode;
    if mode == crate::camera::CameraMode::Game && app.settings.prefs.player.stand.is_none() {
        let (cx, cz) = app.camera_tile();
        let plane = app.interaction.view_plane();
        app.settings.prefs.player.stand = Some(Stand {
            x: cx,
            y: cz,
            plane,
        });
        app.player_model.teleport((cx, cz, plane));
    }

    let player = &app.settings.prefs.player;
    let spec = player
        .stand
        .filter(|_| player.show_model)
        .map(|stand| PlayerSpec {
            look: player.look(),
            tile: (stand.x, stand.y),
            plane: stand.plane,
        });
    let running = player.running;
    let mut host = PlayerHost {
        world: &app.world,
        map: app.session.document(),
        renderer: &mut app.renderer,
        dt: ctx.input(|i| i.stable_dt),
        spec,
        running,
    };
    if app.player_model.update(&mut host) {
        ctx.request_repaint();
    }

    if let Some(tile) = app.player_model.current_tile()
        && let Some(stand) = &mut app.settings.prefs.player.stand
    {
        stand.x = tile.0;
        stand.y = tile.1;
    }

    if mode == crate::camera::CameraMode::Game
        && let Some(pos) = app.player_model.world_position()
    {
        let origin = app.renderer.render_origin();
        let local_x = pos[0] - origin.0 as f32;
        let local_y = pos[1] - 100.0;
        let local_z = pos[2] - origin.1 as f32;
        let dist = app.interaction.camera().orbit_distance.clamp(300.0, 3000.0);
        app.interaction
            .focus_camera([local_x, local_y, local_z], dist);
        app.rebase_if_needed();
    }
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

            ui.add_space(4.0);
            if ui
                .button(egui::RichText::new("✨ Open Character & Player Studio...").strong())
                .on_hover_text(
                    "Open dedicated window for detailed appearance, equipment, skills & variables",
                )
                .clicked()
            {
                app.ui.player_window_open = true;
            }
            ui.add_space(4.0);

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
                ui.checkbox(&mut player.show_model, "Draw model")
                    .on_hover_text("Off shows only the marker");
                ui.checkbox(&mut player.show_overlay, "Overlay")
                    .on_hover_text("Show 2D tile outline and name label marker over the player");
            });
            if let Some(error) = app.player_model.error() {
                ui.colored_label(Color32::LIGHT_RED, error);
            }
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
                        .button(format!("{} Go to", crate::icons::TELEPORT))
                        .clicked()
                    {
                        go_to = Some(*stand);
                    }
                });
            } else {
                ui.weak("Not in the world.");
            }

            crate::theme::section_heading(ui, "Quick Presets");
            ui.horizontal_wrapped(|ui| {
                if ui.small_button("Default Male").clicked() {
                    player.female = false;
                    player.kits = [
                        Some(0),
                        Some(14),
                        Some(18),
                        Some(26),
                        Some(33),
                        Some(36),
                        Some(42),
                    ];
                }
                if ui.small_button("Default Female").clicked() {
                    player.female = true;
                    player.kits = [
                        Some(45),
                        None,
                        Some(56),
                        Some(61),
                        Some(68),
                        Some(70),
                        Some(79),
                    ];
                }
                if ui.small_button("Bald Monk").clicked() {
                    player.kits[0] = None;
                    player.kits[1] = None;
                }
            });
            ui.horizontal_wrapped(|ui| {
                if ui.small_button("Rune Knight").clicked() {
                    player.wear(0, Some(1163)); // Full helm
                    player.wear(4, Some(1127)); // Platebody
                    player.wear(7, Some(1079)); // Platelegs
                    player.wear(5, Some(1201)); // Kiteshield
                    player.wear(3, Some(1333)); // Scimitar
                    player.wear(1, Some(1052)); // Cape of legends
                    player.wear(10, Some(4131)); // Boots
                }
                if ui.small_button("Mystic Mage").clicked() {
                    player.wear(0, Some(4099)); // Mystic hat
                    player.wear(4, Some(4101)); // Mystic top
                    player.wear(7, Some(4103)); // Mystic robe
                    player.wear(3, Some(1393)); // Fire staff
                    player.wear(1, Some(2412)); // Saradomin cape
                }
                if ui.small_button("Clear Gear").clicked() {
                    player.equipment.clear();
                }
            });

            crate::theme::section_heading(ui, "Status & Movement");
            let hitpoints = u16::from(player.levels[3]);
            let prayer_lvl = u16::from(player.levels[5]);
            ui.horizontal_wrapped(|ui| {
                ui.add(
                    egui::DragValue::new(&mut player.hitpoints)
                        .range(0..=hitpoints)
                        .prefix("HP "),
                );
                ui.add(
                    egui::DragValue::new(&mut player.prayer)
                        .range(0..=prayer_lvl)
                        .prefix("Prayer "),
                );
                ui.checkbox(&mut player.running, "Running")
                    .on_hover_text("Toggle player running (or Ctrl+click in Game mode)");
            });
            ui.add(egui::Slider::new(&mut player.run_energy, 0..=100).text("Run energy %"));

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

/// The mock player in the viewport: its tile, name, and ground click markers.
pub(super) fn draw_world(
    app: &EditorApp,
    painter: &egui::Painter,
    viewport: egui::Rect,
    pixels_per_point: f32,
    width: u32,
    height: u32,
) {
    let project = |p: [f64; 3], lift: f64| {
        app.screen_point(
            [p[0], p[1] - lift, p[2]],
            viewport,
            pixels_per_point,
            width,
            height,
        )
    };

    // Draw click marker (yellow X on ground) with fading alpha
    if let Some((tile, timer)) = app.player_model.click_marker() {
        let plane = app.interaction.view_plane();
        if let Some(corners) = crate::interact::tile_corners(app.world.infos(), tile, plane)
            && let (Some(c0), Some(c1), Some(c2), Some(c3)) = (
                project(corners[0], 2.0),
                project(corners[1], 2.0),
                project(corners[2], 2.0),
                project(corners[3], 2.0),
            )
        {
            let alpha = (timer / 1.2).clamp(0.0, 1.0);
            let yellow = Color32::from_rgba_unmultiplied(255, 230, 40, (alpha * 240.0) as u8);
            let stroke = Stroke::new(2.5, yellow);
            painter.line_segment([c0, c2], stroke);
            painter.line_segment([c1, c3], stroke);
        }
    }

    let player = &app.settings.prefs.player;
    if !player.show_overlay || app.interaction.camera().mode == crate::camera::CameraMode::Game {
        return;
    }
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

pub(super) fn walk_to(app: &mut EditorApp, target_tile: (i32, i32)) {
    let plane = app.interaction.view_plane();
    app.player_model.set_click_marker(target_tile);
    if app.settings.prefs.player.stand.is_none() {
        app.settings.prefs.player.stand = Some(Stand {
            x: target_tile.0,
            y: target_tile.1,
            plane,
        });
        app.player_model
            .teleport((target_tile.0, target_tile.1, plane));
        return;
    }
    let nav = crate::app::npc_runtime::WorldNavigation {
        world: &app.world,
        map: app.session.document(),
    };
    let outcome = app.player_model.walk_to(&nav, target_tile, plane);
    match outcome {
        osrs_playtest::walker::WalkOutcome::Walking { exact } => {
            if !exact {
                app.ui.notify("Walking as close as possible");
            }
        }
        osrs_playtest::walker::WalkOutcome::AlreadyThere => {}
        osrs_playtest::walker::WalkOutcome::Unreachable => {
            app.ui.notify("I can't reach that!");
        }
    }
}

pub(super) fn handle_game_mode_click(app: &mut EditorApp, response: &egui::Response) -> bool {
    if app.interaction.camera().mode != crate::camera::CameraMode::Game {
        return false;
    }
    if response.clicked_by(egui::PointerButton::Primary)
        && let Some(tile) = app.interaction.hover().tile.map(|p| p.tile)
    {
        if response.ctx.input(|i| i.modifiers.ctrl) {
            app.settings.prefs.player.running = !app.settings.prefs.player.running;
        }
        walk_to(app, tile);
        return true;
    }
    false
}
