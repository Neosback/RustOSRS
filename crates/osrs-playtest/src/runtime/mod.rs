//! Deterministic server movement with a separate 50 Hz client pose clock.
mod actor;
mod occupancy;
mod sequence;
use crate::navigation::{Navigation, Route, RouteTarget, find_path};
use actor::Actor;
use occupancy::Occupancy;
use osrs_core::{
    actor_snapshot::{
        ActorStatus, NpcActorSnapshot, PlaytestIdentity, PlaytestSnapshot, PlaytestStamp,
    },
    definitions::SequenceDefinition,
    entity::NpcDefinition,
    ids::{NpcId, SequenceId},
    scenario::{ActorLocation, NpcSpawnId, ScenarioDefinition},
};
use std::{collections::BTreeMap, sync::Arc};

pub const CLIENT_CYCLE_MS: u64 = 20;
pub const CYCLES_PER_SERVER_TICK: u64 = 30;

/// Immutable definition set bound to one cache fingerprint. Cache reads stay out
/// of simulation; callers populate this before installing a scenario.
#[derive(Debug, Default)]
pub struct RuntimeAssets {
    pub client_fingerprint: String,
    pub npcs: BTreeMap<NpcId, Arc<NpcDefinition>>,
    pub sequences: BTreeMap<SequenceId, Arc<SequenceDefinition>>,
}
#[derive(Debug, Clone, Copy)]
pub enum MoveSpeed {
    Crawl,
    Walk,
    Run,
}
impl MoveSpeed {
    fn steps(self, tick: u64) -> u8 {
        match self {
            Self::Crawl => u8::from(tick.is_multiple_of(2)),
            Self::Walk => 1,
            Self::Run => 2,
        }
    }
}
#[derive(Debug, Clone)]
pub enum RuntimeCommand {
    Start,
    Pause,
    SingleStep,
    Restart,
    Move {
        actor: NpcSpawnId,
        destination: ActorLocation,
        route: bool,
        speed: MoveSpeed,
    },
    Animate {
        actor: NpcSpawnId,
        sequence: Option<SequenceId>,
    },
}

pub struct PlaytestRuntime {
    scenario: ScenarioDefinition,
    assets: Arc<RuntimeAssets>,
    actors: BTreeMap<NpcSpawnId, Actor>,
    identity: PlaytestIdentity,
    cycle: u64,
    tick: u64,
    running: bool,
}
impl PlaytestRuntime {
    pub fn new(
        scenario: ScenarioDefinition,
        assets: Arc<RuntimeAssets>,
        identity: PlaytestIdentity,
    ) -> Result<Self, String> {
        scenario.validate()?;
        if assets.client_fingerprint != scenario.client_fingerprint {
            return Err("scenario and actor assets use different client caches".into());
        }
        if assets.npcs.values().any(|d| {
            d.identity.provenance.cache_fingerprint().to_string() != assets.client_fingerprint
        }) {
            return Err("NPC assets contain mixed cache provenance".into());
        }
        let actors = scenario
            .spawns
            .iter()
            .map(|s| {
                (
                    s.id,
                    Actor::new(s, assets.npcs.get(&s.npc).map(Arc::as_ref), scenario.seed),
                )
            })
            .collect();
        Ok(Self {
            scenario,
            assets,
            actors,
            identity,
            cycle: 0,
            tick: 0,
            running: false,
        })
    }
    pub fn identity(&self) -> PlaytestIdentity {
        self.identity
    }
    pub fn running(&self) -> bool {
        self.running
    }
    pub fn snapshot(&self) -> PlaytestSnapshot {
        PlaytestSnapshot {
            stamp: PlaytestStamp {
                identity: self.identity,
                server_tick: self.tick,
                client_cycle: self.cycle,
            },
            actors: self.actors.values().map(Actor::snapshot).collect(),
        }
    }
    /// Snapshot only the actors `keep` accepts (by simulated tile), so a large scenario
    /// does not clone every actor for each extraction.
    pub fn snapshot_where(
        &self,
        mut keep: impl FnMut(&NpcActorSnapshot) -> bool,
    ) -> PlaytestSnapshot {
        PlaytestSnapshot {
            stamp: self.snapshot_stamp(),
            actors: self
                .actors
                .values()
                .map(Actor::snapshot)
                .filter(|actor| keep(actor))
                .collect(),
        }
    }
    pub fn snapshot_stamp(&self) -> PlaytestStamp {
        PlaytestStamp {
            identity: self.identity,
            server_tick: self.tick,
            client_cycle: self.cycle,
        }
    }
    pub fn actor(&self, id: NpcSpawnId) -> Option<NpcActorSnapshot> {
        self.actors.get(&id).map(Actor::snapshot)
    }
    /// Queued walking destinations of one actor, nearest first.
    pub fn destinations(&self, id: NpcSpawnId) -> Vec<ActorLocation> {
        self.actors
            .get(&id)
            .map(|actor| actor.destination.iter().copied().collect())
            .unwrap_or_default()
    }
    pub fn command(
        &mut self,
        command: RuntimeCommand,
        view: &dyn Navigation,
    ) -> Result<(), String> {
        match command {
            RuntimeCommand::Start => {
                self.revalidate(view);
                self.running = true;
            }
            RuntimeCommand::Pause => self.running = false,
            RuntimeCommand::SingleStep => {
                self.running = false;
                for _ in 0..CYCLES_PER_SERVER_TICK {
                    self.cycle(view);
                }
            }
            RuntimeCommand::Restart => {
                self.identity.run_generation = self
                    .identity
                    .run_generation
                    .checked_add(1)
                    .ok_or("runtime generation exhausted")?;
                self.cycle = 0;
                self.tick = 0;
                self.running = false;
                self.actors = self
                    .scenario
                    .spawns
                    .iter()
                    .map(|s| {
                        (
                            s.id,
                            Actor::new(
                                s,
                                self.assets.npcs.get(&s.npc).map(Arc::as_ref),
                                self.scenario.seed,
                            ),
                        )
                    })
                    .collect();
                self.revalidate(view);
            }
            RuntimeCommand::Move {
                actor,
                destination,
                route,
                speed,
            } => {
                let actor = self.actors.get_mut(&actor).ok_or("unknown actor")?;
                if actor.spawn.location.plane != destination.plane {
                    return Err("walking cannot change plane".into());
                }
                actor.speed = speed;
                actor.manual = true;
                actor.destination.clear();
                if route {
                    match find_path(
                        view,
                        actor.location,
                        RouteTarget {
                            location: destination,
                            width: 1,
                            height: 1,
                        },
                        actor.size,
                        actor.spawn.restriction,
                        false,
                    ) {
                        Route::AlreadyThere => {}
                        Route::Found { waypoints, .. } => actor.destination.extend(waypoints),
                        Route::Unreachable => {
                            return Err("no route in current collision coverage".into());
                        }
                    }
                } else {
                    actor.destination.push_back(destination);
                }
            }
            RuntimeCommand::Animate { actor, sequence } => {
                if sequence.is_some_and(|id| !self.assets.sequences.contains_key(&id)) {
                    return Err("animation definition is unavailable".into());
                }
                let actor = self.actors.get_mut(&actor).ok_or("unknown actor")?;
                actor.sequences.action.sequence = sequence;
                actor.sequences.action.reset_frame();
            }
        }
        Ok(())
    }
    /// A map edit invalidates every old result immediately. The owner supplies
    /// accepted new collision inputs before resuming; no obsolete worker can advance it.
    pub fn invalidate_map(&mut self, revision: u64) {
        self.running = false;
        self.identity.map_revision = revision;
    }
    pub fn revalidate(&mut self, view: &dyn Navigation) {
        for actor in self.actors.values_mut() {
            actor.revalidate(view);
        }
    }
    pub fn advance_cycles(&mut self, cycles: u32, view: &dyn Navigation) {
        if self.running {
            for _ in 0..cycles {
                self.cycle(view);
            }
        }
    }
    fn cycle(&mut self, view: &dyn Navigation) {
        if self.cycle.is_multiple_of(CYCLES_PER_SERVER_TICK) {
            self.revalidate(view);
            let mut occupancy = Occupancy::new(self.actors.values());
            for actor in self.actors.values_mut() {
                if matches!(
                    actor.status,
                    ActorStatus::Parked
                        | ActorStatus::MissingDefinition
                        | ActorStatus::InvalidPlacement
                ) {
                    continue;
                }
                occupancy.remove(actor);
                actor.server_tick(self.tick, &occupancy.view(view));
                occupancy.add(actor);
            }
            self.tick = self.tick.saturating_add(1);
        }
        for actor in self.actors.values_mut() {
            actor.client_cycle(&self.assets);
        }
        self.cycle = self.cycle.saturating_add(1);
    }
}
