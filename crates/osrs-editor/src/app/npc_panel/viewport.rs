//! Armed viewport gestures and the world overlay (anchors, footprints, routes, reasons).

use super::{actor_location, authoring::status_text, scenario::plan_and_commit};
use crate::app::{
    EditorApp,
    npc_runtime::{Arm, WorldNavigation},
};
use crate::settings::RailTab;
use eframe::egui::{self, Color32, Stroke};
use osrs_core::scenario::{MovementRestriction, NpcBehavior, NpcSpawn};
use osrs_edit::{SpawnDraft, plan_add_spawn};
use osrs_playtest::{
    navigation::{Route, RouteTarget, find_path},
    runtime::{MoveSpeed, RuntimeCommand},
};
// ----------------------------------------------------------------------------- viewport

/// Armed gestures own the viewport click. Returns true when the click was consumed or an armed
/// gesture is pending, so the active tool ignores this frame's pointer.
pub(in crate::app) fn armed_input(app: &mut EditorApp, response: &egui::Response) -> bool {
    if !app.settings.prefs.npc.spawns {
        app.npcs.cancel_gesture();
        return false;
    }
    let ctx = response.ctx.clone();
    if app.npcs.arm == Arm::None {
        // With the panel open a plain click selects the NPC on the hovered tile.
        if response.clicked()
            && app.settings.prefs.properties_panel
            && app.settings.prefs.rail_tab == RailTab::Npc
            && let Some(tile) = app.interaction.hover().tile.map(|pick| pick.tile)
        {
            select_at(app, tile);
        }
        return false;
    }
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.npcs.arm = Arm::None;
        return true;
    }
    if !response.clicked() {
        return true;
    }
    let Some(tile) = app.interaction.hover().tile.map(|pick| pick.tile) else {
        return true;
    };
    let Some(location) = actor_location(tile, app.interaction.view_plane()) else {
        return true;
    };
    match app.npcs.arm.clone() {
        Arm::Place { npc, symbol } => {
            let draft = SpawnDraft::new(npc, symbol, location);
            let mut placed = None;
            if plan_and_commit(app, |doc| {
                plan_add_spawn(doc, draft).map(|(patch, id)| {
                    placed = Some(id);
                    patch
                })
            }) {
                app.npcs.selection = placed;
            }
            // Placement stays armed so several can be placed; Esc or another selection ends it.
        }
        Arm::Destination { route } => {
            if let Some(actor) = app.npcs.selection {
                let speed = ctx
                    .data(|d| d.get_temp::<u8>(egui::Id::new("npc-speed")))
                    .unwrap_or(1);
                app.npcs.queue(RuntimeCommand::Move {
                    actor,
                    destination: location,
                    route,
                    speed: match speed {
                        0 => MoveSpeed::Crawl,
                        2 => MoveSpeed::Run,
                        _ => MoveSpeed::Walk,
                    },
                });
            }
            app.npcs.arm = Arm::None;
        }
        Arm::None => {}
    }
    true
}

fn select_at(app: &mut EditorApp, tile: (i32, i32)) {
    let plane = app.interaction.view_plane();
    let found = app.session.scenario().definition().and_then(|d| {
        d.spawns
            .iter()
            .filter(|s| s.location.plane.get() == plane)
            .find(|s| {
                let size = app
                    .npcs
                    .assets()
                    .and_then(|a| a.npcs.get(&s.npc))
                    .map_or(1, |d| i32::from(d.size));
                (s.location.tile.x..s.location.tile.x + size).contains(&tile.0)
                    && (s.location.tile.y..s.location.tile.y + size).contains(&tile.1)
            })
            .map(|s| s.id)
    });
    if found.is_some() {
        app.npcs.selection = found;
    }
}

/// Anchors, footprints, patrol routes, wander ranges, destinations and the live actor markers.
pub(in crate::app) fn draw_world(
    app: &EditorApp,
    painter: &egui::Painter,
    viewport: egui::Rect,
    pixels_per_point: f32,
    width: u32,
    height: u32,
) {
    if !app.settings.prefs.npc.spawns {
        return;
    }
    let Some(definition) = app.session.scenario().definition() else {
        return;
    };
    let plane = app.interaction.view_plane();
    let (cx, cy) = app.camera_tile();
    let reach = i32::from(app.settings.prefs.npc.distance);
    let project = |p: [f64; 3], lift: f64| {
        app.screen_point(
            [p[0], p[1] - lift, p[2]],
            viewport,
            pixels_per_point,
            width,
            height,
        )
    };
    let corner = |tile: (i32, i32), index: usize| {
        crate::interact::tile_corners(app.world.infos(), tile, plane).map(|c| c[index])
    };
    let outline = |tile: (i32, i32), size: i32, colour: Color32, width: f32| {
        let points: Vec<egui::Pos2> = [
            corner(tile, 0),
            corner((tile.0 + size - 1, tile.1), 1),
            corner((tile.0 + size - 1, tile.1 + size - 1), 2),
            corner((tile.0, tile.1 + size - 1), 3),
        ]
        .into_iter()
        .flatten()
        .filter_map(|c| project(c, 4.0))
        .collect();
        if points.len() == 4 {
            painter.add(egui::Shape::closed_line(points, Stroke::new(width, colour)));
        }
    };
    let centre = |tile: (i32, i32)| {
        let c = crate::interact::tile_corners(app.world.infos(), tile, plane)?;
        Some([
            (c[0][0] + c[2][0]) / 2.0,
            (c[0][1] + c[2][1]) / 2.0,
            (c[0][2] + c[2][2]) / 2.0,
        ])
    };
    let size_of = |spawn: &NpcSpawn| {
        app.npcs
            .assets()
            .and_then(|a| a.npcs.get(&spawn.npc))
            .map_or(1, |d| i32::from(d.size))
    };
    let anchor = Color32::from_rgb(120, 200, 255);
    let mut drawn = std::collections::HashSet::new();
    let max_outlines = usize::from(app.settings.prefs.npc.max_drawn);
    let selected_colour = Color32::from_rgb(255, 214, 90);
    for spawn in &definition.spawns {
        let tile = (spawn.location.tile.x, spawn.location.tile.y);
        if spawn.location.plane.get() != plane
            || (tile.0 - cx).abs() > reach
            || (tile.1 - cy).abs() > reach
        {
            continue;
        }
        let selected = app.npcs.selection == Some(spawn.id);
        let colour = if selected { selected_colour } else { anchor };
        if selected {
            outline(tile, size_of(spawn), colour, 2.5);
        } else if app.settings.prefs.npc.footprints && drawn.len() < max_outlines {
            // One outline per NPC, under where it stands now (not its spawn tile, which it walks
            // away from), and one per tile when several stand together.
            if let Some(actor) = app.npcs.actor(spawn.id)
                && actor.location.plane.get() == plane
                && !matches!(
                    actor.status,
                    osrs_core::actor_snapshot::ActorStatus::Parked
                        | osrs_core::actor_snapshot::ActorStatus::MissingDefinition
                )
            {
                let here = (actor.location.tile.x, actor.location.tile.y);
                if drawn.insert(here) {
                    outline(here, i32::from(actor.size), colour, 1.0);
                }
            }
        }
        if !selected {
            continue;
        }
        if let NpcBehavior::Wander { radius } = &spawn.behavior {
            let r = i32::from(*radius);
            outline(
                (tile.0 - r, tile.1 - r),
                2 * r + 1,
                selected_colour.gamma_multiply(0.5),
                1.0,
            );
        }
        if let NpcBehavior::Patrol { waypoints } = &spawn.behavior {
            let mut previous = centre(tile).and_then(|c| project(c, 20.0));
            for (index, waypoint) in waypoints.iter().enumerate() {
                if waypoint.destination.plane.get() != plane {
                    previous = None;
                    continue;
                }
                let wp = (waypoint.destination.tile.x, waypoint.destination.tile.y);
                let point = centre(wp).and_then(|c| project(c, 20.0));
                if let (Some(a), Some(b)) = (previous, point) {
                    painter
                        .line_segment([a, b], Stroke::new(1.5, Color32::from_rgb(180, 255, 140)));
                }
                if let Some(point) = point {
                    painter.circle_filled(point, 4.0, Color32::from_rgb(180, 255, 140));
                    let pause = if waypoint.pause_ticks > 0 {
                        format!("{} ({}t)", index + 1, waypoint.pause_ticks)
                    } else {
                        format!("{}", index + 1)
                    };
                    painter.text(
                        point + egui::vec2(6.0, -6.0),
                        egui::Align2::LEFT_BOTTOM,
                        pause,
                        egui::FontId::proportional(11.0),
                        Color32::WHITE,
                    );
                }
                previous = point;
            }
        }
    }
    // The live actor: where the simulation currently has the selected NPC and where it is going.
    if let Some(id) = app.npcs.selection
        && let Some(actor) = app.npcs.actor(id)
        && actor.location.plane.get() == plane
    {
        let tile = (actor.location.tile.x, actor.location.tile.y);
        outline(
            tile,
            i32::from(actor.size),
            Color32::from_rgb(255, 120, 120),
            2.0,
        );
        let mut previous = centre(tile).and_then(|c| project(c, 20.0));
        for destination in app.npcs.destinations(id) {
            let t = (destination.tile.x, destination.tile.y);
            let point = centre(t).and_then(|c| project(c, 20.0));
            if let (Some(a), Some(b)) = (previous, point) {
                painter.line_segment([a, b], Stroke::new(2.0, Color32::from_rgb(255, 120, 120)));
            }
            previous = point;
        }
        if let Some(head) = centre(tile).and_then(|c| project(c, 10.0)) {
            painter.text(
                head + egui::vec2(0.0, -16.0),
                egui::Align2::CENTER_BOTTOM,
                status_text(&actor.status).split(':').next().unwrap_or(""),
                egui::FontId::proportional(11.0),
                Color32::WHITE,
            );
        }
    }
    // The route a click would take, with the reason when none exists.
    if let (Arm::Destination { route }, Some(id), Some(hover)) = (
        &app.npcs.arm,
        app.npcs.selection,
        app.interaction.hover().tile.map(|p| p.tile),
    ) && let Some(actor) = app.npcs.actor(id)
        && let Some(target) = actor_location(hover, plane)
    {
        let navigation = WorldNavigation {
            world: &app.world,
            map: app.session.document(),
        };
        let spawn = app.session.scenario().spawn(id);
        let restriction = spawn.map_or(MovementRestriction::Normal, |s| s.restriction);
        let (colour, text) = if *route {
            match find_path(
                &navigation,
                actor.location,
                RouteTarget {
                    location: target,
                    width: 1,
                    height: 1,
                },
                actor.size,
                restriction,
                false,
            ) {
                Route::AlreadyThere => (Color32::GRAY, "already there".to_owned()),
                Route::Found { waypoints, exact } => {
                    let mut previous = centre((actor.location.tile.x, actor.location.tile.y))
                        .and_then(|c| project(c, 20.0));
                    for waypoint in &waypoints {
                        let point = centre((waypoint.tile.x, waypoint.tile.y))
                            .and_then(|c| project(c, 20.0));
                        if let (Some(a), Some(b)) = (previous, point) {
                            painter.line_segment([a, b], Stroke::new(2.0, Color32::LIGHT_GREEN));
                        }
                        previous = point;
                    }
                    (
                        Color32::LIGHT_GREEN,
                        format!(
                            "{} waypoints{}",
                            waypoints.len(),
                            if exact { "" } else { " (nearest)" }
                        ),
                    )
                }
                Route::Unreachable => (
                    Color32::LIGHT_RED,
                    "no route in the current collision coverage".to_owned(),
                ),
            }
        } else {
            let step = osrs_playtest::navigation::validated_step(
                &navigation,
                actor.location,
                target,
                actor.size,
                restriction,
                0,
            );
            match step {
                Some(_) => (Color32::LIGHT_GREEN, "first step is open".to_owned()),
                None => (
                    Color32::LIGHT_RED,
                    "blocked: the first step is rejected by collision or size".to_owned(),
                ),
            }
        };
        outline(hover, 1, colour, 2.0);
        if let Some(point) = centre(hover).and_then(|c| project(c, 30.0)) {
            painter.text(
                point,
                egui::Align2::CENTER_BOTTOM,
                text,
                egui::FontId::proportional(12.0),
                colour,
            );
        }
    }
}
