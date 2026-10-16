use osrs_core::scenario::{
    ActorBlocking, MovementRestriction, NpcBehavior, NpcSpawn, ScenarioDefinition,
};
use serde::Serialize;
use std::collections::BTreeMap;

/// Reviewable new files. This function never overwrites the OpenRune source.
#[derive(Debug)]
pub struct OpenRuneExport {
    pub spawns_toml: String,
    pub behaviors_toml: String,
}

/// OpenRune stores behavior per NPC definition. Refuse per-spawn conflicts,
/// unresolved symbols, and numeric/symbol aliases instead of broadening an edit.
pub fn export_scenario(scenario: &ScenarioDefinition) -> Result<OpenRuneExport, String> {
    scenario.validate()?;
    let mut definitions: BTreeMap<u32, &NpcSpawn> = BTreeMap::new();
    let mut symbols = BTreeMap::new();
    let mut spawns = Vec::new();
    for spawn in &scenario.spawns {
        if spawn.symbol.strip_prefix("npc.").is_none_or(str::is_empty) {
            return Err(format!("spawn {} needs an OpenRune NPC symbol", spawn.id.0));
        }
        if symbols
            .insert(&spawn.symbol, spawn.npc)
            .is_some_and(|id| id != spawn.npc)
        {
            return Err(format!("symbol {} has conflicting IDs", spawn.symbol));
        }
        if let Some(old) = definitions.insert(spawn.npc.get(), spawn)
            && (old.symbol != spawn.symbol
                || old.behavior != spawn.behavior
                || old.restriction != spawn.restriction
                || old.blocking != spawn.blocking
                || old.facing != spawn.facing)
        {
            return Err(format!(
                "{} has incompatible per-spawn settings ({} and {}); OpenRune behavior is shared by definition",
                spawn.symbol, old.id.0, spawn.id.0
            ));
        }
        let (region, tile) = spawn.location.tile.split_region();
        spawns.push(SpawnRow {
            npc: &spawn.symbol,
            coords: format!(
                "{}_{}_{}_{}_{}",
                spawn.location.plane.get(),
                region.x,
                region.y,
                tile.x(),
                tile.y()
            ),
        });
    }
    let mut npc = Vec::new();
    for spawn in definitions.values() {
        let (mode, radius, waypoints) = match &spawn.behavior {
            NpcBehavior::Stationary => ("None", 0, Vec::new()),
            NpcBehavior::Wander { radius } => ("Wander", *radius, Vec::new()),
            NpcBehavior::Patrol { waypoints } => (
                "Patrol",
                0,
                waypoints
                    .iter()
                    .map(|p| WaypointRow {
                        destination: CoordRow {
                            x: p.destination.tile.x,
                            z: p.destination.tile.y,
                            level: p.destination.plane.get(),
                        },
                        pause_delay: p.pause_ticks,
                    })
                    .collect(),
            ),
        };
        let direction = match spawn.facing {
            0 => "South",
            256 => "SouthWest",
            512 => "West",
            768 => "NorthWest",
            1024 => "North",
            1280 => "NorthEast",
            1536 => "East",
            1792 => "SouthEast",
            _ => {
                return Err(format!(
                    "{} facing cannot be represented by OpenRune's eight spawn directions",
                    spawn.symbol
                ));
            }
        };
        npc.push(BehaviorRow {
            id: &spawn.symbol,
            inherit: &spawn.symbol,
            default_mode: mode,
            wander_range: radius,
            move_restrict: spawn.restriction,
            block_walk: spawn.blocking,
            respawn_dir: direction,
            waypoints,
        });
    }
    Ok(OpenRuneExport {
        spawns_toml: toml::to_string_pretty(&SpawnFile { spawn: spawns })
            .map_err(|e| e.to_string())?,
        behaviors_toml: toml::to_string_pretty(&BehaviorFile { npc }).map_err(|e| e.to_string())?,
    })
}
#[derive(Serialize)]
struct SpawnFile<'a> {
    spawn: Vec<SpawnRow<'a>>,
}
#[derive(Serialize)]
struct SpawnRow<'a> {
    npc: &'a str,
    coords: String,
}
#[derive(Serialize)]
struct BehaviorFile<'a> {
    npc: Vec<BehaviorRow<'a>>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BehaviorRow<'a> {
    id: &'a str,
    inherit: &'a str,
    default_mode: &'a str,
    wander_range: u16,
    move_restrict: MovementRestriction,
    block_walk: ActorBlocking,
    respawn_dir: &'a str,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    waypoints: Vec<WaypointRow>,
}
#[derive(Serialize)]
struct WaypointRow {
    destination: CoordRow,
    pause_delay: u32,
}
#[derive(Serialize)]
struct CoordRow {
    x: i32,
    z: i32,
    level: u8,
}
