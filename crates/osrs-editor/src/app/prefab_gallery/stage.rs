//! The gallery's stage: a second, isolated editor over a blank corner of the map, where the
//! selected prefab is placed so it can be looked at from every side with the real renderer.
//!
//! The stage owns its own world, document, history and renderer (sharing only the GPU device), the
//! way the read-only export review does, so nothing here touches the map being edited. The prefab
//! goes onto the stage through the same planner and edit gateway as a placement on the map, and
//! the next selection undoes it.

use crate::app::{EditorApp, editing::commit_map_patch, interaction_runtime::CameraInput};
use eframe::egui;
use osrs_core::{
    coords::{MapTile, SourcePlane},
    ids::{FloorUnderlayId, ObjectId},
};
use osrs_edit::{
    LocKind, MapOp, MapPatch, MapPosition, PlaceSpec, Prefab, TerrainTile, plan_place_prefab,
    turned_size,
};

/// Centre of the stage (world tile): the middle of map square (50, 33), inside a block of nine
/// squares the cache keeps empty (no objects, no floors), so nothing else is ever in the picture.
pub(super) const CENTRE: (i32, i32) = (3232, 2144);
/// Half the side of the floor laid under the prefab, in tiles.
const PAD_HALF: i32 = 72;
/// Underlay of the stage floor: the grass most captured buildings stand on.
const GROUND_UNDERLAY: u32 = 50;

/// What is on the stage now.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Shown {
    file: String,
    turns: u8,
}

pub(super) struct Stage {
    viewer: Box<EditorApp>,
    padded: bool,
    shown: Option<Shown>,
    /// Frames since the prefab was last put on the stage (the world needs a few to build it).
    since_shown: u32,
    /// Why the last placement failed (shown under the view).
    pub(super) problem: Option<String>,
}

impl Stage {
    /// A stage sharing `parent`'s GPU device and cache. Native only: the browser build cannot
    /// read a cache folder or share the device this way.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn new(parent: &EditorApp, ctx: &egui::Context) -> Result<Self, String> {
        use crate::hud::Destination;
        use osrs_cache::source::CacheSource;
        let label = parent.project.cache_label().clone();
        let mut viewer = EditorApp::from_source(
            ctx,
            parent.renderer.render_state(),
            CacheSource::Dir(label.clone()),
            label,
            (CENTRE.0.div_euclid(64) - 1) * 64,
            (CENTRE.1.div_euclid(64) - 1) * 64,
            false,
        )?;
        viewer.settings.prefs = parent.settings.prefs.clone();
        let prefs = &mut viewer.settings.prefs;
        prefs.tool = crate::paint::Tool::Select;
        prefs.properties_panel = false;
        prefs.inspector_follow = false;
        prefs.stats_overlay = false;
        prefs.show_minimap = false;
        prefs.hover_tile = false;
        prefs.load_radius = 1;
        // A quiet backdrop: the edge of the floor is never in the way of the prefab.
        prefs.sky_color = [0.30, 0.33, 0.38];
        viewer.teleport_to(Destination::Tile {
            x: CENTRE.0,
            y: CENTRE.1,
            plane: 0,
        });
        viewer
            .interaction
            .set_camera_mode(crate::camera::CameraMode::Orbit);
        Ok(Self {
            viewer: Box::new(viewer),
            padded: false,
            shown: None,
            since_shown: 0,
            problem: None,
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) fn new(_parent: &EditorApp, _ctx: &egui::Context) -> Result<Self, String> {
        Err("The 3D gallery needs the desktop app.".to_owned())
    }

    pub(super) fn viewer(&mut self) -> &mut EditorApp {
        &mut self.viewer
    }

    pub(super) fn ready(&self) -> bool {
        self.padded && self.viewer.world.is_ready()
    }

    /// Once per frame: let the stage's world stream and animate, and lay its floor when the empty
    /// map square has loaded.
    pub(super) fn step(&mut self, ctx: &egui::Context) {
        let viewer = &mut self.viewer;
        viewer.rebase_if_needed();
        viewer.stream(ctx);
        viewer.update_animations();
        viewer.palettes.poll();
        viewer.poll_catalog();
        viewer.assets.poll(ctx);
        self.since_shown = self.since_shown.saturating_add(1);
        if !self.padded && viewer.world.is_ready() && viewer.world.resident_regions() > 0 {
            let plane = SourcePlane::new(0).unwrap_or_else(|| unreachable!("plane 0 exists"));
            let mut ops = Vec::new();
            for dx in -PAD_HALF..PAD_HALF {
                for dy in -PAD_HALF..PAD_HALF {
                    let coord = MapPosition::new(MapTile::new(CENTRE.0 + dx, CENTRE.1 + dy), plane);
                    let before = viewer.session.document().terrain_at(coord);
                    let after = TerrainTile {
                        underlay: Some(FloorUnderlayId::new(GROUND_UNDERLAY)),
                        overlay: None,
                        flags: 0,
                    };
                    if before != after {
                        ops.push(MapOp::SetTerrain {
                            coord,
                            before,
                            after,
                        });
                    }
                }
            }
            self.padded = commit_map_patch(viewer, MapPatch::new(ops), "Stage floor");
        }
    }

    /// Put `prefab` (turned `turns` quarter turns) on the stage when it is not already there.
    /// `size_of` gives an object's footprint (the catalog of the editor being worked in).
    pub(super) fn show(
        &mut self,
        file: &str,
        prefab: &Prefab,
        turns: u8,
        size_of: &dyn Fn(ObjectId, LocKind) -> (u8, u8),
    ) {
        if !self.ready() {
            return;
        }
        let want = Shown {
            file: file.to_owned(),
            turns,
        };
        if self.shown.as_ref() == Some(&want) {
            return;
        }
        let viewer = &mut self.viewer;
        if self.shown.take().is_some() {
            viewer.undo();
        }
        self.problem = None;
        self.since_shown = 0;
        let (width, height) = turned_size(prefab.width, prefab.height, turns);
        let origin = MapTile::new(
            CENTRE.0 - i32::from(width) / 2,
            CENTRE.1 - i32::from(height) / 2,
        );
        let Some(base_plane) = SourcePlane::new(0) else {
            return;
        };
        let spec = PlaceSpec {
            origin,
            base_plane,
            quarter_turns: turns,
            terrain: true,
            heights: true,
            objects: true,
            replace_objects: false,
        };
        match plan_place_prefab(viewer.session.document(), prefab, spec, size_of) {
            Ok(placed) => {
                if commit_map_patch(viewer, placed.patch, "Gallery preview") {
                    self.shown = Some(want);
                }
            }
            Err(error) => self.problem = Some(error.to_string()),
        }
        // Every prefab starts from the same three-quarter view, which is also what its still is.
        viewer.interaction.set_camera_pose(0.6, 0.75, None);
        // Frame it: pivot at the middle of the footprint, far enough back to see all of it.
        let side = f32::from(width.max(height)).max(6.0);
        let origin = viewer.renderer.render_origin();
        viewer.interaction.focus_camera(
            [
                (CENTRE.0 * 128 + 64 - origin.0) as f32,
                -(side * 40.0).min(900.0),
                (CENTRE.1 * 128 + 64 - origin.1) as f32,
            ],
            (side * 128.0 * 1.5).clamp(900.0, 9000.0),
        );
    }

    /// The file and turns of the prefab on the stage.
    pub(super) fn shown(&self) -> Option<(&str, u8)> {
        self.shown.as_ref().map(|s| (s.file.as_str(), s.turns))
    }

    /// Whether the prefab on the stage is built and drawn (so a still of it is complete).
    pub(super) fn settled(&self) -> bool {
        self.shown.is_some() && self.since_shown >= 45 && self.viewer.world.in_flight_count() == 0
    }

    /// A still of the stage, `width` x `height` RGBA8, in the stage's own view.
    pub(super) fn still(&self, width: u32, height: u32) -> Option<Vec<u8>> {
        let viewer = &self.viewer;
        let prefs = &viewer.settings.prefs;
        let params = osrs_render::gpu::FrameParams {
            camera: viewer.interaction.camera().reference(width as f32),
            view_plane: viewer.interaction.view_plane(),
            tick: 0,
            brightness: prefs.brightness,
            remove_color_banding: prefs.remove_color_banding,
            bright_textures: prefs.bright_textures,
            fog: prefs.fog(),
            colorblind: prefs.colorblind(),
            clear_color: prefs.sky_color.map(f64::from),
            profile: prefs.render_profile.render_profile(),
        };
        viewer.renderer.render_to_rgba(params, width, height)
    }

    /// Turn the camera round the prefab by `pixels` of drag (what a right drag of that length does).
    pub(super) fn spin(&mut self, pixels: f32) {
        self.viewer.interaction.apply_camera_input(CameraInput {
            delta: [pixels, 0.0],
            secondary_drag: true,
            ..CameraInput::default()
        });
    }
}
