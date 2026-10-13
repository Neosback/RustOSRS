//! Editing objects: move, turn, copy and delete the selected placements.
//!
//! Completed object actions commit through the authoritative semantic editor session. The existing
//! world-worker `LocEdit` layer remains a downstream projection that rebuilds affected regions and
//! re-selects moved/new placements after the rebuild arrives.

mod commit;

use super::EditorApp;
use super::editing::EditStep;
use crate::icons;
use crate::paint::Tool;
use commit::{ObjectCommitPlan, build_object_commit_plan};
pub(super) use commit::{ObjectHistoryRoute, build_object_history_projection};
use eframe::egui::{self, Color32, Pos2, Rect, Stroke};
use egui_phosphor::regular as ph;
use osrs_world::{LocEdit, LocSpec};
use web_time::Instant;

/// Highest tile coordinate on the map (255 regions of 64 tiles, less one).
const MAP_LIMIT: i32 = 16383;

/// The selected objects being dragged to a new tile.
#[derive(Debug, Clone)]
pub struct ObjectDrag {
    /// Tile the drag began on.
    pub anchor: (i32, i32),
    /// The placements being moved, as they were.
    pub specs: Vec<LocSpec>,
    /// Tiles moved so far.
    pub delta: (i32, i32),
}

impl EditorApp {
    /// What the map file says about each selected object.
    pub(super) fn selected_specs(&self) -> Vec<LocSpec> {
        let mut objects = self.selection_objects.clone();
        if let Some(selected) = &self.selected
            && !objects
                .iter()
                .any(|o| o.region == selected.region && o.index == selected.index)
        {
            objects.push(selected.clone());
        }
        objects
            .iter()
            .filter_map(|object| {
                let loc = self.infos.get(&object.region)?.locs.get(object.index)?;
                Some(LocSpec {
                    object_id: loc.definition.identity.id.get(),
                    tile: loc.tile,
                    plane: loc.plane,
                    loc_type: loc.loc_type,
                    orientation: loc.orientation,
                })
            })
            .collect()
    }

    /// Commit one complete object action semantically, project it to the world worker, and select
    /// the placements it made. Returns whether a non-empty semantic transaction committed.
    fn edit_objects(&mut self, removed: Vec<LocSpec>, added: Vec<LocSpec>, label: &str) -> bool {
        if removed.is_empty() && added.is_empty() {
            return false;
        }

        let ObjectCommitPlan {
            patch,
            removed,
            added,
        } = match build_object_commit_plan(self.session.document(), &removed, &added) {
            Ok(plan) => plan,
            Err(error) => {
                self.notice = Some((
                    format!("Object action was not committed: {error:?}"),
                    Instant::now(),
                ));
                return false;
            }
        };

        match self.session.commit(patch) {
            Ok(Some(_)) => {}
            Ok(None) => {
                self.notice = Some((
                    "Object action made no semantic changes".to_owned(),
                    Instant::now(),
                ));
                return false;
            }
            Err(error) => {
                self.notice = Some((
                    format!("Object action was rejected by the semantic session: {error:?}"),
                    Instant::now(),
                ));
                return false;
            }
        }

        let mut edits: Vec<LocEdit> = removed.iter().map(|s| LocEdit::Remove(*s)).collect();
        edits.extend(added.iter().map(|s| LocEdit::Add(*s)));
        self.send_loc_edits(edits);
        self.push_step(EditStep::semantic_object(removed, added.clone()));
        self.clear_selection();
        self.pending_reselect = added;
        self.notice = Some((label.to_owned(), Instant::now()));
        true
    }

    pub(super) fn delete_selected_objects(&mut self) {
        let specs = self.selected_specs();
        let count = specs.len();
        self.edit_objects(
            specs,
            Vec::new(),
            &format!(
                "Deleted {count} object{}",
                if count == 1 { "" } else { "s" }
            ),
        );
    }

    /// Turn the selected objects `quarters` quarter turns (clockwise seen from above is `+1`).
    pub(super) fn rotate_selected_objects(&mut self, quarters: i8) {
        let specs = self.selected_specs();
        let turned: Vec<LocSpec> = specs
            .iter()
            .map(|spec| LocSpec {
                orientation: (spec.orientation as i8 + quarters).rem_euclid(4) as u8,
                ..*spec
            })
            .collect();
        let count = specs.len();
        self.edit_objects(
            specs,
            turned,
            &format!("Turned {count} object{}", if count == 1 { "" } else { "s" }),
        );
    }

    /// Why `specs` cannot sit `delta` tiles and `planes` floors away, if they cannot: off the map,
    /// on a floor that does not exist, or in a square the cache has no map for (nothing is loaded
    /// there, so the object could not be placed or seen).
    pub(super) fn move_blocked(
        &self,
        specs: &[LocSpec],
        delta: (i32, i32),
        planes: i32,
    ) -> Option<&'static str> {
        for spec in specs {
            let tile = (spec.tile.0 + delta.0, spec.tile.1 + delta.1);
            if !(0..=MAP_LIMIT).contains(&tile.0) || !(0..=MAP_LIMIT).contains(&tile.1) {
                return Some("that is off the edge of the map");
            }
            if !(0..=3).contains(&(i32::from(spec.plane) + planes)) {
                return Some("there is no floor there");
            }
            if !self
                .infos
                .contains_key(&(tile.0.div_euclid(64), tile.1.div_euclid(64)))
            {
                return Some("the cache has no map there");
            }
        }
        None
    }

    /// Move the selected objects `planes` floors up (positive) or down; the view follows them.
    pub(super) fn shift_floor(&mut self, planes: i32) {
        let specs = self.selected_specs();
        if specs.is_empty() {
            return;
        }
        if let Some(why) = self.move_blocked(&specs, (0, 0), planes) {
            self.notice = Some((format!("Can't move them: {why}"), Instant::now()));
            return;
        }
        let moved: Vec<LocSpec> = specs
            .iter()
            .map(|spec| LocSpec {
                plane: (i32::from(spec.plane) + planes) as u8,
                ..*spec
            })
            .collect();
        let count = specs.len();
        let target = moved[0].plane;
        if self.edit_objects(
            specs,
            moved,
            &format!(
                "Moved {count} object{} {} a floor",
                if count == 1 { "" } else { "s" },
                if planes > 0 { "up" } else { "down" }
            ),
        ) {
            self.view_plane = target;
        }
    }

    /// Move the selected objects by `delta` tiles.
    pub(super) fn move_objects(&mut self, specs: Vec<LocSpec>, delta: (i32, i32)) {
        if delta == (0, 0) {
            return;
        }
        if let Some(why) = self.move_blocked(&specs, delta, 0) {
            self.notice = Some((format!("Can't move them: {why}"), Instant::now()));
            return;
        }
        let moved: Vec<LocSpec> = specs
            .iter()
            .map(|spec| LocSpec {
                tile: (spec.tile.0 + delta.0, spec.tile.1 + delta.1),
                ..*spec
            })
            .collect();
        let count = specs.len();
        self.edit_objects(
            specs,
            moved,
            &format!("Moved {count} object{}", if count == 1 { "" } else { "s" }),
        );
    }

    /// Copy the selected objects one tile east (the copies are selected, ready to drag).
    pub(super) fn duplicate_selected_objects(&mut self) {
        let specs = self.selected_specs();
        if let Some(why) = self.move_blocked(&specs, (1, 0), 0) {
            self.notice = Some((format!("Can't copy them: {why}"), Instant::now()));
            return;
        }
        let copies: Vec<LocSpec> = specs
            .iter()
            .map(|spec| LocSpec {
                tile: (spec.tile.0 + 1, spec.tile.1),
                ..*spec
            })
            .collect();
        let count = specs.len();
        self.edit_objects(
            Vec::new(),
            copies,
            &format!("Copied {count} object{}", if count == 1 { "" } else { "s" }),
        );
    }

    /// Select the placements an edit made, once the rebuilt regions are in.
    pub(super) fn resolve_reselect(&mut self) {
        if self.pending_reselect.is_empty() {
            return;
        }
        let mut found = Vec::new();
        for spec in &self.pending_reselect {
            let Some(info) = self.infos.get(&spec.region()) else {
                continue;
            };
            let position = info.locs.iter().position(|loc| {
                loc.definition.identity.id.get() == spec.object_id
                    && loc.tile == spec.tile
                    && loc.plane == spec.plane
                    && loc.loc_type == spec.loc_type
                    && loc.orientation == spec.orientation
            });
            if let Some(index) = position {
                found.push(crate::pick::ObjectPick {
                    region: spec.region(),
                    index,
                });
            }
        }
        if found.len() == self.pending_reselect.len() {
            self.selected = found.last().cloned();
            self.selection_objects = found;
            self.pending_reselect.clear();
        }
    }

    /// Keys: delete, turn, duplicate (with objects selected, in Select and in the Objects tool).
    pub(super) fn object_actions(&mut self, ctx: &egui::Context) {
        if self.keys_blocked(ctx)
            || self.quick_open.is_some()
            || !matches!(self.prefs.tool, Tool::Select | Tool::Objects)
            || (self.selection_objects.is_empty() && self.selected.is_none())
        {
            return;
        }
        let (delete, rotate, duplicate, reverse) = ctx.input(|i| {
            (
                self.keymap.pressed(i, "object.delete") || i.key_pressed(egui::Key::Backspace),
                self.keymap.pressed(i, "object.rotate"),
                self.keymap.pressed(i, "object.duplicate"),
                i.modifiers.shift && i.key_pressed(egui::Key::R),
            )
        });
        if delete {
            self.delete_selected_objects();
        } else if rotate {
            self.rotate_selected_objects(1);
        } else if reverse {
            self.rotate_selected_objects(-1);
        } else if duplicate {
            self.duplicate_selected_objects();
        }
    }

    /// Start dragging the selection when a drag begins on a selected object; the drag ends by
    /// committing the move. Returns whether a drag is in progress (the marquee stays out of it).
    pub(super) fn object_drag_input(
        &mut self,
        ctx: &egui::Context,
        response: &egui::Response,
    ) -> bool {
        if self.prefs.tool != Tool::Objects {
            self.object_drag = None;
            return false;
        }
        let primary = egui::PointerButton::Primary;
        if response.drag_started_by(primary)
            && let Some(object) = &self.hover.object
            && let Some(tile) = self.hover.tile.map(|p| p.tile)
            && self
                .selection_objects
                .iter()
                .any(|o| o.region == object.region && o.index == object.index)
        {
            self.object_drag = Some(ObjectDrag {
                anchor: tile,
                specs: self.selected_specs(),
                delta: (0, 0),
            });
        }
        let Some(drag) = &mut self.object_drag else {
            return false;
        };
        if let Some(tile) = self.hover.tile.map(|p| p.tile) {
            drag.delta = (tile.0 - drag.anchor.0, tile.1 - drag.anchor.1);
        }
        if response.drag_stopped_by(primary) || !ctx.input(|i| i.pointer.primary_down()) {
            let drag = self.object_drag.take();
            if let Some(drag) = drag {
                self.move_objects(drag.specs, drag.delta);
            }
            return false;
        }
        true
    }

    /// The footprints the dragged objects would land on.
    pub(super) fn draw_object_drag(
        &self,
        painter: &egui::Painter,
        viewport: Rect,
        pixels_per_point: f32,
        width: u32,
        height: u32,
    ) {
        let Some(drag) = &self.object_drag else {
            return;
        };
        let colors = &self.prefs.colors;
        let (mut fill, mut edge) = (
            colors.get(crate::style::ColorKey::HoverTileFill),
            colors.get(crate::style::ColorKey::HoverTileOutline),
        );
        // Where the objects cannot go the footprints turn red.
        if self.move_blocked(&drag.specs, drag.delta, 0).is_some() {
            fill = Color32::from_rgba_unmultiplied(235, 70, 70, 70);
            edge = Color32::from_rgb(245, 90, 90);
        }
        for spec in &drag.specs {
            let target = (spec.tile.0 + drag.delta.0, spec.tile.1 + drag.delta.1);
            let Some(corners) = crate::interact::tile_corners(&self.infos, target, self.view_plane)
            else {
                continue;
            };
            let points: Vec<Pos2> = corners
                .iter()
                .filter_map(|c| self.screen_point(*c, viewport, pixels_per_point, width, height))
                .collect();
            if points.len() == 4 {
                painter.add(egui::Shape::convex_polygon(
                    points,
                    fill.gamma_multiply(2.0),
                    Stroke::new(2.0, edge),
                ));
            }
        }
    }

    /// The Objects tool's drawer: what it can do to the selected objects, in groups.
    pub(super) fn object_drawer(&mut self, ui: &mut egui::Ui) {
        let count = self
            .selection_objects
            .len()
            .max(usize::from(self.selected.is_some()));
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Objects").strong().size(15.0));
            ui.label(match count {
                0 => "Click an object (Shift adds, drag a box for several)".to_owned(),
                1 => "1 selected. Drag its arrows to move it, its ring to turn it.".to_owned(),
                n => format!("{n} selected. Drag the arrows to move them all."),
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                self.drawer_collapse(ui);
            });
        });
        ui.add_space(6.0);
        let any = count > 0;
        let key = |action: &str| {
            self.keymap
                .binding(action)
                .map(|b| b.describe())
                .unwrap_or_default()
        };
        let (rotate_key, duplicate_key, delete_key) = (
            key("object.rotate"),
            key("object.duplicate"),
            key("object.delete"),
        );
        let mut step = None;
        let mut floor = 0;
        let mut turn = 0;
        let (mut copy, mut delete) = (false, false);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(5.0, 5.0);
            ui.spacing_mut().button_padding = egui::vec2(10.0, 7.0);
            group(ui, "Move one tile", |ui| {
                ui.horizontal(|ui| {
                    for (icon, tip, delta) in [
                        (ph::ARROW_LEFT, "West", (-1, 0)),
                        (ph::ARROW_UP, "North", (0, 1)),
                        (ph::ARROW_DOWN, "South", (0, -1)),
                        (ph::ARROW_RIGHT, "East", (1, 0)),
                    ] {
                        if ui
                            .add_enabled(any, egui::Button::new(icon))
                            .on_hover_text(format!("Move one tile {tip}"))
                            .clicked()
                        {
                            step = Some(delta);
                        }
                    }
                });
            });
            group(ui, "Floor", |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(any, egui::Button::new(format!("{}  Up", ph::ARROW_LINE_UP)))
                        .on_hover_text("Move up one floor (the one above, z + 1)")
                        .clicked()
                    {
                        floor = 1;
                    }
                    if ui
                        .add_enabled(
                            any,
                            egui::Button::new(format!("{}  Down", ph::ARROW_LINE_DOWN)),
                        )
                        .on_hover_text("Move down one floor (z - 1)")
                        .clicked()
                    {
                        floor = -1;
                    }
                });
            });
            group(ui, "Turn", |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(any, egui::Button::new(ph::ARROW_COUNTER_CLOCKWISE))
                        .on_hover_text("Quarter turn anticlockwise (Shift + the turn key)")
                        .clicked()
                    {
                        turn = -1;
                    }
                    if ui
                        .add_enabled(any, egui::Button::new(icons::ROTATE))
                        .on_hover_text(format!("Quarter turn clockwise ({rotate_key})"))
                        .clicked()
                    {
                        turn = 1;
                    }
                });
            });
            group(ui, "Edit", |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            any,
                            egui::Button::new(format!("{}  Copy", icons::DUPLICATE)),
                        )
                        .on_hover_text(format!(
                            "Copy one tile east ({duplicate_key}); then drag the copy"
                        ))
                        .clicked()
                    {
                        copy = true;
                    }
                    let danger = ui.visuals().error_fg_color;
                    if ui
                        .add_enabled(
                            any,
                            egui::Button::new(
                                egui::RichText::new(format!("{}  Delete", icons::DELETE))
                                    .color(danger),
                            ),
                        )
                        .on_hover_text(format!("Delete ({delete_key})"))
                        .clicked()
                    {
                        delete = true;
                    }
                });
            });
        });
        ui.add_space(2.0);
        ui.weak(
            "Objects only move across the ground (the green square on the gizmo) and between \
             floors; the map has no height setting for them. Ctrl+Z undoes any of these.",
        );
        if let Some(delta) = step {
            let specs = self.selected_specs();
            self.move_objects(specs, delta);
        }
        if floor != 0 {
            self.shift_floor(floor);
        }
        if turn != 0 {
            self.rotate_selected_objects(turn);
        }
        if copy {
            self.duplicate_selected_objects();
        }
        if delete {
            self.delete_selected_objects();
        }
    }
}

/// A captioned cluster of drawer controls.
fn group(ui: &mut egui::Ui, caption: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.vertical(|ui| {
        ui.label(
            egui::RichText::new(caption.to_uppercase())
                .size(10.5)
                .strong()
                .color(ui.visuals().weak_text_color()),
        );
        add(ui);
    });
    ui.add_space(10.0);
}
