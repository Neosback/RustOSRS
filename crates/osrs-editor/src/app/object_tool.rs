//! Editing objects: move, turn, copy and delete the selected placements.
//!
//! Completed object actions commit through the authoritative semantic editor session. The existing
//! world-worker `LocEdit` layer remains a downstream projection that rebuilds affected regions and
//! re-selects moved/new placements after the rebuild arrives.

mod commit;
mod place;

mod drawer;

use super::EditorApp;
use super::edit_runtime::{EditExecution, EditExecutionError};
use super::editing::EditStep;
use crate::icons;
use crate::paint::Tool;
#[cfg(not(target_arch = "wasm32"))]
pub(super) use commit::spec_from_placement;
use commit::{ObjectCommitPlan, build_object_commit_plan};
pub(super) use commit::{ObjectHistoryRoute, build_object_history_projection};
use eframe::egui::{self, Color32, Pos2, Rect, Stroke};
use egui_phosphor::regular as ph;
use osrs_world::LocSpec;
pub(super) use place::PlaceState;
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
        let mut objects = self.interaction.selection_objects.clone();
        if let Some(selected) = &self.interaction.selected
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

    /// Commit one complete object action through the application gateway, project it to the world
    /// worker, and select the placements it made. Returns whether a non-empty semantic transaction
    /// committed and finalized successfully.
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

        let route = ObjectHistoryRoute::new(removed.clone(), added.clone());
        let execution = self.session.execute(patch, |document| {
            build_object_history_projection(document, &route, true)
        });
        let projection = match execution {
            Ok(EditExecution::Committed { projection, .. }) => projection,
            Ok(EditExecution::NoChange) => {
                self.notice = Some((
                    "Object action made no semantic changes".to_owned(),
                    Instant::now(),
                ));
                return false;
            }
            Err(EditExecutionError::Semantic(error)) => {
                self.notice = Some((
                    format!("Object action was rejected by the semantic session: {error:?}"),
                    Instant::now(),
                ));
                return false;
            }
            Err(EditExecutionError::Projection(error)) => {
                self.notice = Some((
                    format!("Object action was rolled back: {error:?}"),
                    Instant::now(),
                ));
                return false;
            }
            Err(EditExecutionError::Rollback {
                projection,
                rollback,
            }) => {
                self.notice = Some((
                    format!(
                        "Object projection failed and semantic rollback failed: {projection:?} ({rollback:?})"
                    ),
                    Instant::now(),
                ));
                return false;
            }
            Err(EditExecutionError::Finalize(error)) => {
                self.notice = Some((
                    format!(
                        "Object projection succeeded but semantic finalize failed: {error:?}"
                    ),
                    Instant::now(),
                ));
                return false;
            }
        };

        self.send_loc_edits(projection.edits);
        self.push_step(EditStep::semantic_object(removed, added));
        self.clear_selection();
        self.interaction.pending_reselect = projection.reselect;
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
            self.interaction.view_plane = target;
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
        if self.interaction.pending_reselect.is_empty() {
            return;
        }
        let mut found = Vec::new();
        for spec in &self.interaction.pending_reselect {
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
        if found.len() == self.interaction.pending_reselect.len() {
            self.interaction.selected = found.last().cloned();
            self.interaction.selection_objects = found;
            self.interaction.pending_reselect.clear();
        }
    }

    /// Keys: delete, turn, duplicate (with objects selected, in Select and in the Objects tool).
    pub(super) fn object_actions(&mut self, ctx: &egui::Context) {
        if self.placer.is_armed() && self.prefs.tool == Tool::Objects && !self.keys_blocked(ctx) {
            let (rotate, reverse) = ctx.input(|i| {
                (
                    self.keymap.pressed(i, "object.rotate"),
                    i.modifiers.shift && i.key_pressed(egui::Key::R),
                )
            });
            if rotate {
                self.rotate_placement(1);
            } else if reverse {
                self.rotate_placement(-1);
            }
        }
        if self.keys_blocked(ctx)
            || self.quick_open.is_some()
            || !matches!(self.prefs.tool, Tool::Select | Tool::Objects)
            || (self.interaction.selection_objects.is_empty()
                && self.interaction.selected.is_none())
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
            self.interaction.object_drag = None;
            return false;
        }
        let primary = egui::PointerButton::Primary;
        if response.drag_started_by(primary)
            && let Some(object) = &self.interaction.hover.object
            && let Some(tile) = self.interaction.hover.tile.map(|p| p.tile)
            && self
                .interaction
                .selection_objects
                .iter()
                .any(|o| o.region == object.region && o.index == object.index)
        {
            self.interaction.object_drag = Some(ObjectDrag {
                anchor: tile,
                specs: self.selected_specs(),
                delta: (0, 0),
            });
        }
        let Some(drag) = &mut self.interaction.object_drag else {
            return false;
        };
        if let Some(tile) = self.interaction.hover.tile.map(|p| p.tile) {
            drag.delta = (tile.0 - drag.anchor.0, tile.1 - drag.anchor.1);
        }
        if response.drag_stopped_by(primary) || !ctx.input(|i| i.pointer.primary_down()) {
            let drag = self.interaction.object_drag.take();
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
        let Some(drag) = &self.interaction.object_drag else {
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
            let Some(corners) =
                crate::interact::tile_corners(&self.infos, target, self.interaction.view_plane)
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
}

#[cfg(test)]
mod gateway_tests;
