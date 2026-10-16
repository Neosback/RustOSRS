//! Offline NPC scenario playtest: the owner of everything between an authored scenario and the
//! entity geometry on screen.
//!
//! Authored scenario state lives in the session (`ScenarioDocument`, with undo/redo). This owner
//! holds only disposable projections: the catalog of NPC definitions, the immutable asset set,
//! the deterministic `PlaytestRuntime`, and the bookkeeping that accepts worker results. Every
//! result is stamped with `PlaytestIdentity`; a result for an older source, map edit, scenario
//! revision or run is discarded. Runtime movement never enters authoring history.

mod files;
mod import;
mod navigation;

pub(super) use files::{OpenTask, SaveTask, open_text, save_text};
pub(super) use import::{ImportEvent, ImportJob, ImportPreview, InstallationData};
#[cfg(not(target_arch = "wasm32"))]
pub(super) use import::{preview_import, read_installation};
pub(super) use navigation::WorldNavigation;

use super::{
    edit_runtime::EditRuntime, render_runtime::RenderRuntime, world_runtime::WorldRuntime,
};
use osrs_core::{
    actor_snapshot::{NpcActorSnapshot, PlaytestIdentity, PlaytestStamp},
    ids::{NpcId, VarpId},
    scenario::{NpcSpawnId, ScenarioDefinition},
};
use osrs_playtest::runtime::{CLIENT_CYCLE_MS, PlaytestRuntime, RuntimeAssets, RuntimeCommand};
use osrs_render::SceneGeometry;
use osrs_world::{
    NpcCatalogEntry,
    npc::{NpcAssetsUpdate, NpcFrameInput, NpcGeometryUpdate},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

/// Renderer key of the entity layer. It never collides with a streamed region or the animated-loc
/// layer, and moving an actor replaces only this layer, never terrain.
pub(super) const NPC_KEY: (i32, i32) = (i32::MIN + 1, i32::MIN + 1);
/// Most simulated client cycles run in one frame, so a stalled window cannot freeze the editor
/// catching up (the clock simply falls behind).
const MAX_CYCLES_PER_FRAME: u32 = 50;
/// Most actors extracted into one geometry request.
const MAX_RENDERED_ACTORS: usize = 512;
const MAX_DIAGNOSTICS: usize = 64;

/// The open cache the scenario is bound to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SourceBinding {
    pub fingerprint: String,
    pub build: u32,
}

/// Whether a stamped result belongs to the state it was requested for.
pub(super) fn identity_current(result: PlaytestIdentity, current: PlaytestIdentity) -> bool {
    result == current
}

struct Live {
    sim: PlaytestRuntime,
    scenario_revision: u64,
    map_revision: u64,
}

/// What the last extraction request was built from; a different key means new geometry is due.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SentKey {
    stamp: PlaytestStamp,
    infos_version: u64,
    camera: (i32, i32),
}

/// A pending viewport gesture armed from the panel.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) enum Arm {
    #[default]
    None,
    /// Place an NPC of this definition on the next tile click.
    Place { npc: NpcId, symbol: String },
    /// Send the selected actor to the next clicked tile (`route`: full routing, else step rules).
    Destination { route: bool },
}

pub(super) struct NpcHost<'a> {
    pub session: &'a EditRuntime,
    pub world: &'a WorldRuntime,
    pub renderer: &'a mut RenderRuntime,
    pub dt: f32,
    pub camera: (i32, i32),
    pub radius_tiles: i32,
    pub varps: BTreeMap<VarpId, i32>,
}

#[derive(Default)]
pub(super) struct NpcRuntime {
    source: Option<SourceBinding>,
    source_generation: u64,
    catalog: Arc<Vec<NpcCatalogEntry>>,
    catalog_requested: bool,
    pub(super) installation: Option<InstallationData>,
    pub(super) import_job: Option<ImportJob>,
    pub(super) preview: Option<ImportPreview>,
    pub(super) file_selection: Vec<bool>,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub(super) file_filter: String,
    live: Option<Live>,
    assets: Option<Arc<RuntimeAssets>>,
    assets_pending: bool,
    assets_missing: BTreeSet<NpcId>,
    assets_requested: BTreeSet<NpcId>,
    run_generation: u64,
    clock: f32,
    queued: Vec<RuntimeCommand>,
    geometry_inflight: Option<PlaytestStamp>,
    geometry_sent: Option<SentKey>,
    geometry_shown: bool,
    stale_rejected: u64,
    diagnostics: Vec<String>,
    pub(super) selection: Option<NpcSpawnId>,
    pub(super) draft: Option<(osrs_core::scenario::NpcSpawn, osrs_core::scenario::NpcSpawn)>,
    results_for: Option<(String, usize)>,
    results: Vec<usize>,
    pub(super) search: String,
    pub(super) arm: Arm,
    pub(super) notice: Option<String>,
    pub(super) open_task: Option<(OpenKind, OpenTask)>,
    pub(super) save_task: Option<SaveTask>,
    pub(super) export_preview: Option<ExportPreview>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OpenKind {
    Scenario,
}

/// A reviewed OpenRune export; files are written only after this is shown.
pub(super) struct ExportPreview {
    pub spawns_toml: String,
    pub behaviors_toml: String,
    pub error: Option<String>,
}

impl NpcRuntime {
    // ------------------------------------------------------------------ source lifecycle

    /// A cache worker finished opening: everything derived from the old source is obsolete.
    pub(super) fn source_opened(
        &mut self,
        fingerprint: String,
        build: u32,
        world: &WorldRuntime,
        renderer: &mut RenderRuntime,
    ) {
        let generation = self.source_generation + 1;
        *self = Self {
            source: Some(SourceBinding { fingerprint, build }),
            source_generation: generation,
            ..Self::default()
        };
        renderer.upload_transient(NPC_KEY, &SceneGeometry::default());
        world.request_npc_catalog();
        self.catalog_requested = true;
    }

    pub(super) fn source(&self) -> Option<&SourceBinding> {
        self.source.as_ref()
    }

    pub(super) fn catalog(&self) -> &Arc<Vec<NpcCatalogEntry>> {
        &self.catalog
    }

    /// Catalog indices matching `search` (name substring, or an exact NPC ID), cached per query.
    pub(super) fn search_results(&mut self) -> &[usize] {
        let key = (self.search.clone(), self.catalog.len());
        if self.results_for.as_ref() != Some(&key) {
            let needle = self.search.trim().to_ascii_lowercase();
            let id = needle.parse::<u32>().ok();
            self.results =
                self.catalog
                    .iter()
                    .enumerate()
                    .filter(|(_, entry)| {
                        !needle.is_empty()
                            && (Some(entry.id.get()) == id
                                || entry.name.as_ref().is_some_and(|name| {
                                    name.to_ascii_lowercase().contains(&needle)
                                }))
                    })
                    .map(|(index, _)| index)
                    .take(200)
                    .collect();
            self.results_for = Some(key);
        }
        &self.results
    }

    pub(super) fn diagnostics(&self) -> &[String] {
        &self.diagnostics
    }

    pub(super) fn stale_rejected(&self) -> u64 {
        self.stale_rejected
    }

    fn report(&mut self, message: String) {
        if !self.diagnostics.contains(&message) {
            if self.diagnostics.len() >= MAX_DIAGNOSTICS {
                self.diagnostics.remove(0);
            }
            self.diagnostics.push(message);
        }
    }

    // ------------------------------------------------------------------ worker results

    pub(super) fn catalog_arrived(
        &mut self,
        entries: Arc<Vec<NpcCatalogEntry>>,
        errors: Vec<String>,
    ) {
        self.catalog = entries;
        for error in errors.into_iter().take(8) {
            self.report(error);
        }
    }

    pub(super) fn assets_arrived(&mut self, update: NpcAssetsUpdate) {
        if update.identity.source_generation != self.source_generation {
            self.stale_rejected += 1;
            return;
        }
        self.assets_pending = false;
        self.assets_missing = self
            .assets_requested
            .iter()
            .filter(|id| !update.assets.npcs.contains_key(id))
            .copied()
            .collect();
        for error in update.errors.into_iter().take(16) {
            self.report(error);
        }
        self.assets = Some(update.assets);
        // The runtime is rebuilt on the next update with the new asset set.
        self.live = None;
    }

    pub(super) fn geometry_arrived(
        &mut self,
        update: NpcGeometryUpdate,
        renderer: &mut RenderRuntime,
    ) {
        self.geometry_inflight = None;
        let Some(live) = &self.live else {
            self.stale_rejected += 1;
            return;
        };
        if !identity_current(update.stamp.identity, live.sim.identity()) {
            self.stale_rejected += 1;
            self.geometry_sent = None;
            return;
        }
        renderer.upload_transient(NPC_KEY, &update.geometry);
        self.geometry_shown = !update.geometry.zones.is_empty();
        for error in update.errors.into_iter().take(16) {
            self.report(error);
        }
    }

    // ------------------------------------------------------------------ playtest

    pub(super) fn queue(&mut self, command: RuntimeCommand) {
        self.queued.push(command);
    }

    pub(super) fn running(&self) -> bool {
        self.live.as_ref().is_some_and(|live| live.sim.running())
    }

    pub(super) fn has_runtime(&self) -> bool {
        self.live.is_some()
    }

    pub(super) fn actor(&self, id: NpcSpawnId) -> Option<NpcActorSnapshot> {
        self.live.as_ref()?.sim.actor(id)
    }

    pub(super) fn destinations(&self, id: NpcSpawnId) -> Vec<osrs_core::scenario::ActorLocation> {
        self.live
            .as_ref()
            .map(|live| live.sim.destinations(id))
            .unwrap_or_default()
    }

    pub(super) fn assets(&self) -> Option<&Arc<RuntimeAssets>> {
        self.assets.as_ref()
    }

    pub(super) fn stamp(&self) -> Option<PlaytestStamp> {
        self.live.as_ref().map(|live| live.sim.snapshot_stamp())
    }

    pub(super) fn missing_definitions(&self) -> &BTreeSet<NpcId> {
        &self.assets_missing
    }

    fn required_ids(scenario: &ScenarioDefinition) -> BTreeSet<NpcId> {
        scenario.spawns.iter().map(|s| s.npc).collect()
    }

    fn identity(&self, host: &NpcHost<'_>, scenario_revision: u64) -> PlaytestIdentity {
        PlaytestIdentity {
            source_generation: self.source_generation,
            map_revision: host.session.map_revision(),
            scenario_revision,
            run_generation: self.run_generation,
        }
    }

    /// Reconcile with authored state, advance the clock, and request fresh geometry.
    /// Returns true while the simulation needs continuous repainting.
    pub(super) fn update(&mut self, host: &mut NpcHost<'_>) -> bool {
        if self.source.is_none() {
            return false;
        }
        let scenario = host.session.scenario();
        let Some(definition) = scenario.definition() else {
            if self.live.take().is_some() || self.geometry_shown {
                host.renderer
                    .upload_transient(NPC_KEY, &SceneGeometry::default());
                self.geometry_shown = false;
                self.geometry_sent = None;
            }
            self.queued.clear();
            return false;
        };
        let scenario_revision = scenario.revision();
        let stale = self
            .live
            .as_ref()
            .is_none_or(|live| live.scenario_revision != scenario_revision);
        if stale {
            self.rebuild(host, definition, scenario_revision);
        }
        let Some(live) = self.live.as_mut() else {
            return false;
        };
        let navigation = WorldNavigation {
            world: host.world,
            map: host.session.document(),
        };
        let map_revision = host.session.map_revision();
        if live.map_revision != map_revision {
            live.sim.invalidate_map(map_revision);
            live.map_revision = map_revision;
        }
        let mut commands = std::mem::take(&mut self.queued);
        let mut restarted = false;
        for command in commands.drain(..) {
            restarted |= matches!(command, RuntimeCommand::Restart);
            if let Err(error) = live.sim.command(command, &navigation) {
                self.notice = Some(error);
            }
        }
        if restarted {
            self.run_generation = live.sim.identity().run_generation;
            self.geometry_sent = None;
        }
        if live.sim.running() {
            self.clock += host.dt.min(0.25);
            let step = CLIENT_CYCLE_MS as f32 / 1000.0;
            let cycles = ((self.clock / step) as u32).min(MAX_CYCLES_PER_FRAME);
            self.clock = (self.clock - cycles as f32 * step).clamp(0.0, step * 4.0);
            live.sim.advance_cycles(cycles, &navigation);
        } else {
            self.clock = 0.0;
        }
        self.request_geometry(host);
        self.live.as_ref().is_some_and(|live| live.sim.running())
    }

    fn rebuild(
        &mut self,
        host: &NpcHost<'_>,
        definition: &ScenarioDefinition,
        scenario_revision: u64,
    ) {
        self.live = None;
        self.geometry_sent = None;
        let required = Self::required_ids(definition);
        let covered = self.assets.as_ref().is_some_and(|assets| {
            required
                .iter()
                .all(|id| assets.npcs.contains_key(id) || self.assets_missing.contains(id))
        });
        if !covered {
            if !self.assets_pending {
                self.assets_pending = true;
                self.assets_requested.clone_from(&required);
                host.world.request_npc_assets(
                    PlaytestIdentity {
                        source_generation: self.source_generation,
                        map_revision: host.session.map_revision(),
                        scenario_revision,
                        run_generation: self.run_generation,
                    },
                    required,
                );
            }
            return;
        }
        let Some(assets) = self.assets.clone() else {
            return;
        };
        self.run_generation += 1;
        let identity = self.identity(host, scenario_revision);
        match PlaytestRuntime::new(definition.clone(), assets, identity) {
            Ok(sim) => {
                self.live = Some(Live {
                    sim,
                    scenario_revision,
                    map_revision: identity.map_revision,
                });
            }
            Err(error) => self.report(format!("playtest unavailable: {error}")),
        }
    }

    fn request_geometry(&mut self, host: &NpcHost<'_>) {
        let Some(live) = &self.live else {
            return;
        };
        if self.geometry_inflight.is_some() {
            return;
        }
        let key = SentKey {
            stamp: live.sim.snapshot_stamp(),
            infos_version: host.world.infos_version(),
            camera: host.camera,
        };
        if self.geometry_sent.as_ref() == Some(&key) {
            return;
        }
        let (cx, cy) = host.camera;
        let radius = i64::from(host.radius_tiles);
        let mut snapshot = live.sim.snapshot_where(|actor| {
            let dx = i64::from(actor.location.tile.x - cx);
            let dy = i64::from(actor.location.tile.y - cy);
            dx * dx + dy * dy <= radius * radius
        });
        if snapshot.actors.len() > MAX_RENDERED_ACTORS {
            snapshot.actors.sort_by_key(|actor| {
                let dx = i64::from(actor.location.tile.x - cx);
                let dy = i64::from(actor.location.tile.y - cy);
                dx * dx + dy * dy
            });
            snapshot.actors.truncate(MAX_RENDERED_ACTORS);
        }
        let ground_heights = snapshot
            .actors
            .iter()
            .filter_map(|actor| Some((actor.spawn, ground_height(host.world, actor)?)))
            .collect();
        self.geometry_inflight = Some(snapshot.stamp);
        self.geometry_sent = Some(key);
        host.world.request_npc_geometry(NpcFrameInput {
            snapshot,
            ground_heights,
            varps: host.varps.clone(),
        });
    }
}

/// Terrain height under an actor's simulated position, from the accepted region projection.
fn ground_height(world: &WorldRuntime, actor: &NpcActorSnapshot) -> Option<i32> {
    let (x, z) = (actor.position.x.units(), actor.position.z.units());
    let region = (
        x.div_euclid(128).div_euclid(64),
        z.div_euclid(128).div_euclid(64),
    );
    let info = world.infos().get(&region)?;
    let height = info.height_at(usize::from(actor.location.plane.get()), x as f32, z as f32)?;
    Some(height.round() as i32)
}

#[cfg(test)]
mod tests;
