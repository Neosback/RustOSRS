//! Placing a prefab in the viewport: the armed gesture and its ghost.

use crate::app::{EditorApp, prefab_runtime::Armed};
use eframe::egui::{self, Color32, Pos2, Stroke};
use osrs_core::{
    coords::{MapTile, SourcePlane},
    ids::ObjectId,
};
use osrs_edit::{
    LocKind, MapOp, PlaceSpec, PlacedPrefab, PrefabError, plan_place_prefab, turned_size,
};

/// Most object markers the ghost draws.
const MAX_MARKERS: usize = 800;

/// Poll the library's file dialogs (called every frame, panel open or not).
pub(super) fn poll(app: &mut EditorApp) {
    app.prefabs.poll_files();
}

/// The south-west tile of the placed rectangle when the cursor is on `hover`: the prefab is
/// centred on the cursor.
fn origin_for(hover: (i32, i32), width: u16, height: u16) -> MapTile {
    MapTile::new(
        hover.0 - i32::from(width) / 2,
        hover.1 - i32::from(height) / 2,
    )
}

/// What placing the armed prefab with its centre on `hover` would do.
pub(super) fn plan(
    app: &EditorApp,
    armed: &Armed,
    hover: (i32, i32),
) -> Result<(PlacedPrefab, MapTile), PrefabError> {
    let entry = app.prefabs.entry(&armed.file).ok_or(PrefabError::Nothing)?;
    let base_plane =
        SourcePlane::new(app.interaction.view_plane()).ok_or(PrefabError::BadSize {
            width: 0,
            height: 0,
            planes: 0,
        })?;
    let (width, height) = turned_size(entry.prefab.width, entry.prefab.height, armed.turns);
    let origin = origin_for(hover, width, height);
    let spec = PlaceSpec {
        origin,
        base_plane,
        quarter_turns: armed.turns,
        terrain: armed.terrain,
        heights: armed.heights,
        objects: armed.objects,
        replace_objects: armed.replace_objects,
    };
    let size_of = |id: ObjectId, _: LocKind| app.placer.object_size(id.get());
    plan_place_prefab(app.session.document(), &entry.prefab, spec, &size_of)
        .map(|placed| (placed, origin))
}

/// While a prefab is armed it owns the viewport click. Returns true when the click was consumed or
/// a placement is pending, so the active tool ignores this frame's pointer.
pub(in crate::app) fn armed_input(app: &mut EditorApp, response: &egui::Response) -> bool {
    if app.prefabs.build.choosing {
        return choose_site(app, response);
    }
    if app.prefabs.armed.is_none() {
        return false;
    }
    let ctx = response.ctx.clone();
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.prefabs.disarm();
        return true;
    }
    if !app.keys_blocked(&ctx)
        && let Some(armed) = app.prefabs.armed.as_mut()
        && ctx.input(|i| i.key_pressed(egui::Key::R))
    {
        let back = ctx.input(|i| i.modifiers.shift);
        armed.turns = (armed.turns + if back { 3 } else { 1 }) & 3;
    }
    if !response.clicked() {
        return true;
    }
    let Some(hover) = app.interaction.hover().tile.map(|pick| pick.tile) else {
        return true;
    };
    let Some(armed) = app.prefabs.armed.clone() else {
        return true;
    };
    place(app, &armed, hover);
    true
}

/// The next click puts the generator's site under the cursor; Esc stops.
fn choose_site(app: &mut EditorApp, response: &egui::Response) -> bool {
    if response.ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.prefabs.build.choosing = false;
        return true;
    }
    if response.clicked()
        && let Some(tile) = app.interaction.hover().tile.map(|pick| pick.tile)
    {
        app.prefabs.build.centre_site_on(tile);
    }
    true
}

/// Plan and commit the armed prefab with its centre on `hover`; the outcome is shown as a notice.
pub(in crate::app) fn place(app: &mut EditorApp, armed: &Armed, hover: (i32, i32)) -> bool {
    let name = app
        .prefabs
        .entry(&armed.file)
        .map_or_else(String::new, |entry| entry.prefab.name.clone());
    match plan(app, armed, hover) {
        Ok((placed, _)) => {
            let skipped = placed.skipped_heights;
            let committed =
                crate::app::editing::commit_map_patch(app, placed.patch, &format!("Placed {name}"));
            if committed && skipped > 0 {
                app.prefabs.say(format!(
                    "{skipped} height points were outside the loaded map and were left alone"
                ));
            }
            committed
        }
        Err(error) => {
            app.prefabs.say(format!("Cannot place {name}: {error}"));
            false
        }
    }
}

/// A screen-space helper shared by the ghosts: projecting world points over the viewport.
struct Ghost<'a> {
    app: &'a EditorApp,
    painter: &'a egui::Painter,
    viewport: egui::Rect,
    pixels_per_point: f32,
    width: u32,
    height: u32,
    plane: u8,
}

impl Ghost<'_> {
    fn project(&self, p: [f64; 3], lift: f64) -> Option<Pos2> {
        self.app.screen_point(
            [p[0], p[1] - lift, p[2]],
            self.viewport,
            self.pixels_per_point,
            self.width,
            self.height,
        )
    }

    fn corner(&self, tile: (i32, i32), index: usize) -> Option<[f64; 3]> {
        crate::interact::tile_corners(self.app.world.infos(), tile, self.plane).map(|c| c[index])
    }

    /// The outline of a `w` x `h` rectangle whose south-west tile is `origin`.
    fn outline(&self, origin: MapTile, w: u16, h: u16, colour: Color32) {
        let (x0, y0) = (origin.x, origin.y);
        let (x1, y1) = (x0 + i32::from(w) - 1, y0 + i32::from(h) - 1);
        let points: Vec<Pos2> = [
            self.corner((x0, y0), 0),
            self.corner((x1, y0), 1),
            self.corner((x1, y1), 2),
            self.corner((x0, y1), 3),
        ]
        .into_iter()
        .flatten()
        .filter_map(|c| self.project(c, 4.0))
        .collect();
        if points.len() == 4 {
            self.painter
                .add(egui::Shape::closed_line(points, Stroke::new(2.0, colour)));
        }
    }

    /// A dot on each object a patch would add (green) or remove (red) on the viewed plane.
    fn markers(&self, patch: &osrs_edit::MapPatch) {
        for op in patch.ops().iter().take(MAX_MARKERS * 4) {
            let (placement, tint) = match op {
                MapOp::AddObject { placement } => (placement, Color32::from_rgb(120, 230, 140)),
                MapOp::RemoveObject { placement } => (placement, Color32::from_rgb(240, 100, 90)),
                _ => continue,
            };
            if placement.position.plane.index().get() != self.plane {
                continue;
            }
            let tile = (placement.position.tile.x, placement.position.tile.y);
            let Some(c) = crate::interact::tile_corners(self.app.world.infos(), tile, self.plane)
            else {
                continue;
            };
            let centre = [
                (c[0][0] + c[2][0]) / 2.0,
                (c[0][1] + c[2][1]) / 2.0,
                (c[0][2] + c[2][2]) / 2.0,
            ];
            if let Some(at) = self.project(centre, 30.0) {
                self.painter
                    .circle_filled(at, 3.0, tint.gamma_multiply(0.85));
            }
        }
    }
}

/// The ghosts: for an armed prefab, its rectangle and a marker per object it adds or removes
/// (the outline is red when it cannot be planned); for the generator, the site and the building.
pub(in crate::app) fn draw_world(
    app: &EditorApp,
    painter: &egui::Painter,
    viewport: egui::Rect,
    pixels_per_point: f32,
    width: u32,
    height: u32,
) {
    let ghost = Ghost {
        app,
        painter,
        viewport,
        pixels_per_point,
        width,
        height,
        plane: app.interaction.view_plane(),
    };
    let hover = app.interaction.hover().tile.map(|pick| pick.tile);
    if let (Some(armed), Some(hover)) = (&app.prefabs.armed, hover) {
        match plan(app, armed, hover) {
            Ok((placed, origin)) => {
                ghost.outline(
                    origin,
                    placed.width,
                    placed.height,
                    Color32::from_rgb(120, 230, 140),
                );
                ghost.markers(&placed.patch);
            }
            Err(error) => {
                let Some(entry) = app.prefabs.entry(&armed.file) else {
                    return;
                };
                let (w, h) = turned_size(entry.prefab.width, entry.prefab.height, armed.turns);
                ghost.outline(
                    origin_for(hover, w, h),
                    w,
                    h,
                    Color32::from_rgb(240, 100, 90),
                );
                if let Some(position) = ghost.corner(hover, 0).and_then(|c| ghost.project(c, 40.0))
                {
                    painter.text(
                        position,
                        egui::Align2::LEFT_BOTTOM,
                        error.to_string(),
                        egui::FontId::proportional(12.0),
                        Color32::from_rgb(255, 160, 150),
                    );
                }
            }
        }
    }
    let build = &app.prefabs.build;
    if build.choosing
        && let Some(hover) = hover
    {
        let (w, h) = (build.form.width, build.form.height);
        ghost.outline(
            origin_for(hover, w, h),
            w,
            h,
            Color32::from_rgb(110, 190, 255),
        );
    }
    if app.settings.prefs.properties_panel
        && app.settings.prefs.rail_tab == crate::settings::RailTab::Prefabs
        && app.prefabs.page == crate::app::prefab_runtime::Page::Build
    {
        if let Some((origin, w, h)) = build.site_rect() {
            ghost.outline(origin, w, h, Color32::from_rgb(110, 190, 255));
        }
        if let Some(preview) = &build.preview {
            ghost.markers(&preview.apply);
        }
    }
}
