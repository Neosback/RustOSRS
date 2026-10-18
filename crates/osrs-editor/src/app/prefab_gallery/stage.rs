//! The gallery's stage: a second, isolated editor over a synthetic map, where the
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
    ids::ObjectId,
};
use osrs_edit::{LocKind, PlaceSpec, Prefab, plan_place_exact, turned_size};

/// Coordinates of the virtual stage. No terrain or placements are read from this cache location.
pub(in crate::app) const CENTRE: (i32, i32) = (3232, 2144);
/// Half the side of the floor laid under the prefab, in tiles.
const PAD_HALF: i32 = 72;
/// Underlay of the stage floor: the grass most captured buildings stand on.
#[cfg(not(target_arch = "wasm32"))]
const GROUND_UNDERLAY: u16 = 50;

/// What is on the stage now.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Shown {
    file: String,
    turns: u8,
}

pub(in crate::app) struct Stage {
    viewer: Option<Box<EditorApp>>,
    padded: bool,
    shown: Option<Shown>,
    /// Frames since the prefab was last put on the stage (the world needs a few to build it).
    since_shown: u32,
    /// Why the last placement failed (shown under the view).
    pub(in crate::app) problem: Option<String>,
}

impl Stage {
    /// A stage sharing `parent`'s GPU device and cache. Native only: the browser build cannot
    /// read a cache folder or share the device this way.
    #[cfg(not(target_arch = "wasm32"))]
    pub(in crate::app) fn new(parent: &EditorApp, ctx: &egui::Context) -> Result<Self, String> {
        use crate::hud::Destination;
        let label = parent.project.cache_label().clone();
        let mut viewer = EditorApp::from_source(
            ctx,
            parent.renderer.render_state(),
            parent.project.source().clone(),
            label,
            (CENTRE.0.div_euclid(64) - 1) * 64,
            (CENTRE.1.div_euclid(64) - 1) * 64,
            false,
        )?;
        viewer.world.use_virtual_map(GROUND_UNDERLAY);
        viewer.prefabs.stage_region = Some(MapTile::new(CENTRE.0, CENTRE.1).split_region().0);
        viewer.settings.prefs = parent.settings.prefs.clone();
        viewer
            .settings
            .keymap
            .load(&parent.settings.keymap.to_saved());
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
        viewer.palettes.load(parent.project.source().clone());
        viewer.teleport_to(Destination::Tile {
            x: CENTRE.0,
            y: CENTRE.1,
            plane: 0,
        });
        viewer
            .interaction
            .set_camera_mode(crate::camera::CameraMode::Orbit);
        Ok(Self {
            viewer: Some(Box::new(viewer)),
            padded: false,
            shown: None,
            since_shown: 0,
            problem: None,
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub(in crate::app) fn new(_parent: &EditorApp, _ctx: &egui::Context) -> Result<Self, String> {
        Err("The 3D gallery needs the desktop app.".to_owned())
    }

    pub(in crate::app) fn viewer(&mut self) -> &mut EditorApp {
        self.viewer
            .as_deref_mut()
            .unwrap_or_else(|| unreachable!("stage viewer exists until drop"))
    }

    pub(in crate::app) fn ready(&self) -> bool {
        self.padded
            && self
                .viewer
                .as_deref()
                .unwrap_or_else(|| unreachable!())
                .world
                .is_ready()
    }

    /// Once per frame: let the synthetic source stream and animate. Its floor is source
    /// baseline data, so opening the workspace creates no undo entry.
    pub(in crate::app) fn step(&mut self, ctx: &egui::Context) {
        let viewer = self.viewer.as_deref_mut().unwrap_or_else(|| unreachable!());
        viewer.rebase_if_needed();
        viewer.stream(ctx);
        viewer.update_animations();
        viewer.palettes.poll();
        viewer.poll_catalog();
        viewer.assets.poll(ctx);
        crate::app::prefab_panel::update(viewer);
        self.since_shown = self.since_shown.saturating_add(1);
        if !self.padded
            && self.problem.is_none()
            && viewer.world.is_ready()
            && viewer.world.resident_regions() > 0
        {
            let doc = viewer.session.document();
            let covered = (CENTRE.0 - PAD_HALF..=CENTRE.0 + PAD_HALF).all(|x| {
                (CENTRE.1 - PAD_HALF..=CENTRE.1 + PAD_HALF).all(|y| {
                    (0..4).all(|p| {
                        SourcePlane::new(p).is_some_and(|plane| {
                            doc.height_at(osrs_edit::HeightVertexCoord::new(
                                MapTile::new(x, y),
                                plane,
                            ))
                            .is_some()
                        })
                    })
                })
            });
            if !covered {
                return;
            }
            self.padded = true;
        }
    }

    /// Put `prefab` (turned `turns` quarter turns) on the stage when it is not already there.
    /// `size_of` gives an object's footprint (the catalog of the editor being worked in).
    pub(in crate::app) fn show(
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
        let viewer = self.viewer.as_deref_mut().unwrap_or_else(|| unreachable!());
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
        match plan_place_exact(viewer.session.document(), prefab, spec, size_of) {
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

    pub(in crate::app) fn clear_preview(&mut self) {
        if self.shown.take().is_some() {
            self.viewer().undo();
        }
    }

    /// The file and turns of the prefab on the stage.
    pub(in crate::app) fn shown(&self) -> Option<(&str, u8)> {
        self.shown.as_ref().map(|s| (s.file.as_str(), s.turns))
    }

    /// Whether the prefab on the stage is built and drawn (so a still of it is complete).
    pub(in crate::app) fn settled(&self) -> bool {
        self.shown.is_some()
            && self.since_shown >= 45
            && self
                .viewer
                .as_deref()
                .unwrap_or_else(|| unreachable!())
                .world
                .in_flight_count()
                == 0
    }

    /// A still of the stage, `width` x `height` RGBA8, in the stage's own view.
    pub(in crate::app) fn still(&self, width: u32, height: u32) -> Option<Vec<u8>> {
        let viewer = self.viewer.as_deref().unwrap_or_else(|| unreachable!());
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
    pub(in crate::app) fn spin(&mut self, pixels: f32) {
        self.viewer().interaction.apply_camera_input(CameraInput {
            delta: [pixels, 0.0],
            secondary_drag: true,
            ..CameraInput::default()
        });
    }
}

impl Drop for Stage {
    fn drop(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(mut viewer) = self.viewer.take() {
            viewer.world.shutdown_in_background();
            viewer.assets.shutdown_in_background();
        }
    }
}
