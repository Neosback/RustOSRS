//! Playback controls, NPC search and placement, and editing the selected spawn.

use super::{FACINGS, scenario::plan_and_commit};
use crate::app::{EditorApp, npc_runtime::Arm};
use eframe::egui::{self, Color32};
use osrs_core::{
    actor_snapshot::ActorStatus,
    coords::{MapTile, PlaneIndex},
    ids::SequenceId,
    scenario::{
        ActorBlocking, MovementRestriction, NpcBehavior, NpcSpawn, NpcSpawnId, PatrolWaypoint,
    },
};
use osrs_edit::{plan_remove_spawns, plan_update_spawn};
use osrs_playtest::runtime::RuntimeCommand;
pub(super) fn playback_section(app: &mut EditorApp, ui: &mut egui::Ui) {
    crate::theme::section_heading(ui, "Playtest");
    if !app.npcs.has_runtime() {
        ui.weak("Preparing NPC definitions...");
        return;
    }
    ui.horizontal_wrapped(|ui| {
        if app.npcs.running() {
            if ui.button("Pause").clicked() {
                app.npcs.queue(RuntimeCommand::Pause);
            }
        } else if ui.button("Play").clicked() {
            app.npcs.queue(RuntimeCommand::Start);
        }
        if ui.button("Step").on_hover_text("One server tick").clicked() {
            app.npcs.queue(RuntimeCommand::SingleStep);
        }
        if ui.button("Restart").clicked() {
            app.npcs.queue(RuntimeCommand::Restart);
        }
    });
    if let Some(stamp) = app.npcs.stamp() {
        ui.weak(format!(
            "tick {} \u{b7} cycle {} \u{b7} run {}",
            stamp.server_tick, stamp.client_cycle, stamp.identity.run_generation
        ));
    }
    if app.npcs.stale_rejected() > 0 {
        ui.weak(format!(
            "{} stale results discarded",
            app.npcs.stale_rejected()
        ));
    }
    if !app.npcs.missing_definitions().is_empty() {
        ui.colored_label(
            Color32::LIGHT_RED,
            format!(
                "{} NPC definitions are missing from the open cache",
                app.npcs.missing_definitions().len()
            ),
        );
    }
}

pub(super) fn place_section(app: &mut EditorApp, ui: &mut egui::Ui) {
    crate::theme::section_heading(ui, "Place");
    ui.add(
        egui::TextEdit::singleline(&mut app.npcs.search)
            .hint_text("Search NPCs by name or ID")
            .desired_width(f32::INFINITY),
    );
    let results: Vec<usize> = app.npcs.search_results().to_vec();
    let catalog = app.npcs.catalog().clone();
    if catalog.is_empty() {
        ui.weak("Reading NPC definitions...");
    }
    egui::ScrollArea::vertical()
        .max_height(150.0)
        .id_salt("npc-results")
        .show(ui, |ui| {
            for index in results.into_iter().take(60) {
                let entry = &catalog[index];
                let name = entry.name.as_deref().unwrap_or("(unnamed)");
                let symbol = app
                    .npcs
                    .installation
                    .as_ref()
                    .and_then(|data| data.npc_symbols.get(&entry.id.get()))
                    .cloned();
                let armed = matches!(&app.npcs.arm, Arm::Place { npc, .. } if *npc == entry.id);
                let label = format!(
                    "{name} ({}){}",
                    entry.id.get(),
                    if entry.renderable {
                        ""
                    } else {
                        " \u{b7} no model"
                    }
                );
                if ui.selectable_label(armed, label).clicked() {
                    app.npcs.arm = if armed {
                        Arm::None
                    } else {
                        Arm::Place {
                            npc: entry.id,
                            symbol: symbol.unwrap_or_default(),
                        }
                    };
                }
            }
        });
    match &app.npcs.arm {
        Arm::Place { npc, symbol } => {
            ui.label(format!(
                "Click a tile to place NPC {} (Esc cancels)",
                npc.get()
            ));
            if symbol.is_empty() {
                ui.weak(
                    "No OpenRune symbol known: this spawn cannot be exported until one is set.",
                );
            }
        }
        Arm::Destination { .. } => {
            ui.label("Click a tile to send the selected NPC there (Esc cancels)");
        }
        Arm::None => {}
    }
}

pub(super) fn selection_section(app: &mut EditorApp, ui: &mut egui::Ui) {
    crate::theme::section_heading(ui, "NPC");
    let Some(id) = app.npcs.selection else {
        ui.weak("Click an NPC marker in the viewport, or place a new one.");
        return;
    };
    let Some(committed) = app.session.scenario().spawn(id).cloned() else {
        app.npcs.selection = None;
        app.npcs.draft = None;
        return;
    };
    // Keep an editing draft only while the committed spawn is the one it was made from; an undo,
    // redo or replacement discards it.
    if app
        .npcs
        .draft
        .as_ref()
        .is_none_or(|(base, _)| *base != committed)
    {
        app.npcs.draft = Some((committed.clone(), committed.clone()));
    }
    let name = app
        .npcs
        .catalog()
        .iter()
        .find(|e| e.id == committed.npc)
        .and_then(|e| e.name.clone())
        .unwrap_or_else(|| "(unnamed)".into());
    ui.label(
        egui::RichText::new(format!(
            "{name} \u{b7} NPC {} \u{b7} spawn {}",
            committed.npc.get(),
            id.0
        ))
        .strong(),
    );
    if committed.symbol.is_empty() {
        ui.weak("No OpenRune symbol (export blocked)");
    } else {
        ui.weak(&committed.symbol);
    }
    let actor = app.npcs.actor(id);
    if let Some(actor) = &actor {
        ui.weak(status_text(&actor.status));
    }
    let size = app
        .npcs
        .assets()
        .and_then(|a| a.npcs.get(&committed.npc))
        .map_or(1, |d| d.size);
    ui.weak(format!("footprint {size}\u{d7}{size}"));
    if let Some(definition) = app
        .npcs
        .assets()
        .and_then(|a| a.npcs.get(&committed.npc))
        .cloned()
    {
        let actions: Vec<&str> = definition
            .actions
            .iter()
            .flatten()
            .map(String::as_str)
            .collect();
        ui.weak(format!(
            "{}{} model(s){}",
            definition
                .combat_level
                .map_or(String::new(), |level| format!("level {level} \u{b7} ")),
            definition.body_models.len(),
            if actions.is_empty() {
                String::new()
            } else {
                format!(" \u{b7} {}", actions.join(", "))
            }
        ));
    }

    let mut settle = false;
    let mut delete = false;
    let mut revert = false;
    if let Some((_, draft)) = &mut app.npcs.draft {
        ui.horizontal_wrapped(|ui| {
            ui.label("Tile");
            let mut x = draft.location.tile.x;
            let mut y = draft.location.tile.y;
            let rx = ui.add(egui::DragValue::new(&mut x).prefix("x "));
            let ry = ui.add(egui::DragValue::new(&mut y).prefix("y "));
            settle |= rx.drag_stopped() || rx.lost_focus() || ry.drag_stopped() || ry.lost_focus();
            draft.location.tile = MapTile::new(x, y);
            let mut plane = i32::from(draft.location.plane.get());
            let rp = ui.add(
                egui::DragValue::new(&mut plane)
                    .range(0..=3)
                    .prefix("plane "),
            );
            settle |= rp.drag_stopped() || rp.lost_focus();
            if let Some(plane) = PlaneIndex::new(plane as u8) {
                draft.location.plane = plane;
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("Facing");
            for (label, value) in FACINGS {
                if ui.selectable_label(draft.facing == value, label).clicked() {
                    draft.facing = value;
                    settle = true;
                }
            }
        });
        let mut facing = i32::from(draft.facing);
        let rf = ui.add(egui::Slider::new(&mut facing, 0..=2047).text("units"));
        settle |= rf.drag_stopped() || rf.lost_focus();
        draft.facing = facing as u16;

        behavior_ui(ui, draft, &mut settle);

        ui.horizontal_wrapped(|ui| {
            ui.label("Restriction");
            egui::ComboBox::from_id_salt("npc-restriction")
                .selected_text(format!("{:?}", draft.restriction))
                .show_ui(ui, |ui| {
                    for option in [
                        MovementRestriction::Normal,
                        MovementRestriction::Blocked,
                        MovementRestriction::BlockedNormal,
                        MovementRestriction::Indoors,
                        MovementRestriction::Outdoors,
                        MovementRestriction::NoMove,
                        MovementRestriction::PassThru,
                    ] {
                        settle |= ui
                            .selectable_value(&mut draft.restriction, option, format!("{option:?}"))
                            .changed();
                    }
                });
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("Blocking");
            egui::ComboBox::from_id_salt("npc-blocking")
                .selected_text(format!("{:?}", draft.blocking))
                .show_ui(ui, |ui| {
                    for option in [ActorBlocking::None, ActorBlocking::Npc, ActorBlocking::All] {
                        settle |= ui
                            .selectable_value(&mut draft.blocking, option, format!("{option:?}"))
                            .changed();
                    }
                });
        });
        ui.horizontal_wrapped(|ui| {
            revert = ui
                .button("Revert")
                .on_hover_text("Discard unsaved edits")
                .clicked();
            delete = ui
                .button(format!("{} Delete", egui_phosphor::regular::TRASH))
                .clicked();
        });
    }
    movement_ui(app, ui, id);
    if delete {
        if plan_and_commit(app, |doc| plan_remove_spawns(doc, &[id])) {
            app.npcs.selection = None;
        }
        app.npcs.draft = None;
    } else if revert {
        app.npcs.draft = None;
    } else if settle && let Some((_, draft)) = app.npcs.draft.clone() {
        // An unchanged draft makes an empty patch, which commits nothing and keeps redo intact.
        plan_and_commit(app, |doc| plan_update_spawn(doc, id, |s| *s = draft));
        app.npcs.draft = None;
    }
}

fn behavior_ui(ui: &mut egui::Ui, draft: &mut NpcSpawn, settle: &mut bool) {
    let kind = match &draft.behavior {
        NpcBehavior::Stationary => 0,
        NpcBehavior::Wander { .. } => 1,
        NpcBehavior::Patrol { .. } => 2,
    };
    ui.horizontal_wrapped(|ui| {
        ui.label("Behavior");
        for (index, label) in ["Stationary", "Wander", "Patrol"].into_iter().enumerate() {
            if ui.selectable_label(kind == index, label).clicked() && kind != index {
                draft.behavior = match index {
                    0 => NpcBehavior::Stationary,
                    1 => NpcBehavior::Wander { radius: 5 },
                    _ => NpcBehavior::Patrol {
                        waypoints: vec![PatrolWaypoint {
                            destination: draft.location,
                            pause_ticks: 0,
                        }],
                    },
                };
                *settle = true;
            }
        }
    });
    match &mut draft.behavior {
        NpcBehavior::Stationary => {}
        NpcBehavior::Wander { radius } => {
            let mut value = i32::from(*radius);
            let r = ui.add(egui::Slider::new(&mut value, 0..=64).text("radius"));
            *settle |= r.drag_stopped() || r.lost_focus();
            *radius = value as u16;
        }
        NpcBehavior::Patrol { waypoints } => {
            let mut remove = None;
            for (index, waypoint) in waypoints.iter_mut().enumerate() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!("{}.", index + 1));
                    let (mut x, mut y) = (waypoint.destination.tile.x, waypoint.destination.tile.y);
                    let rx = ui.add(egui::DragValue::new(&mut x).prefix("x "));
                    let ry = ui.add(egui::DragValue::new(&mut y).prefix("y "));
                    *settle |= rx.drag_stopped()
                        || rx.lost_focus()
                        || ry.drag_stopped()
                        || ry.lost_focus();
                    waypoint.destination.tile = MapTile::new(x, y);
                    let mut pause = waypoint.pause_ticks as i32;
                    let rp = ui.add(
                        egui::DragValue::new(&mut pause)
                            .range(0..=10_000)
                            .prefix("pause "),
                    );
                    *settle |= rp.drag_stopped() || rp.lost_focus();
                    waypoint.pause_ticks = pause.max(0) as u32;
                    if ui.small_button(egui_phosphor::regular::TRASH).clicked() {
                        remove = Some(index);
                    }
                });
            }
            if let Some(index) = remove
                && waypoints.len() > 1
            {
                waypoints.remove(index);
                *settle = true;
            }
            if ui.button("Add waypoint at spawn").clicked() {
                waypoints.push(PatrolWaypoint {
                    destination: draft.location,
                    pause_ticks: 0,
                });
                *settle = true;
            }
        }
    }
}

fn movement_ui(app: &mut EditorApp, ui: &mut egui::Ui, id: NpcSpawnId) {
    if !app.npcs.has_runtime() {
        return;
    }
    crate::theme::section_heading(ui, "Move");
    let speed_id = egui::Id::new("npc-speed");
    let mut speed: u8 = ui.data(|d| d.get_temp(speed_id)).unwrap_or(1);
    ui.horizontal_wrapped(|ui| {
        for (value, label) in [(0u8, "Crawl"), (1, "Walk"), (2, "Run")] {
            if ui.selectable_label(speed == value, label).clicked() {
                speed = value;
            }
        }
    });
    ui.data_mut(|d| d.insert_temp(speed_id, speed));
    ui.horizontal_wrapped(|ui| {
        if ui
            .selectable_label(
                app.npcs.arm == (Arm::Destination { route: false }),
                "Step to...",
            )
            .on_hover_text("OpenRune validated stepping: diagonal, then horizontal, then vertical")
            .clicked()
        {
            app.npcs.arm = Arm::Destination { route: false };
        }
        if ui
            .selectable_label(
                app.npcs.arm == (Arm::Destination { route: true }),
                "Route to...",
            )
            .on_hover_text("Bounded breadth-first routing around obstacles")
            .clicked()
        {
            app.npcs.arm = Arm::Destination { route: true };
        }
    });
    let assets = app.npcs.assets().cloned();
    let Some(spawn) = app.session.scenario().spawn(id) else {
        return;
    };
    let Some(definition) = assets.as_ref().and_then(|a| a.npcs.get(&spawn.npc)) else {
        return;
    };
    let movement = definition.movement_sequences;
    let mut sequences: Vec<(&str, SequenceId)> = Vec::new();
    for (label, sequence) in [
        ("idle", movement.idle),
        ("walk", movement.walk),
        ("walk back", movement.walk_back),
        ("walk left", movement.walk_left),
        ("walk right", movement.walk_right),
        ("turn left", movement.turn_left),
        ("turn right", movement.turn_right),
    ] {
        if let Some(sequence) = sequence {
            sequences.push((label, sequence));
        }
    }
    ui.horizontal_wrapped(|ui| {
        ui.label("Animate");
        for (label, sequence) in sequences {
            if ui.small_button(label).clicked() {
                app.npcs.queue(RuntimeCommand::Animate {
                    actor: id,
                    sequence: Some(sequence),
                });
            }
        }
        if ui.small_button("stop").clicked() {
            app.npcs.queue(RuntimeCommand::Animate {
                actor: id,
                sequence: None,
            });
        }
    });
}

pub(super) fn status_text(status: &ActorStatus) -> &'static str {
    match status {
        ActorStatus::Ready => "Ready",
        ActorStatus::Blocked => {
            "Blocked: the next step is rejected by collision, size or occupancy"
        }
        ActorStatus::Parked => "Parked: collision for this area is not available",
        ActorStatus::InvalidPlacement => {
            "Invalid placement: the spawn tile is blocked or restricted"
        }
        ActorStatus::MissingDefinition => "Missing NPC definition in the open cache",
    }
}
