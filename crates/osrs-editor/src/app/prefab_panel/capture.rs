//! Building capture review over an immutable authoritative snapshot. Cache reads run off-thread.
mod analysis;
use crate::app::EditorApp;
pub(in crate::app) use analysis::start;
use eframe::egui;
use osrs_core::coords::{MapTile, SourcePlane};
use osrs_edit::{
    BuildingCaptureSpec, HeightVertexCoord, MapDocument, MapPosition, ObjectPlacement,
    PrefabRelation,
};
use osrs_scene::{
    building_census::{
        CensusArea, Ground, ObjectInfo, ObjectTable,
        description::{BuildingDescription, describe},
    },
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
    inputs: std::sync::Arc<ReviewInputs>,
    rooms: Vec<RoomChoice>,
    overrides: Vec<(ObjectPlacement, bool)>,
    labels: BTreeMap<String, String>,
    predictions: BTreeMap<(osrs_core::ids::ObjectId, u8), String>,
    source: u64,
    revision: u64,
    description: BuildingDescription,
    objects: Vec<ObjectPlacement>,
    included: Vec<bool>,
    planes: BTreeSet<u8>,
    provenance: String,
    object_sizes: BTreeMap<osrs_core::ids::ObjectId, (u16, u16)>,
    confirmed: bool,
}

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

pub(super) fn poll(app: &mut EditorApp) {
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

fn intent(review: &Review, document: &MapDocument) -> Result<BuildingCaptureSpec, String> {
    let floors: BTreeSet<_> = review
        .rooms
        .iter()
        .filter(|r| r.included && review.planes.contains(&r.plane))
        .flat_map(|r| {
            r.tiles
                .iter()
                .filter_map(|&t| SourcePlane::new(r.plane).map(|p| MapPosition::new(t, p)))
        })
        .collect();
    let objects: Vec<_> = review
        .objects
        .iter()
        .zip(&review.included)
        .filter_map(|(&o, &yes)| yes.then_some(o))
        .collect();
    let mut support = BTreeSet::new();
    for (tile, plane) in floors
        .iter()
        .map(|p| (p.tile, p.plane))
        .chain(objects.iter().map(|p| (p.position.tile, p.position.plane)))
    {
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            let coord = HeightVertexCoord::new(MapTile::new(tile.x + dx, tile.y + dy), plane);
            if document.height_at(coord).is_none() {
                return Err(format!(
                    "Missing height at {coord:?}; load the surrounding map."
                ));
            }
            support.insert(coord);
        }
    }
    for object in &objects {
        let &(w, h) = review
            .object_sizes
            .get(&object.object_id)
            .ok_or("Missing object dimensions")?;
        let (w, h) = if object.orientation.get() & 1 == 1 {
            (h, w)
        } else {
            (w, h)
        };
        for dx in 0..=w {
            for dy in 0..=h {
                let c = HeightVertexCoord::new(
                    MapTile::new(
                        object.position.tile.x + i32::from(dx),
                        object.position.tile.y + i32::from(dy),
                    ),
                    object.position.plane,
                );
                if document.height_at(c).is_none() {
                    return Err(format!("Missing model height samples at {c:?}"));
                }
                support.insert(c);
            }
        }
    }
    let mut remap = BTreeMap::new();
    let mut index = 0;
    for (old, &yes) in review.included.iter().enumerate() {
        if yes {
            remap.insert(old, index);
            index += 1;
        }
    }
    let relations = review
        .description
        .connections
        .iter()
        .filter_map(|r| {
            Some(PrefabRelation {
                from: *remap.get(&r.from)?,
                to: *remap.get(&r.to)?,
                kind: format!("{:?}", r.kind),
                evidence: format!("{:?}", r.evidence),
                human: false,
            })
        })
        .collect();
    let mut labels = review.labels.clone();
    for (old, new) in remap {
        if let Some(m) = review.description.members.get(old) {
            labels.insert(
                format!("inferred:member:{new}:role"),
                format!("{:?}", m.role),
            );
            labels.insert(
                format!("inferred:member:{new}:evidence"),
                format!("{:?}", m.evidence),
            );
        }
    }
    Ok(BuildingCaptureSpec {
        floors,
        support,
        objects,
        object_sizes: review.object_sizes.clone(),
        provenance: review.provenance.clone(),
        relations,
        labels,
        reviewed: review.confirmed,
    })
}

pub(super) fn show(app: &mut EditorApp, ui: &mut egui::Ui) {
    let seed = app
        .interaction
        .selection_tiles()
        .iter()
        .copied()
        .min()
        .or_else(|| app.interaction.inspect_tile());
    if ui.add_enabled(seed.is_some(),egui::Button::new("Review and capture building")).on_hover_text("Copy owned floors and exact pieces, including outside-anchored walls, without surrounding ground").clicked()
        && let Some((x,y))=seed { start(app,MapTile::new(x,y)); }
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
                        "general", "living", "kitchen", "bedroom", "storage", "bank", "church",
                        "workshop", "tavern",
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
                next.included[i] = next.planes.contains(&o.position.plane.index().get())
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
