//! Selecting objects and tiles: click, shift to add, Cmd to toggle, drag a box.
//!
//! What a click or box selects follows the select mode (objects, tiles, or both), set from the floating
//! toolbar. Tiles are selected on the viewed plane; objects are those placed on it. The selection is
//! what other tools act on ("apply to the selected tiles").

use super::EditorApp;
use crate::interact;
use crate::paint::SelectMode;
use crate::pick::{self, ObjectPick};
use crate::style::ColorKey;
use eframe::egui::{self, Pos2, Rect, Stroke};
use std::collections::HashSet;

/// A box being dragged out with the left button.
#[derive(Debug, Clone, Copy)]
pub struct Marquee {
    pub start: Pos2,
    pub current: Pos2,
    pub additive: bool,
    pub toggle: bool,
}

impl Marquee {
    pub fn rect(&self) -> Rect {
        Rect::from_two_pos(self.start, self.current)
    }
}

/// Most tiles drawn as selected at once (a box over a whole region is thousands).
const MAX_DRAWN_TILES: usize = 6000;
/// A drag shorter than this is a click.
const MIN_BOX: f32 = 4.0;

fn same(a: &ObjectPick, b: &ObjectPick) -> bool {
    a.region == b.region && a.index == b.index
}

impl EditorApp {
    pub(super) fn clear_selection(&mut self) {
        self.selected = None;
        self.selection_objects.clear();
        self.selection_tiles.clear();
    }

    /// Select exactly one object (the context menu's Select, focus, the smoke hook).
    pub(super) fn select_only(&mut self, object: ObjectPick) {
        self.selection_objects = vec![object.clone()];
        self.selected = Some(object);
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
        self.selected = found.last().cloned();
        self.selection_objects = found;
        self.notice = Some((
            format!(
                "Selected {count} object{} (id {id})",
                if count == 1 { "" } else { "s" }
            ),
            web_time::Instant::now(),
        ));
    }

    fn toggle_object(&mut self, object: ObjectPick) {
        if let Some(index) = self.selection_objects.iter().position(|o| same(o, &object)) {
            self.selection_objects.remove(index);
            self.selected = self.selection_objects.last().cloned();
        } else {
            self.selection_objects.push(object.clone());
            self.selected = Some(object);
        }
    }

    fn add_object(&mut self, object: ObjectPick) {
        if !self.selection_objects.iter().any(|o| same(o, &object)) {
            self.selection_objects.push(object.clone());
        }
        self.selected = Some(object);
    }

    fn toggle_tile(&mut self, tile: (i32, i32)) {
        if !self.selection_tiles.remove(&tile) {
            self.selection_tiles.insert(tile);
        }
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
            || self.gizmo_busy
        {
            self.marquee = None;
            return;
        }
        // Dragging a selected object moves it (Objects tool); that drag is not a box.
        if self.object_drag_input(ctx, response) {
            self.marquee = None;
            return;
        }
        let modifiers = ctx.input(|i| i.modifiers);
        let (additive, toggle) = (modifiers.shift, modifiers.command);
        let primary = egui::PointerButton::Primary;
        if response.drag_started_by(primary)
            && !modifiers.alt
            && let Some(position) = ctx.input(|i| i.pointer.interact_pos())
        {
            self.marquee = Some(Marquee {
                start: position,
                current: position,
                additive,
                toggle,
            });
        }
        if let Some(marquee) = &mut self.marquee {
            if let Some(position) = ctx.input(|i| i.pointer.interact_pos()) {
                marquee.current = position.clamp(rect.min, rect.max);
            }
            marquee.additive |= additive;
            marquee.toggle |= toggle;
        }
        if response.drag_stopped_by(primary)
            && let Some(marquee) = self.marquee.take()
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
        let object = self.hover.object.clone();
        let tile = self.hover.tile.map(|pick| pick.tile);
        if !additive && !toggle {
            if mode.objects() {
                self.selection_objects.clear();
                self.selected = None;
            }
            if mode.tiles() {
                self.selection_tiles.clear();
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
            && (mode == SelectMode::Tiles || self.selected.is_none() || additive || toggle)
        {
            if toggle {
                self.toggle_tile(tile);
            } else {
                self.selection_tiles.insert(tile);
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
        let plane = self.view_plane;
        let to_screen = |app: &Self, point: [f64; 3]| -> Option<Pos2> {
            pick::project(
                &app.camera,
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
                self.selection_objects.clear();
                self.selected = None;
            }
            if mode.tiles() {
                self.selection_tiles.clear();
            }
        }
        for tile in tiles {
            if marquee.toggle {
                self.toggle_tile(tile);
            } else {
                self.selection_tiles.insert(tile);
            }
        }
        for object in objects {
            if marquee.toggle {
                self.toggle_object(object);
            } else {
                self.add_object(object);
            }
        }
        let (objects, tiles) = (self.selection_objects.len(), self.selection_tiles.len());
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
            &self.camera,
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
        let plane = self.view_plane;
        let quad = |corners: [[f64; 3]; 4]| -> Option<Vec<Pos2>> {
            let points: Vec<Pos2> = corners
                .iter()
                .filter_map(|c| self.screen_point(*c, viewport, pixels_per_point, width, height))
                .collect();
            (points.len() == 4).then_some(points)
        };
        for tile in self.selection_tiles.iter().take(MAX_DRAWN_TILES) {
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
        for object in &self.selection_objects {
            if self.selected.as_ref().is_some_and(|s| same(s, object)) {
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
        if let Some(marquee) = &self.marquee {
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
