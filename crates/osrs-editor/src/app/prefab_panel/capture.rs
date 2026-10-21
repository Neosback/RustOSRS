//! Building capture review over an immutable authoritative snapshot. Cache reads run off-thread.
mod analysis;
mod intent;
mod membership;
mod metadata;
mod options;
pub(in crate::app) mod refine;
#[cfg(not(target_arch = "wasm32"))]
mod worker;
use crate::app::EditorApp;
pub(in crate::app) use analysis::{start, start_selection};
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
    building_census::{CensusArea, Ground, ObjectInfo, ObjectTable},
    object_semantics::Placed,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub(in crate::app) struct CaptureReview {
    #[cfg(not(target_arch = "wasm32"))]
    pending: Option<worker::Job>,
    #[cfg(not(target_arch = "wasm32"))]
    worker: Option<worker::Worker>,
    review: Option<Review>,
    refining: bool,
    open: bool,
    #[cfg(not(target_arch = "wasm32"))]
    additive: bool,
    layer: refine::Layer,
}
#[derive(Clone)]
struct Review {
    #[cfg(not(target_arch = "wasm32"))]
    inputs: std::sync::Arc<ReviewInputs>,
    rooms: Vec<RoomChoice>,
    sides: Vec<(MapTile, u8)>,
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

impl Review {
    fn is_current(&self, revision: u64, source: u64) -> bool {
        (self.revision, self.source) == (revision, source)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn admit(self, revision: u64, source: u64) -> Result<Self, String> {
        if !self.is_current(revision, source) {
            return Err("The map changed during analysis. Review the building again.".into());
        }
        Ok(self)
    }

    fn capture_spec(
        &self,
        document: &MapDocument,
        revision: u64,
        source: u64,
    ) -> Result<BuildingCaptureSpec, String> {
        if !self.is_current(revision, source) {
            return Err("The map changed. Review the building again before capture.".into());
        }
        intent(self, document)
    }
}

impl CaptureReview {
    fn clear(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        let worker = self.worker.take();
        *self = Self::default();
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.worker = worker;
        }
    }

    fn is_current(&self, revision: u64, source: u64) -> bool {
        let current = self
            .review
            .as_ref()
            .is_none_or(|r| r.is_current(revision, source));
        #[cfg(not(target_arch = "wasm32"))]
        {
            current
                && self
                    .pending
                    .as_ref()
                    .is_none_or(|j| j.is_current(revision, source))
        }
        #[cfg(target_arch = "wasm32")]
        {
            current
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn start(
        &mut self,
        revision: u64,
        source: u64,
        work: impl FnOnce() -> Result<Review, String> + Send + 'static,
    ) {
        self.pending = None;
        self.review = None;
        if self.worker.as_ref().is_none_or(worker::Worker::is_finished) {
            self.worker = Some(worker::Worker::new());
        }
        if let Some(worker) = &mut self.worker {
            self.pending = Some(worker.start(revision, source, work));
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn poll_response(&mut self, revision: u64, source: u64) -> Option<Result<Review, String>> {
        if !self.is_current(revision, source) {
            self.clear();
            return Some(Err(
                "The map changed during analysis. Review the building again.".into(),
            ));
        }
        let job = self.pending.as_ref()?;
        let result = match job.receive.try_recv() {
            Ok(result) => result.and_then(|review| review.admit(revision, source)),
            Err(std::sync::mpsc::TryRecvError::Empty) => return None,
            Err(_) => Err("Building analysis stopped before completion.".into()),
        };
        self.pending = None;
        if result.is_err() {
            self.clear();
        }
        Some(result)
    }
}

pub(super) fn poll(
    #[cfg_attr(target_arch = "wasm32", allow(unused_variables))] app: &mut EditorApp,
) {
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(result) = app
        .prefabs
        .capture_review
        .poll_response(app.session.revision(), app.project.generation())
    {
        match result {
            Ok(review) => {
                let additive = app.prefabs.capture_review.additive;
                refine::project(app, &review, additive);
                app.prefabs.capture_review.review = Some(review);
                app.prefabs
                    .say("Building selected. Shift adds, Alt removes; Enter reviews capture.");
            }
            Err(error) => app.prefabs.say(error),
        }
    }
}

pub(super) fn show(app: &mut EditorApp, ui: &mut egui::Ui) {
    if !app.prefabs.capture_review.open {
        return;
    }
    if refine::retire_stale(app) {
        ui.label("Map changed: refresh the building review.");
        return;
    }
    refine::controls(app, ui);
    let Some(review) = &mut app.prefabs.capture_review.review else {
        return;
    };
    if !review.is_current(app.session.revision(), app.project.generation()) {
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
    let replacement = rebuild.then(|| {
        let mut next = review.clone();
        next.confirmed = false;
        next
    });
    let spec = save.then(|| {
        review.capture_spec(
            app.session.document(),
            app.session.revision(),
            app.project.generation(),
        )
    });
    if cancel {
        refine::cancel(app);
        return;
    }
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(mut next) = replacement {
        app.prefabs
            .capture_review
            .start(next.revision, next.source, move || {
                membership::refresh(&mut next);
                Ok(next)
            });
        return;
    }
    #[cfg(target_arch = "wasm32")]
    let _ = replacement;
    if let Some(spec) = spec {
        let name = if app.prefabs.form.name.trim().is_empty() {
            "Captured building"
        } else {
            &app.prefabs.form.name
        };
        let reviewed = app.prefabs.capture_review.review.clone();
        let result = spec
            .and_then(|s| {
                osrs_edit::capture_building_prefab(app.session.document(), &s, name)
                    .map_err(|e| e.to_string())
            })
            .and_then(|mut p| {
                if let Some(review) = &reviewed {
                    metadata::apply(review, &mut p)?;
                }
                app.prefabs.add(p)
            });
        match result {
            Ok(_) => {
                refine::cancel(app);
                app.prefabs.studio_request = app.prefabs.selected.clone();
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
