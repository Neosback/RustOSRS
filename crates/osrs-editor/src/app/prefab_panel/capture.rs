//! Building capture review over an immutable authoritative snapshot. Cache reads run off-thread.
mod analysis;
mod intent;
mod options;
use crate::app::EditorApp;
pub(in crate::app) use analysis::start;
use eframe::egui;
use intent::intent;
use options::Options;
use osrs_core::coords::{MapTile, SourcePlane};
use osrs_edit::{
    BuildingCaptureSpec, HeightVertexCoord, MapDocument, MapPosition, ObjectPlacement,
    PrefabRelation,
};
use osrs_scene::building_census::description::BuildingDescription;
#[cfg(not(target_arch = "wasm32"))]
use osrs_scene::{
    building_census::{CensusArea, Ground, ObjectInfo, ObjectTable, description::describe},
    object_semantics::Placed,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub(in crate::app) struct CaptureReview {
    #[cfg(not(target_arch = "wasm32"))]
    pending: Option<std::sync::mpsc::Receiver<Result<Review, String>>>,
    review: Option<Review>,
}
#[derive(Clone)]
struct Review {
    #[cfg(not(target_arch = "wasm32"))]
    inputs: std::sync::Arc<ReviewInputs>,
    rooms: Vec<RoomChoice>,
    overrides: Vec<(ObjectPlacement, bool)>,
    labels: BTreeMap<String, String>,
    predictions: BTreeMap<(osrs_core::ids::ObjectId, u8), String>,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    source: u64,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    revision: u64,
    description: BuildingDescription,
    objects: Vec<ObjectPlacement>,
    included: Vec<bool>,
    planes: BTreeSet<u8>,
    provenance: String,
    object_sizes: BTreeMap<osrs_core::ids::ObjectId, (u16, u16)>,
    confirmed: bool,
    options: Options,
}

#[cfg(not(target_arch = "wasm32"))]
struct ReviewInputs {
    area: CensusArea,
    placements: Vec<Placed>,
    table: ObjectTable,
}
#[derive(Clone)]
struct RoomChoice {
    plane: u8,
    tiles: Vec<MapTile>,
    included: bool,
    role: String,
}

pub(super) fn poll(
    #[cfg_attr(target_arch = "wasm32", allow(unused_variables))] app: &mut EditorApp,
) {
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(receiver) = &app.prefabs.capture_review.pending {
        match receiver.try_recv() {
            Ok(result) => {
                app.prefabs.capture_review.pending = None;
                match result {
                    Ok(review)
                        if review.source == app.project.generation()
                            && review.revision == app.session.map_revision() =>
                    {
                        app.prefabs.capture_review.review = Some(review);
                        app.prefabs.clear_notice();
                    }
                    Ok(_) => app
                        .prefabs
                        .say("The map changed during analysis. Review the building again."),
                    Err(error) => app.prefabs.say(error),
                }
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(_) => {
                app.prefabs.capture_review.pending = None;
                app.prefabs
                    .say("Building analysis stopped before completion.");
            }
        }
    }
}

pub(super) fn show(app: &mut EditorApp, ui: &mut egui::Ui) {
    let Some(review) = &mut app.prefabs.capture_review.review else {
        return;
    };
    if review.source != app.project.generation() || review.revision != app.session.map_revision() {
        ui.label("Map changed: refresh the building review.");
        return;
    }
    ui.label(format!(
        "{} placements; {} connections",
        review.objects.len(),
        review.description.connections.len()
    ));
    for issue in &review.description.review {
        ui.colored_label(ui.visuals().warn_fg_color, issue);
    }
    let mut rebuild = false;
    if review.options.show(ui) {
        review.confirmed = false;
    }
    for &plane in review.description.floors.keys() {
        let mut included = review.planes.contains(&plane);
        if ui
            .checkbox(&mut included, format!("Include floor on plane {plane}"))
            .changed()
        {
            if included {
                review.planes.insert(plane);
            } else {
                review.planes.remove(&plane);
            }
            review.confirmed = false;
            rebuild = true;
        }
    }
    egui::CollapsingHeader::new("Rooms: split or merge ownership").show(ui, |ui| {
        if ui.button("Merge all candidate rooms").clicked() {
            for r in &mut review.rooms {
                r.included = true;
            }
            rebuild = true;
        }
        for (index, room) in review.rooms.iter_mut().enumerate() {
            if ui
                .checkbox(
                    &mut room.included,
                    format!(
                        "Room {} · {} tiles · plane {}",
                        index + 1,
                        room.tiles.len(),
                        room.plane
                    ),
                )
                .changed()
            {
                rebuild = true;
            }
            let before = room.role.clone();
            egui::ComboBox::from_id_salt(("capture-room-role", index))
                .selected_text(&room.role)
                .show_ui(ui, |ui| {
                    for role in [
                        "general",
                        "living",
                        "kitchen",
                        "bedroom",
                        "Pub",
                        "smithy",
                        "chapel",
                        "bank",
                        "shop",
                        "dining",
                        "study",
                        "storage",
                        "workshop",
                        "throne",
                        "prison",
                        "crypt",
                        "classroom",
                        "magic_shop",
                        "armoury",
                        "archery_shop",
                        "clothes_shop",
                        "crafting_shop",
                        "fishing_shop",
                        "food_shop",
                        "farming_shop",
                    ] {
                        ui.selectable_value(&mut room.role, role.into(), role);
                    }
                });
            if room.role != before
                && let Some(first) = room.tiles.first()
            {
                review.labels.insert(
                    format!("human:room:p{}:{}:{}", room.plane, first.x, first.y),
                    room.role.clone(),
                );
                review.confirmed = false;
            }
        }
    });
    egui::CollapsingHeader::new("Pieces and attachment evidence").show(ui, |ui| {
        egui::ScrollArea::vertical()
            .max_height(210.0)
            .show(ui, |ui| {
                for (i, m) in review.description.members.iter().enumerate() {
                    if !review.options.includes(m.role) {
                        continue;
                    }
                    let p = m.placement;
                    let key = format!("human:object:{}:{}", p.id.get(), p.kind);
                    let mut label = review.labels.get(&key).cloned().unwrap_or_default();
                    if let Some(suggestion) = review.predictions.get(&(p.id, p.kind)) {
                        ui.weak(format!("Learned suggestion: {suggestion}"));
                    }
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut label)
                                .hint_text("Confirm object role (optional)"),
                        )
                        .changed()
                    {
                        if label.trim().is_empty() {
                            review.labels.remove(&key);
                        } else {
                            review.labels.insert(key, label.trim().to_owned());
                        }
                        review.confirmed = false;
                    }

                    if ui
                        .checkbox(
                            &mut review.included[i],
                            format!(
                                "{} {:?} at {},{} p{} r{}",
                                p.id.get(),
                                m.role,
                                p.tile.0,
                                p.tile.1,
                                p.plane,
                                p.orientation
                            ),
                        )
                        .on_hover_text(format!("Evidence: {:?}", m.evidence))
                        .changed()
                    {
                        review.confirmed = false;
                        let placement = review.objects[i];
                        review.overrides.retain(|(p, _)| *p != placement);
                        review.overrides.push((placement, review.included[i]));
                    }
                }
            });
    });
    ui.checkbox(
        &mut review.confirmed,
        "I reviewed floors, shared boundaries and included pieces",
    );
    let save = ui
        .add_enabled(
            review.confirmed && !review.planes.is_empty(),
            egui::Button::new("Capture reviewed building"),
        )
        .clicked();
    let cancel = ui.button("Cancel building capture").clicked();
    #[cfg(not(target_arch = "wasm32"))]
    let replacement = if rebuild {
        let mut next = review.clone();
        next.confirmed = false;
        let (send, receive) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut active = next.description.clone();
            active.floors.clear();
            for room in next
                .rooms
                .iter()
                .filter(|r| r.included && next.planes.contains(&r.plane))
            {
                active
                    .floors
                    .entry(room.plane)
                    .or_default()
                    .extend(room.tiles.iter().copied());
            }
            let active = osrs_scene::building_census::description::describe_floors(
                &next.inputs.area,
                &next.inputs.placements,
                &next.inputs.table,
                active,
            );
            for (i, o) in next.objects.iter().enumerate() {
                let roof = next.description.members.get(i).is_some_and(|m| {
                    m.role == osrs_scene::building_census::description::MemberRole::Roof
                });
                next.included[i] = (roof || next.planes.contains(&o.position.plane.index().get()))
                    && next.overrides.iter().find(|(p, _)| p == o).map_or_else(
                        || {
                            active.members.iter().any(|m| {
                                m.placement.id == o.object_id
                                    && m.placement.tile == (o.position.tile.x, o.position.tile.y)
                                    && m.placement.plane == o.position.plane.index().get()
                                    && m.placement.kind == o.kind.get()
                                    && m.placement.orientation == o.orientation.get()
                            })
                        },
                        |(_, yes)| *yes,
                    );
            }
            let _ = send.send(Ok(next));
        });
        Some(receive)
    } else {
        None
    };
    let spec = save.then(|| intent(review, app.session.document()));
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(pending) = replacement {
        app.prefabs.capture_review.pending = Some(pending);
        app.prefabs.capture_review.review = None;
        return;
    }
    #[cfg(target_arch = "wasm32")]
    let _ = rebuild;
    if cancel {
        app.prefabs.capture_review.review = None;
    }
    if let Some(spec) = spec {
        let name = if app.prefabs.form.name.trim().is_empty() {
            "Captured building"
        } else {
            &app.prefabs.form.name
        };
        let result = spec
            .and_then(|s| {
                osrs_edit::capture_building_prefab(app.session.document(), &s, name)
                    .map_err(|e| e.to_string())
            })
            .and_then(|p| app.prefabs.add(p));
        match result {
            Ok(_) => {
                app.prefabs.capture_review.review = None;
                app.prefabs
                    .say("Building captured with sparse terrain ownership.");
            }
            Err(e) => app.prefabs.say(e),
        }
    }
}

/// Prefer the selected interior tile nearest the selection centre. An inspected tile is enough
/// when no tiles are selected; never turn a wall's outside anchor into a terrain rectangle.
pub(super) fn seed(app: &EditorApp) -> Option<MapTile> {
    let tiles = app.interaction.selection_tiles();
    if tiles.is_empty() {
        return app
            .interaction
            .inspect_tile()
            .map(|(x, y)| MapTile::new(x, y));
    }
    let min_x = tiles.iter().map(|t| t.0).min()?;
    let max_x = tiles.iter().map(|t| t.0).max()?;
    let min_y = tiles.iter().map(|t| t.1).min()?;
    let max_y = tiles.iter().map(|t| t.1).max()?;
    let cx = (i64::from(min_x) + i64::from(max_x)) / 2;
    let cy = (i64::from(min_y) + i64::from(max_y)) / 2;
    tiles
        .iter()
        .min_by_key(|t| (i64::from(t.0) - cx).abs() + (i64::from(t.1) - cy).abs())
        .map(|&(x, y)| MapTile::new(x, y))
}

#[cfg(test)]
mod tests;
