//! Selecting objects and tiles: click, shift to add, Cmd to toggle, drag a box.
//!
//! What a click or box selects follows the select mode (objects, tiles, or both), set from the floating
//! toolbar. Tiles are selected on the viewed plane; objects are those placed on it. The selection is
//! what other tools act on ("apply to the selected tiles").

mod boundary;

use super::{EditorApp, interaction_runtime::Marquee};
use crate::interact;
use crate::paint::SelectMode;
use crate::pick::{self, ObjectPick};
use crate::style::ColorKey;
use eframe::egui::{self, Pos2, Rect, Stroke};
use std::collections::HashSet;

/// Most tiles drawn as selected at once (a box over a whole region is thousands).
/// Most selected objects that get their own footprint drawn.
const MAX_FOOTPRINTS: usize = 40;
/// Most selected tiles drawn with a fill; a larger selection is outlined only.
const MAX_DRAWN_TILES: usize = 20_000;
/// Longest side of a box selection, in tiles.
const MAX_BOX_SIDE: i32 = 160;
/// A drag shorter than this is a click.
const MIN_BOX: f32 = 4.0;

/// The ground tile under a screen position (what a click there would land on), or `None` off the
/// loaded map.
fn ground_tile_at(
    app: &EditorApp,
    position: Pos2,
    viewport: Rect,
    pixels_per_point: f32,
    width: u32,
    height: u32,
) -> Option<(i32, i32)> {
    let ray = pick::ray_through_pixel(
        app.interaction.camera(),
        app.renderer.render_origin(),
        width as f32,
        height as f32,
        (position.x - viewport.min.x) * pixels_per_point,
        (position.y - viewport.min.y) * pixels_per_point,
    );
    pick::pick(
        app.world.infos(),
        &ray,
        app.interaction.view_plane(),
        app.renderer.roof_view(),
    )
    .tile
    .map(|tile| tile.tile)
}

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
        for (region, info) in self.world.infos() {
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
        self.ui.notify(format!(
            "Selected {count} object{} (id {id})",
            if count == 1 { "" } else { "s" }
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
        self.world
            .infos()
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
        if super::prefab_panel::capture::refine::input(self, response) {
            return;
        }
        let objects_tool = self.settings.prefs.tool == crate::paint::Tool::Objects;
        if !(self.settings.prefs.tool == crate::paint::Tool::Select || objects_tool)
            || self.ui.quick_open.is_some()
            || self.ui.context_menu.is_some()
            || self.interaction.gizmo_busy()
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
        if self.interaction.marquee().is_some() {
            let press = ctx.input(|i| i.pointer.press_origin());
            let start =
                press.and_then(|p| ground_tile_at(self, p, rect, pixels_per_point, width, height));
            let now = ctx
                .input(|i| i.pointer.interact_pos())
                .and_then(|p| ground_tile_at(self, p, rect, pixels_per_point, width, height));
            self.interaction.track_marquee_tiles(start, now);
        }
        if response.drag_stopped_by(primary)
            && let Some(marquee) = self.interaction.finish_marquee()
        {
            let area = marquee.rect();
            if (area.width() >= MIN_BOX || area.height() >= MIN_BOX)
                && let Some(tiles) = marquee.tile_box()
            {
                self.marquee_select(tiles, marquee);
            }
            return;
        }
        if response.clicked() {
            self.click_select(additive, toggle);
        }
    }

    fn click_select(&mut self, additive: bool, toggle: bool) {
        self.interaction.drop_pending_reselect();
        let mode = if self.settings.prefs.tool == crate::paint::Tool::Objects {
            SelectMode::Objects
        } else {
            self.settings.prefs.select_mode
        };
        let object = self.interaction.hover().object.clone();
        let tile = self.interaction.hover().tile.map(|pick| pick.tile);
        if self.settings.prefs.select_mode == SelectMode::Building
            && self.settings.prefs.tool == crate::paint::Tool::Select
        {
            let message = tile
                .and_then(|tile| super::building_pick::select_building(self, tile, additive))
                .unwrap_or_else(|| "Click inside a building to select all of it".to_owned());
            self.ui.notify(message);
            return;
        }
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
            && (mode == SelectMode::Tiles || self.interaction.selected().is_none() || additive || toggle)
        {
            if toggle {
                self.interaction.toggle_tile(tile);
            } else {
                self.interaction.select_tile(tile);
            }
        }
    }

    /// Select what lies in the rectangle of tiles `((x0, y0), (x1, y1))`: tiles and objects by
    /// the tile they stand on, in the map's own axes.
    fn marquee_select(&mut self, area: ((i32, i32), (i32, i32)), marquee: Marquee) {
        self.interaction.drop_pending_reselect();
        let mode = if self.settings.prefs.tool == crate::paint::Tool::Objects {
            SelectMode::Objects
        } else {
            self.settings.prefs.select_mode
        };
        let plane = self.interaction.view_plane();
        let ((x0, y0), (x1, y1)) = area;
        let (x1, y1) = (x1.min(x0 + MAX_BOX_SIDE), y1.min(y0 + MAX_BOX_SIDE));
        let inside = |tile: (i32, i32)| (x0..=x1).contains(&tile.0) && (y0..=y1).contains(&tile.1);
        let mut tiles: HashSet<(i32, i32)> = HashSet::new();
        let mut objects: Vec<ObjectPick> = Vec::new();
        if mode.tiles() {
            for x in x0..=x1 {
                for y in y0..=y1 {
                    if interact::tile_corners(self.world.infos(), (x, y), plane).is_some() {
                        tiles.insert((x, y));
                    }
                }
            }
        }
        if mode.objects() {
            for (region, info) in self.world.infos() {
                for (index, loc) in info.locs.iter().enumerate() {
                    if loc.plane == plane && inside(loc.tile) {
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
        self.ui.notify(format!(
            "Selected {objects} object{} and {tiles} tile{}",
            if objects == 1 { "" } else { "s" },
            if tiles == 1 { "" } else { "s" }
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
            self.interaction.camera(),
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
        let plane = self.interaction.view_plane();
        let quad = |corners: [[f64; 3]; 4]| -> Option<Vec<Pos2>> {
            let points: Vec<Pos2> = corners
                .iter()
                .filter_map(|c| self.screen_point(*c, viewport, pixels_per_point, width, height))
                .collect();
            (points.len() == 4).then_some(points)
        };
        // The selected tiles as one shape: a single fill with no lines between tiles, and an
        // outline only along the edges where a selected tile meets one that is not.
        let selected = self.interaction.selection_tiles();
        let filled = selected.len() <= MAX_DRAWN_TILES;
        if !selected.is_empty() {
            let fill = self.settings.prefs.colors.get(ColorKey::TileSelectionFill);
            let outline = self
                .settings
                .prefs
                .colors
                .get(ColorKey::TileSelectionOutline);
            let mut mesh = egui::epaint::Mesh::default();
            for &tile in selected {
                let Some(corners) = interact::tile_corners(self.world.infos(), tile, plane) else {
                    continue;
                };
                let Some(points) = quad(corners) else {
                    continue;
                };
                if filled {
                    let base = u32::try_from(mesh.vertices.len()).unwrap_or(0);
                    for point in &points {
                        mesh.colored_vertex(*point, fill);
                    }
                    mesh.add_triangle(base, base + 1, base + 2);
                    mesh.add_triangle(base, base + 2, base + 3);
                }
            }
            painter.add(egui::Shape::mesh(mesh));
            let stroke = Stroke::new(2.5, outline);
            for run in boundary::runs(selected) {
                let mut points = Vec::new();
                for (x, y) in run {
                    let point = [
                        ((x, y), 0),
                        ((x - 1, y), 1),
                        ((x - 1, y - 1), 2),
                        ((x, y - 1), 3),
                    ]
                    .into_iter()
                    .find_map(|(tile, corner)| {
                        interact::tile_corners(self.world.infos(), tile, plane).and_then(|c| {
                            self.screen_point(c[corner], viewport, pixels_per_point, width, height)
                        })
                    });
                    if let Some(point) = point {
                        points.push(point);
                    } else if points.len() >= 2 {
                        painter.add(egui::Shape::line(std::mem::take(&mut points), stroke));
                    } else {
                        points.clear();
                    }
                }
                if points.len() >= 2 {
                    painter.add(egui::Shape::line(points, stroke));
                }
            }
            if !filled {
                painter.text(
                    viewport.left_top() + egui::vec2(12.0, 12.0),
                    egui::Align2::LEFT_TOP,
                    format!("{} tiles (outline only)", selected.len()),
                    egui::FontId::proportional(13.0),
                    outline,
                );
            }
        }
        // A whole building is hundreds of objects: their footprints would bury the tile shape.
        let few_objects = self.interaction.selection_objects().len() <= MAX_FOOTPRINTS;
        for object in self
            .interaction
            .selection_objects()
            .iter()
            .filter(|_| few_objects)
        {
            if self.interaction.is_primary_object(object) {
                continue;
            }
            let Some(loc) = self
                .world
                .infos()
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
            let Some(base) = interact::tile_corners(self.world.infos(), loc.tile, plane) else {
                continue;
            };
            let y = base[0][1];
            let (x0, z0) = (f64::from(loc.tile.0) * 128.0, f64::from(loc.tile.1) * 128.0);
            let (x1, z1) = (x0 + size_x * 128.0, z0 + size_y * 128.0);
            if let Some(points) = quad([[x0, y, z0], [x1, y, z0], [x1, y, z1], [x0, y, z1]]) {
                painter.add(egui::Shape::convex_polygon(
                    points,
                    self.settings.prefs.colors.get(ColorKey::SelectionFill),
                    Stroke::new(
                        1.5,
                        self.settings.prefs.colors.get(ColorKey::SelectionOutline),
                    ),
                ));
            }
        }
        if let Some(marquee) = self.interaction.marquee()
            && let Some(((x0, y0), (x1, y1))) = marquee.tile_box()
        {
            let accent = self
                .settings
                .prefs
                .colors
                .get(ColorKey::TileSelectionOutline);
            let plane = self.interaction.view_plane();
            let project = |tile: (i32, i32), corner: usize| {
                interact::tile_corners(self.world.infos(), tile, plane)
                    .map(|c| c[corner])
                    .and_then(|p| self.screen_point(p, viewport, pixels_per_point, width, height))
            };
            // Walk the box's edge tile by tile so the outline follows the ground.
            let mut ring: Vec<Pos2> = Vec::new();
            ring.extend((x0..=x1).filter_map(|x| project((x, y0), 0)));
            ring.extend(project((x1, y0), 1));
            ring.extend((y0..=y1).filter_map(|y| project((x1, y), 1)));
            ring.extend(project((x1, y1), 2));
            ring.extend((x0..=x1).rev().filter_map(|x| project((x, y1), 3)));
            ring.extend((y0..=y1).rev().filter_map(|y| project((x0, y), 3)));
            // A light fill on the four corners, and the outline along the ground.
            let corners: Vec<Pos2> = [
                project((x0, y0), 0),
                project((x1, y0), 1),
                project((x1, y1), 2),
                project((x0, y1), 3),
            ]
            .into_iter()
            .flatten()
            .collect();
            if corners.len() == 4 {
                painter.add(egui::Shape::convex_polygon(
                    corners,
                    accent.gamma_multiply(0.12),
                    Stroke::NONE,
                ));
            }
            if ring.len() >= 3 {
                painter.add(egui::Shape::closed_line(ring, Stroke::new(2.0, accent)));
            }
        }
    }
}
