//! The mock player's model in the world: asks the world worker for the player's geometry (look
//! and idle animation) and shows it on its own entity layer. The look is the Player panel's
//! settings; nothing here changes them, the map, or the NPC layer.
use super::{
    npc_runtime::WorldNavigation, render_runtime::RenderRuntime, world_runtime::WorldRuntime,
};
use osrs_core::{
    coords::{MapTile, PlaneIndex},
    ids::SequenceId,
    player::PlayerLook,
    scenario::ActorLocation,
};
use osrs_edit::MapDocument;
use osrs_playtest::{
    navigation::Navigation,
    walker::{PlayerWalker, WalkOutcome},
};
use osrs_render::SceneGeometry;
use osrs_world::npc::{PlayerFrame, PlayerGeometryUpdate};

/// Renderer key of the player layer (beside the NPC and animated-loc layers).
const PLAYER_KEY: (i32, i32) = (i32::MIN + 2, i32::MIN + 2);
/// The standing animation every player starts from (the client's default player idle).
const IDLE_SEQUENCE: u32 = 808;
/// Standard walking animation.
const WALK_SEQUENCE: u32 = 819;
/// Standard running animation.
const RUN_SEQUENCE: u32 = 824;
/// Server tick duration in seconds (600ms).
const TICK_DURATION: f32 = 0.6;
/// Client cycles are 20 ms; the model is rebuilt every this many (25 times a second).
const CYCLES_PER_REBUILD: f64 = 2.0;

/// Where and how the player stands, from the settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PlayerSpec {
    pub look: PlayerLook,
    pub tile: (i32, i32),
    pub plane: u8,
}

pub(super) struct PlayerHost<'a> {
    pub world: &'a WorldRuntime,
    pub map: &'a MapDocument,
    pub renderer: &'a mut RenderRuntime,
    pub dt: f32,
    pub spec: Option<PlayerSpec>,
    pub running: bool,
}

/// What the last request was built from; a different key means a new model is due.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SentKey {
    spec: PlayerSpec,
    frame: u64,
    infos_version: u64,
    pos: (i32, i32),
    rotation: u16,
    sequence: u32,
}

#[derive(Default)]
pub(super) struct PlayerRuntime {
    token: u64,
    inflight: Option<u64>,
    sent: Option<SentKey>,
    shown: bool,
    cycles: f64,
    error: Option<String>,
    walker: Option<PlayerWalker>,
    tick_timer: f32,
    rotation: u16,
    world_position: Option<[f32; 3]>,
    click_marker: Option<((i32, i32), f32)>,
}

impl PlayerRuntime {
    /// A cache was opened: the old model belongs to the old cache.
    pub(super) fn source_opened(&mut self, renderer: &mut RenderRuntime) {
        let token = self.token + 1;
        *self = Self {
            token,
            ..Self::default()
        };
        renderer.upload_transient(PLAYER_KEY, &SceneGeometry::default());
    }

    /// Why the model could not be drawn, for the Player panel.
    pub(super) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub(super) fn teleport(&mut self, (x, y, plane): (i32, i32, u8)) {
        if let Some(plane_idx) = PlaneIndex::new(plane) {
            let loc = ActorLocation {
                tile: MapTile::new(x, y),
                plane: plane_idx,
            };
            if let Some(walker) = &mut self.walker {
                walker.teleport(loc);
            } else {
                self.walker = Some(PlayerWalker::new(loc));
            }
            self.tick_timer = 0.0;
            let (wx, wz) = (x as f32 * 128.0 + 64.0, y as f32 * 128.0 + 64.0);
            self.world_position = Some([wx, 0.0, wz]);
        }
    }

    pub(super) fn walk_to(
        &mut self,
        nav: &dyn Navigation,
        target: (i32, i32),
        plane: u8,
    ) -> WalkOutcome {
        let Some(plane_idx) = PlaneIndex::new(plane) else {
            return WalkOutcome::Unreachable;
        };
        let target_loc = ActorLocation {
            tile: MapTile::new(target.0, target.1),
            plane: plane_idx,
        };
        let walker = self
            .walker
            .get_or_insert_with(|| PlayerWalker::new(target_loc));
        walker.walk_to(nav, target_loc)
    }

    pub(super) fn current_tile(&self) -> Option<(i32, i32)> {
        self.walker
            .as_ref()
            .map(|w| (w.location().tile.x, w.location().tile.y))
    }

    pub(super) fn world_position(&self) -> Option<[f32; 3]> {
        self.world_position
    }

    pub(super) fn set_click_marker(&mut self, tile: (i32, i32)) {
        self.click_marker = Some((tile, 1.2));
    }

    pub(super) fn click_marker(&self) -> Option<((i32, i32), f32)> {
        self.click_marker
    }

    fn hide(&mut self, renderer: &mut RenderRuntime) {
        if self.shown {
            renderer.upload_transient(PLAYER_KEY, &SceneGeometry::default());
            self.shown = false;
        }
        self.sent = None;
    }

    /// Request a new model when the look, place or animation frame changed. Returns true while the
    /// animation needs continuous repainting.
    pub(super) fn update(&mut self, host: &mut PlayerHost<'_>) -> bool {
        let Some(spec) = host.spec.clone() else {
            self.hide(host.renderer);
            self.error = None;
            self.walker = None;
            self.world_position = None;
            return false;
        };
        let Some(location) = PlaneIndex::new(spec.plane).map(|plane| ActorLocation {
            tile: MapTile::new(spec.tile.0, spec.tile.1),
            plane,
        }) else {
            return false;
        };

        // Sync walker with location if not moving or if teleports occurred.
        let walker = self
            .walker
            .get_or_insert_with(|| PlayerWalker::new(location));
        if !walker.is_moving() && walker.location() != location {
            walker.teleport(location);
            self.tick_timer = 0.0;
        }

        // Advance simulation tick if moving.
        self.tick_timer += host.dt;
        if walker.is_moving() {
            let nav = WorldNavigation {
                world: host.world,
                map: host.map,
            };
            let steps = if host.running { 2 } else { 1 };
            while self.tick_timer >= TICK_DURATION {
                self.tick_timer -= TICK_DURATION;
                walker.tick(&nav, steps);
                if !walker.is_moving() {
                    break;
                }
            }
        } else if self.tick_timer >= TICK_DURATION {
            self.tick_timer = self.tick_timer.fract();
        }

        // Sub-tile interpolation
        let is_moving = walker.is_moving();
        let (wx, wz) = if is_moving {
            let prev = walker.previous();
            let curr = walker.location().tile;
            let phase = (self.tick_timer / TICK_DURATION).clamp(0.0, 1.0);
            let px = prev.x as f32 * 128.0 + 64.0;
            let pz = prev.y as f32 * 128.0 + 64.0;
            let cx = curr.x as f32 * 128.0 + 64.0;
            let cz = curr.y as f32 * 128.0 + 64.0;
            (px + (cx - px) * phase, pz + (cz - pz) * phase)
        } else {
            let curr = walker.location().tile;
            (curr.x as f32 * 128.0 + 64.0, curr.y as f32 * 128.0 + 64.0)
        };

        if let Some((_, timer)) = &mut self.click_marker {
            *timer -= host.dt;
            if *timer <= 0.0 {
                self.click_marker = None;
            }
        }

        if is_moving {
            let target = walker.orientation() as i32;
            let current = self.rotation as i32;
            let mut diff = (target - current) & 2047;
            if diff > 1024 {
                diff -= 2048;
            }
            let turn_speed = (host.dt * 6000.0).round() as i32;
            let step = diff.clamp(-turn_speed, turn_speed);
            self.rotation = ((current + step) & 2047) as u16;
        }

        let sequence_id = if is_moving {
            if host.running {
                RUN_SEQUENCE
            } else {
                WALK_SEQUENCE
            }
        } else {
            IDLE_SEQUENCE
        };

        self.cycles += f64::from(host.dt.min(0.25)) / 0.02;
        let pos_i = (wx.round() as i32, wz.round() as i32);
        let Some(ground) = ground_height(host.world, pos_i.0, pos_i.1, spec.plane) else {
            return true;
        };
        self.world_position = Some([wx, ground as f32, wz]);

        if self.inflight.is_some() {
            return true;
        }

        let key = SentKey {
            frame: (self.cycles / CYCLES_PER_REBUILD) as u64,
            infos_version: host.world.infos_version(),
            spec: spec.clone(),
            pos: pos_i,
            rotation: self.rotation,
            sequence: sequence_id,
        };
        if self.sent.as_ref() == Some(&key) {
            return true;
        }

        self.token += 1;
        self.inflight = Some(self.token);
        self.sent = Some(key);
        host.world.request_player_geometry(PlayerFrame {
            token: self.token,
            look: spec.look,
            location: walker.location(),
            position: pos_i,
            rotation: self.rotation,
            ground_height: ground,
            sequence: Some(SequenceId::new(sequence_id)),
            cycles: self.cycles as u64,
        });
        true
    }

    pub(super) fn arrived(&mut self, update: PlayerGeometryUpdate, renderer: &mut RenderRuntime) {
        if self.inflight != Some(update.token) {
            // An answer to a request from before the cache changed.
            return;
        }
        self.inflight = None;
        renderer.upload_transient(PLAYER_KEY, &update.geometry);
        self.shown = !update.geometry.zones.is_empty();
        self.error = update.errors.into_iter().next();
    }
}

fn ground_height(world: &WorldRuntime, x: i32, z: i32, plane: u8) -> Option<i32> {
    let region = (
        x.div_euclid(128).div_euclid(64),
        z.div_euclid(128).div_euclid(64),
    );
    let info = world.infos().get(&region)?;
    let height = info.height_at(usize::from(plane), x as f32, z as f32)?;
    Some(height.round() as i32)
}
