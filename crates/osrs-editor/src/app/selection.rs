//! Selecting objects and tiles: click, shift to add, Cmd to toggle, drag a box.
//!
//! What a click or box selects follows the select mode (objects, tiles, or both), set from the floating
//! toolbar. Tiles are selected on the viewed plane; objects are those placed on it. The selection is
//! what other tools act on ("apply to the selected tiles").

use super::{EditorApp, interaction_runtime::Marquee};
use crate::interact;
use crate::paint::SelectMode;
use crate::pick::{self, ObjectPick};
use crate::style::ColorKey;
use eframe::egui::{self, Pos2, Rect, Stroke};
use std::collections::HashSet;

/// Most tiles drawn as selected at once (a box over a whole region is thousands).
const MAX_DRAWN_TILES: usize = 6000;
/// A drag shorter than this is a click.
const MIN_BOX: f32 = 4.0;

impl EditorApp {
    pub(super) fn clear_selection(&mut self) {
        self.interaction.clear_selection();
    }

    /// Select exactly one object (the context menu's Select, focus, the smoke hook).
    pub(super) fn select_only(&mut self, object: ObjectPick) {
        let inspect_tile = self.object_tile(&object);
        self.interaction.select_only_object(object, inspect_tile);
    }

    /// Select every loaded object with definition `id`.
    pub(super) fn select_similar(&mut self, id: u32) {
        let mut found = Vec::new();
        for (region, info) in &self.infos {
            for (index, loc) in info.locs.iter().enumerate() {
                if loc.definition.identity.id.get() == id {
                    found.push(ObjectPick {
                        region: *region,
                        index,
                    });
                }
            }
        }
        let count = found.len();
        self.interaction.replace_objects(found);
        self.notice = Some((
            format!(
                "Selected {count} object{} (id {id})",
                if count == 1 { "" } else { "s" }
            ),
            web_time::Instant::now(),
        ));
    }

    fn toggle_object(&mut self, object: ObjectPick) {
        let inspect_tile = self.object_tile(&object);
        self.interaction.toggle_object(object, inspect_tile);
    }

    fn add_object(&mut self, object: ObjectPick) {
        let inspect_tile = self.object_tile(&object);
        self.interaction.add_object(object, inspect_tile);
    }

    /// The Inspector's tile for an object selection (its anchor).
    fn object_tile(&self, object: &ObjectPick) -> Option<(i32, i32)> {
        self.infos
            .get(&object.region)
            .and_then(|info| info.locs.get(object.index))
            .map(|loc| loc.tile)
    }

    /// Click and box selection with the Select tool (called once per frame after picking).
    pub(super) fn selection_input(
        &mut self,
        ctx: &egui::Context,
        response: &egui::Response,
        rect: Rect,
        pixels_per_point: f32,
        width: u32,
        height: u32,
    ) {
        let objects_tool = self.prefs.tool == crate::paint::Tool::Objects;
        if !(self.prefs.tool == crate::paint::Tool::Select || objects_tool)
            || self.quick_open.is_some()
            || self.context_menu.is_some()
            || self.interaction.gizmo_busy
        {
            self.interaction.cancel_marquee();
            return;
        }
        // An armed object is placed by clicking; nothing else is selected or dragged meanwhile.
        if objects_tool && self.placer.is_armed() {
            self.interaction.cancel_marquee();
            if response.clicked() {
                self.place_armed_object();
            }
            return;
        }
        // Dragging a selected object moves it (Objects tool); that drag is not a box.
        if self.object_drag_input(ctx, response) {
            self.interaction.cancel_marquee();
            return;
        }
        let modifiers = ctx.input(|i| i.modifiers);
        let (additive, toggle) = (modifiers.shift, modifiers.command);
        let primary = egui::PointerButton::Primary;
        if response.drag_started_by(primary)
            && !modifiers.alt
            && let Some(position) = ctx.input(|i| i.pointer.interact_pos())
        {
            self.interaction.begin_marquee(position, additive, toggle);
        }
        self.interaction.update_marquee(
            ctx.input(|i| i.pointer.interact_pos()),
            rect,
            additive,
            toggle,
        );
        if response.drag_stopped_by(primary)
            && let Some(marquee) = self.interaction.finish_marquee()
        {
            let area = marquee.rect();
            if area.width() >= MIN_BOX || area.height() >= MIN_BOX {
                self.marquee_select(area, marquee, rect, pixels_per_point, width, height);
            }
            return;
        }
        if response.clicked() {
            self.click_select(additive, toggle);
        }
    }

    fn click_select(&mut self, additive: bool, toggle: bool) {
        let mode = if self.prefs.tool == crate::paint::Tool::Objects {
            SelectMode::Objects
        } else {
            self.prefs.select_mode
        };
        let object = self.interaction.hover.object.clone();
        let tile = self.interaction.hover.tile.map(|pick| pick.tile);
        if !additive && !toggle {
            if mode.objects() {
                self.interaction.clear_objects();
            }
            if mode.tiles() {
                self.interaction.clear_tiles();
            }
        }
        if mode.objects()
            && let Some(object) = object
        {
            if toggle {
                self.toggle_object(object);
            } else {
                self.add_object(object);
            }
        }
        if mode.tiles()
            && let Some(tile) = tile
            // In Both mode an object under the cursor wins the click; the tile follows with Shift.
            && (mode == SelectMode::Tiles || self.interaction.selected.is_none() || additive || toggle)
        {
            if toggle {
                self.interaction.toggle_tile(tile);
            } else {
                self.interaction.select_tile(tile);
            }
        }
    }

    fn marquee_select(
        &mut self,
        area: Rect,
        marquee: Marquee,
        viewport: Rect,
        pixels_per_point: f32,
        width: u32,
        height: u32,
    ) {
        let mode = if self.prefs.tool == crate::paint::Tool::Objects {
            SelectMode::Objects
        } else {
            self.prefs.select_mode
        };
        let plane = self.interaction.view_plane;
        let to_screen = |app: &Self, point: [f64; 3]| -> Option<Pos2> {
            pick::project(
                &app.interaction.camera,
                app.renderer.render_origin(),
                width as f32,
                height as f32,
                point,
            )
            .map(|p| {
                Pos2::new(
                    viewport.min.x + p[0] / pixels_per_point,
                    viewport.min.y + p[1] / pixels_per_point,
                )
            })
        };
        let mut tiles: HashSet<(i32, i32)> = HashSet::new();
        let mut objects: Vec<ObjectPick> = Vec::new();
        for (region, info) in &self.infos {
            if mode.tiles() {
                for lx in 0..64 {
                    for ly in 0..64 {
                        let tile = (region.0 * 64 + lx, region.1 * 64 + ly);
                        let Some(corners) = interact::tile_corners(&self.infos, tile, plane) else {
                            continue;
                        };
                        let centre = [
                            f64::from(tile.0) * 128.0 + 64.0,
                            corners.iter().map(|c| c[1]).sum::<f64>() / 4.0,
                            f64::from(tile.1) * 128.0 + 64.0,
                        ];
                        if let Some(position) = to_screen(self, centre)
                            && area.contains(position)
                        {
                            tiles.insert(tile);
                        }
                    }
                }
            }
            if mode.objects() {
                for (index, loc) in info.locs.iter().enumerate() {
                    if loc.plane != plane {
                        continue;
                    }
                    let Some(corners) = interact::tile_corners(&self.infos, loc.tile, plane) else {
                        continue;
                    };
                    let centre = [
                        f64::from(loc.tile.0) * 128.0 + 64.0,
                        corners.iter().map(|c| c[1]).sum::<f64>() / 4.0,
                        f64::from(loc.tile.1) * 128.0 + 64.0,
                    ];
                    if let Some(position) = to_screen(self, centre)
                        && area.contains(position)
                    {
                        objects.push(ObjectPick {
                            region: *region,
                            index,
                        });
                    }
                }
            }
        }
        if !marquee.additive && !marquee.toggle {
            if mode.objects() {
                self.interaction.clear_objects();
            }
            if mode.tiles() {
                self.interaction.clear_tiles();
            }
        }
        for tile in tiles {
            if marquee.toggle {
                self.interaction.toggle_tile(tile);
            } else {
                self.interaction.add_tile(tile);
            }
        }
        for object in objects {
            if marquee.toggle {
                self.toggle_object(object);
            } else {
                self.add_object(object);
            }
        }
        let (objects, tiles) = self.interaction.selection_counts();
        self.notice = Some((
            format!(
                "Selected {objects} object{} and {tiles} tile{}",
                if objects == 1 { "" } else { "s" },
                if tiles == 1 { "" } else { "s" }
            ),
            web_time::Instant::now(),
        ));
    }

    /// Project a world point to a screen position inside the viewport.
    pub(super) fn screen_point(
        &self,
        point: [f64; 3],
        viewport: Rect,
        pixels_per_point: f32,
        width: u32,
        height: u32,
    ) -> Option<Pos2> {
        pick::project(
            &self.interaction.camera,
            self.renderer.render_origin(),
            width as f32,
            height as f32,
            point,
        )
        .map(|p| {
            Pos2::new(
                viewport.min.x + p[0] / pixels_per_point,
                viewport.min.y + p[1] / pixels_per_point,
            )
        })
    }

    /// Selected tiles, extra selected objects (their footprints) and the box being dragged.
    pub(super) fn draw_selection_extras(
        &self,
        painter: &egui::Painter,
        viewport: Rect,
        pixels_per_point: f32,
        width: u32,
        height: u32,
    ) {
        let plane = self.interaction.view_plane;
        let quad = |corners: [[f64; 3]; 4]| -> Option<Vec<Pos2>> {
            let points: Vec<Pos2> = corners
                .iter()
                .filter_map(|c| self.screen_point(*c, viewport, pixels_per_point, width, height))
                .collect();
            (points.len() == 4).then_some(points)
        };
        for tile in self
            .interaction
            .selection_tiles
            .iter()
            .take(MAX_DRAWN_TILES)
        {
            if let Some(corners) = interact::tile_corners(&self.infos, *tile, plane)
                && let Some(points) = quad(corners)
            {
                painter.add(egui::Shape::convex_polygon(
                    points,
                    self.prefs.colors.get(ColorKey::TileSelectionFill),
                    Stroke::new(1.0, self.prefs.colors.get(ColorKey::TileSelectionOutline)),
                ));
            }
        }
        for object in &self.interaction.selection_objects {
            if self.interaction.is_primary_object(object) {
                continue;
            }
            let Some(loc) = self
                .infos
                .get(&object.region)
                .and_then(|info| info.locs.get(object.index))
            else {
                continue;
            };
            let (mut size_x, mut size_y) = (
                f64::from(loc.definition.size_x),
                f64::from(loc.definition.size_y),
            );
            if loc.orientation & 1 == 1 {
                std::mem::swap(&mut size_x, &mut size_y);
            }
            let Some(base) = interact::tile_corners(&self.infos, loc.tile, plane) else {
                continue;
            };
            let y = base[0][1];
            let (x0, z0) = (f64::from(loc.tile.0) * 128.0, f64::from(loc.tile.1) * 128.0);
            let (x1, z1) = (x0 + size_x * 128.0, z0 + size_y * 128.0);
            if let Some(points) = quad([[x0, y, z0], [x1, y, z0], [x1, y, z1], [x0, y, z1]]) {
                painter.add(egui::Shape::convex_polygon(
                    points,
                    self.prefs.colors.get(ColorKey::SelectionFill),
                    Stroke::new(1.5, self.prefs.colors.get(ColorKey::SelectionOutline)),
                ));
            }
        }
        if let Some(marquee) = self.interaction.marquee() {
            let accent = self.prefs.colors.get(ColorKey::TileSelectionOutline);
            painter.rect(
                marquee.rect(),
                2.0,
                accent.gamma_multiply(0.16),
                Stroke::new(1.0, accent),
                egui::StrokeKind::Inside,
            );
        }
    }
}
