//! The mock player's model in the world: asks the world worker for the player's geometry (look
//! and idle animation) and shows it on its own entity layer. The look is the Player panel's
//! settings; nothing here changes them, the map, or the NPC layer.
use super::{render_runtime::RenderRuntime, world_runtime::WorldRuntime};
use osrs_core::{
    coords::{MapTile, PlaneIndex},
    ids::SequenceId,
    player::PlayerLook,
    scenario::ActorLocation,
};
use osrs_render::SceneGeometry;
use osrs_world::npc::{PlayerFrame, PlayerGeometryUpdate};

/// Renderer key of the player layer (beside the NPC and animated-loc layers).
const PLAYER_KEY: (i32, i32) = (i32::MIN + 2, i32::MIN + 2);
/// The standing animation every player starts from (the client's default player idle).
const IDLE_SEQUENCE: u32 = 808;
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
    pub renderer: &'a mut RenderRuntime,
    pub dt: f32,
    pub spec: Option<PlayerSpec>,
}

/// What the last request was built from; a different key means a new model is due.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SentKey {
    spec: PlayerSpec,
    frame: u64,
    infos_version: u64,
}

#[derive(Default)]
pub(super) struct PlayerRuntime {
    token: u64,
    inflight: Option<u64>,
    sent: Option<SentKey>,
    shown: bool,
    cycles: f64,
    error: Option<String>,
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

    fn hide(&mut self, renderer: &mut RenderRuntime) {
        if self.shown {
            renderer.upload_transient(PLAYER_KEY, &SceneGeometry::default());
            self.shown = false;
        }
        self.sent = None;
    }

    /// Request a new model when the look, place or animation frame changed. Returns true while the
    /// idle animation needs continuous repainting.
    pub(super) fn update(&mut self, host: &mut PlayerHost<'_>) -> bool {
        let Some(spec) = host.spec.clone() else {
            self.hide(host.renderer);
            self.error = None;
            return false;
        };
        self.cycles += f64::from(host.dt.min(0.25)) / 0.02;
        if self.inflight.is_some() {
            return true;
        }
        let key = SentKey {
            frame: (self.cycles / CYCLES_PER_REBUILD) as u64,
            infos_version: host.world.infos_version(),
            spec: spec.clone(),
        };
        if self.sent.as_ref() == Some(&key) {
            return true;
        }
        let Some(location) = PlaneIndex::new(spec.plane).map(|plane| ActorLocation {
            tile: MapTile::new(spec.tile.0, spec.tile.1),
            plane,
        }) else {
            return false;
        };
        let (x, z) = (spec.tile.0 * 128 + 64, spec.tile.1 * 128 + 64);
        // The ground comes from the loaded regions; until they are in, nothing is requested.
        let Some(ground) = ground_height(host.world, x, z, spec.plane) else {
            return true;
        };
        self.token += 1;
        self.inflight = Some(self.token);
        self.sent = Some(key);
        host.world.request_player_geometry(PlayerFrame {
            token: self.token,
            look: spec.look,
            location,
            position: (x, z),
            rotation: 0,
            ground_height: ground,
            sequence: Some(SequenceId::new(IDLE_SEQUENCE)),
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
