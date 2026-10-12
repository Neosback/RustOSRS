//! Editing objects: move, turn, copy and delete the selected placements.
//!
//! Like terrain, object edits live in the world worker's edit layer (`osrs_world::LocEdits`): an edit is
//! a list of placements removed and added, one undo step covers a whole action, and the regions the
//! edits sit in are rebuilt. An object is identified by what the map file says about it (object id,
//! tile, plane, type, orientation), not by its slot in the loaded region, so the identity survives a
//! rebuild; after an action the moved or new objects are re-selected once their region arrives.

use super::EditorApp;
use super::editing::EditStep;
use crate::icons;
use crate::paint::Tool;
use eframe::egui::{self, Color32, Pos2, Rect, Stroke};
use osrs_world::{LocEdit, LocSpec};
use web_time::Instant;

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

    /// Apply an object edit as one undo step and select the placements it made.
    fn edit_objects(&mut self, removed: Vec<LocSpec>, added: Vec<LocSpec>, label: &str) {
        if removed.is_empty() && added.is_empty() {
            return;
        }
        let mut edits: Vec<LocEdit> = removed.iter().map(|s| LocEdit::Remove(*s)).collect();
        edits.extend(added.iter().map(|s| LocEdit::Add(*s)));
        self.send_loc_edits(edits);
        self.push_step(EditStep {
            removed,
            added: added.clone(),
            ..EditStep::default()
        });
        self.clear_selection();
        self.pending_reselect = added;
        self.notice = Some((label.to_owned(), Instant::now()));
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

    /// Move the selected objects by `delta` tiles.
    pub(super) fn move_objects(&mut self, specs: Vec<LocSpec>, delta: (i32, i32)) {
        if delta == (0, 0) {
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
        let (fill, edge) = (
            colors.get(crate::style::ColorKey::HoverTileFill),
            colors.get(crate::style::ColorKey::HoverTileOutline),
        );
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

    /// The Objects tool's drawer: what it can do to the selected objects.
    pub(super) fn object_drawer(&mut self, ui: &mut egui::Ui) {
        let count = self
            .selection_objects
            .len()
            .max(usize::from(self.selected.is_some()));
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Objects").strong());
            ui.weak(match count {
                0 => "Click an object (Shift adds, drag a box for several)".to_owned(),
                1 => "1 selected: drag it to move it".to_owned(),
                n => format!("{n} selected: drag one to move them all"),
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                self.drawer_collapse(ui);
            });
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
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
            if ui
                .add_enabled(any, egui::Button::new(format!("{}  Turn", icons::ROTATE)))
                .on_hover_text(format!(
                    "Quarter turn ({rotate_key}); Shift turns the other way"
                ))
                .clicked()
            {
                self.rotate_selected_objects(1);
            }
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
                self.duplicate_selected_objects();
            }
            if ui
                .add_enabled(
                    any,
                    egui::Button::new(
                        egui::RichText::new(format!("{}  Delete", icons::DELETE)).color(if any {
                            Color32::from_rgb(235, 110, 100)
                        } else {
                            Color32::GRAY
                        }),
                    ),
                )
                .on_hover_text(format!("Delete ({delete_key})"))
                .clicked()
            {
                self.delete_selected_objects();
            }
            ui.separator();
            ui.weak("Ctrl+Z undoes every one of these");
        });
    }
}
