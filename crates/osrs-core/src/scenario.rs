//! Shared offline scenario values. Authored history belongs to osrs-edit;
//! live positions, queues, clocks and occupancy belong to osrs-playtest.
use crate::{
    coords::{MapTile, PlaneIndex},
    ids::NpcId,
};
use serde::{Deserialize, Serialize};

/// Persistent identity of one spawn, including colocated duplicates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NpcSpawnId(pub u64);

/// A tile on a validated semantic plane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ActorLocation {
    pub tile: MapTile,
    pub plane: PlaneIndex,
}

/// OpenRune walking restrictions, kept separate from collision bit layouts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MovementRestriction {
    #[default]
    Normal,
    Blocked,
    BlockedNormal,
    Indoors,
    Outdoors,
    NoMove,
    PassThru,
}

/// Which dynamic occupancy classes this NPC contributes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ActorBlocking {
    None,
    #[default]
    Npc,
    All,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatrolWaypoint {
    pub destination: ActorLocation,
    pub pause_ticks: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NpcBehavior {
    Stationary,
    Wander { radius: u16 },
    Patrol { waypoints: Vec<PatrolWaypoint> },
}

/// Editable spawn and behavior, never the actor's current simulated position.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NpcSpawn {
    pub id: NpcSpawnId,
    pub npc: NpcId,
    pub symbol: String,
    pub location: ActorLocation,
    pub facing: u16,
    pub behavior: NpcBehavior,
    pub restriction: MovementRestriction,
    pub blocking: ActorBlocking,
    pub source: SpawnSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpawnSource {
    pub file: String,
    pub entry: u32,
}

/// Separate versioned sidecar; existing map/project formats are untouched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioDefinition {
    pub version: u32,
    pub build: u32,
    pub client_fingerprint: String,
    pub seed: u64,
    pub spawns: Vec<NpcSpawn>,
}

impl ScenarioDefinition {
    /// Validate before installation or authoring commit, including serde inputs.
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err(format!("unsupported scenario version {}", self.version));
        }
        if self.build < 240 {
            return Err("scenario requires a supported modern cache".into());
        }
        if self.client_fingerprint.len() != 64
            || !self
                .client_fingerprint
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("scenario needs a lowercase SHA-256 client cache fingerprint".into());
        }
        let mut seen = std::collections::BTreeSet::new();
        for spawn in &self.spawns {
            if !seen.insert(spawn.id) {
                return Err(format!("duplicate spawn identity {}", spawn.id.0));
            }
            if spawn.facing >= 2048 {
                return Err(format!("invalid facing for spawn {}", spawn.id.0));
            }
            validate_location(spawn.location)?;
            if let NpcBehavior::Patrol { waypoints } = &spawn.behavior {
                if waypoints.is_empty() {
                    return Err(format!("empty patrol for spawn {}", spawn.id.0));
                }
                for waypoint in waypoints {
                    validate_location(waypoint.destination)?;
                }
            }
        }
        Ok(())
    }
}

fn validate_location(location: ActorLocation) -> Result<(), String> {
    if !(0..=32767).contains(&location.tile.x) || !(0..=32767).contains(&location.tile.y) {
        return Err(format!("out of range actor tile {:?}", location.tile));
    }
    Ok(())
}
