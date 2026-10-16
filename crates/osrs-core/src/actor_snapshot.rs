//! Renderer-neutral offline actor projections. They never own GPU resources.
use crate::{
    coords::LocalXZ,
    entity::ActorAnimationState,
    ids::NpcId,
    scenario::{ActorLocation, NpcSpawnId},
};

/// Identity shared by immutable runtime inputs and accepted output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlaytestIdentity {
    pub source_generation: u64,
    pub map_revision: u64,
    pub scenario_revision: u64,
    pub run_generation: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaytestStamp {
    pub identity: PlaytestIdentity,
    pub server_tick: u64,
    pub client_cycle: u64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActorStatus {
    Ready,
    Blocked,
    Parked,
    InvalidPlacement,
    MissingDefinition,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NpcActorSnapshot {
    pub spawn: NpcSpawnId,
    pub npc: NpcId,
    pub location: ActorLocation,
    pub position: LocalXZ,
    pub orientation: u16,
    pub rotation: u16,
    pub size: u16,
    pub sequences: ActorAnimationState,
    pub status: ActorStatus,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaytestSnapshot {
    pub stamp: PlaytestStamp,
    pub actors: Vec<NpcActorSnapshot>,
}
