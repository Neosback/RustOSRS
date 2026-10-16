use super::GamevalSymbols;
use osrs_core::{
    coords::{MapTile, PlaneIndex},
    ids::NpcId,
    scenario::{
        ActorBlocking, ActorLocation, MovementRestriction, NpcBehavior, NpcSpawn, NpcSpawnId,
        PatrolWaypoint, ScenarioDefinition, SpawnSource,
    },
};
use std::collections::{BTreeMap, BTreeSet};

/// Relative source path and text; also usable with browser-uploaded files.
#[derive(Debug, Clone)]
pub struct SourceText {
    pub path: String,
    pub text: String,
}

/// A preview may contain valid rows and diagnostics. It cannot be installed
/// through `into_scenario` until all errors have been resolved.
#[derive(Debug)]
pub struct ImportReport {
    pub spawns: Vec<NpcSpawn>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}
impl ImportReport {
    pub fn into_scenario(
        self,
        build: u32,
        client_fingerprint: String,
        seed: u64,
    ) -> Result<ScenarioDefinition, Vec<String>> {
        if !self.errors.is_empty() {
            return Err(self.errors);
        }
        let scenario = ScenarioDefinition {
            version: 1,
            build,
            client_fingerprint,
            seed,
            spawns: self.spawns,
        };
        scenario.validate().map_err(|e| vec![e])?;
        Ok(scenario)
    }
}

/// Parse spawns and server overrides without filesystem or application access.
/// Sorted relative paths and ordinal IDs preserve colocated duplicates.
pub fn import_spawns(
    files: &[SourceText],
    server_files: &[SourceText],
    symbols: &GamevalSymbols,
) -> ImportReport {
    let mut report = ImportReport {
        spawns: Vec::new(),
        errors: Vec::new(),
        warnings: Vec::new(),
    };
    let mut overrides = BTreeMap::new();
    for file in server_files {
        let parsed: Result<toml::Value, _> = toml::from_str(&file.text);
        match parsed.map(|doc| {
            doc.get("npc")
                .and_then(toml::Value::as_array)
                .cloned()
                .unwrap_or_default()
        }) {
            Ok(rows) => {
                for row in rows {
                    let Some(id) = row.get("id").and_then(toml::Value::as_str) else {
                        report.errors.push(format!("{}: missing NPC id", file.path));
                        continue;
                    };
                    if overrides.insert(id.to_owned(), row.clone()).is_some() {
                        report.warnings.push(format!(
                            "{}: later NPC override replaces {id} (OpenRune associateBy order)",
                            file.path
                        ));
                    }
                }
            }
            Err(e) => report.errors.push(format!("{}: {e}", file.path)),
        }
    }
    let mut files: Vec<_> = files.iter().collect();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let mut next_id = 1;
    let mut paths = BTreeSet::new();
    for file in files {
        if !paths.insert(&file.path) {
            report
                .errors
                .push(format!("duplicate source path {}", file.path));
            continue;
        }
        let parsed: Result<toml::Value, _> = toml::from_str(&file.text);
        let rows = match parsed {
            Ok(doc) => match doc.get("spawn").and_then(toml::Value::as_array) {
                Some(rows) => rows.clone(),
                None => {
                    report
                        .errors
                        .push(format!("{}: missing [[spawn]] entries", file.path));
                    continue;
                }
            },
            Err(e) => {
                report.errors.push(format!("{}: {e}", file.path));
                continue;
            }
        };
        for (entry, row) in rows.iter().enumerate() {
            let id = NpcSpawnId(next_id);
            next_id += 1;
            match parse_spawn(
                id,
                row,
                &overrides,
                symbols,
                SpawnSource {
                    file: file.path.clone(),
                    entry: entry as u32,
                },
            ) {
                Ok(spawn) => report.spawns.push(spawn),
                Err(e) => report
                    .errors
                    .push(format!("{}: spawn {}: {e}", file.path, entry + 1)),
            }
        }
    }
    report
}

fn parse_spawn(
    id: NpcSpawnId,
    row: &toml::Value,
    overrides: &BTreeMap<String, toml::Value>,
    symbols: &GamevalSymbols,
    source: SpawnSource,
) -> Result<NpcSpawn, String> {
    let symbol = row
        .get("npc")
        .and_then(toml::Value::as_str)
        .ok_or("missing NPC symbol")?;
    let npc = NpcId::new(symbols.resolve(symbol)?);
    let coords = row
        .get("coords")
        .and_then(toml::Value::as_str)
        .ok_or("missing coords")?;
    let parts: Vec<_> = coords
        .split('_')
        .map(str::parse::<i32>)
        .collect::<Result<_, _>>()
        .map_err(|_| format!("invalid coords {coords}"))?;
    if parts.len() != 5
        || !(0..=3).contains(&parts[0])
        || !(0..=511).contains(&parts[1])
        || !(0..=511).contains(&parts[2])
        || !(0..=63).contains(&parts[3])
        || !(0..=63).contains(&parts[4])
    {
        return Err(format!("invalid coords {coords}"));
    }
    let location = parse_location(parts[0], parts[1] * 64 + parts[3], parts[2] * 64 + parts[4])?;
    let settings = inherited(symbol, overrides, &mut BTreeSet::new())?;
    let restriction: MovementRestriction =
        field_enum(&settings, "moveRestrict", MovementRestriction::default())?;
    let blocking: ActorBlocking = field_enum(&settings, "blockWalk", ActorBlocking::default())?;
    let mode = settings
        .get("defaultMode")
        .and_then(toml::Value::as_str)
        .unwrap_or("Wander");
    let behavior = match mode {
        "None" => NpcBehavior::Stationary,
        "Wander" => NpcBehavior::Wander {
            radius: u16::try_from(integer(&settings, "wanderRange", 5)?)
                .map_err(|_| "invalid wanderRange")?,
        },
        "Patrol" => {
            let rows = settings
                .get("waypoints")
                .and_then(toml::Value::as_array)
                .ok_or("Patrol needs waypoints")?;
            if rows.is_empty() {
                return Err("Patrol needs nonempty waypoints".into());
            }
            let mut waypoints = Vec::new();
            for row in rows {
                let dest = row.get("destination").ok_or("waypoint needs destination")?;
                let destination = parse_location(
                    integer(dest, "level", -1)?,
                    integer(dest, "x", -1)?,
                    integer(dest, "z", -1)?,
                )?;
                let pause_ticks = u32::try_from(integer(row, "pause_delay", 0)?)
                    .map_err(|_| "invalid pause_delay")?;
                waypoints.push(PatrolWaypoint {
                    destination,
                    pause_ticks,
                });
            }
            NpcBehavior::Patrol { waypoints }
        }
        unsupported => return Err(format!("unsupported scripted NPC mode {unsupported}")),
    };
    let facing = match settings
        .get("respawnDir")
        .and_then(toml::Value::as_str)
        .unwrap_or("South")
    {
        "South" => 0,
        "SouthWest" => 256,
        "West" => 512,
        "NorthWest" => 768,
        "North" => 1024,
        "NorthEast" => 1280,
        "East" => 1536,
        "SouthEast" => 1792,
        v => return Err(format!("unknown respawnDir {v}")),
    };
    Ok(NpcSpawn {
        id,
        npc,
        symbol: symbol.to_owned(),
        location,
        facing,
        behavior,
        restriction,
        blocking,
        source,
    })
}
fn inherited(
    symbol: &str,
    overrides: &BTreeMap<String, toml::Value>,
    seen: &mut BTreeSet<String>,
) -> Result<toml::Value, String> {
    if !seen.insert(symbol.to_owned()) {
        return Err(format!("cyclic NPC inheritance {symbol}"));
    }
    let Some(row) = overrides.get(symbol) else {
        return Ok(toml::Value::Table(Default::default()));
    };
    let mut result = if let Some(parent) = row
        .get("inherit")
        .and_then(toml::Value::as_str)
        .filter(|p| *p != symbol)
    {
        inherited(parent, overrides, seen)?
    } else {
        toml::Value::Table(Default::default())
    };
    let target = result
        .as_table_mut()
        .ok_or("NPC inheritance table expected")?;
    for (key, value) in row.as_table().ok_or("NPC table expected")? {
        target.insert(key.clone(), value.clone());
    }
    Ok(result)
}
fn field_enum<T: serde::de::DeserializeOwned>(
    value: &toml::Value,
    field: &str,
    default: T,
) -> Result<T, String> {
    value
        .get(field)
        .map(|v| {
            v.clone()
                .try_into()
                .map_err(|e: toml::de::Error| format!("{field}: {e}"))
        })
        .unwrap_or(Ok(default))
}
fn integer(value: &toml::Value, field: &str, default: i32) -> Result<i32, String> {
    match value.get(field) {
        None => Ok(default),
        Some(v) => v
            .as_integer()
            .and_then(|v| i32::try_from(v).ok())
            .ok_or_else(|| format!("invalid {field}")),
    }
}
fn parse_location(plane: i32, x: i32, y: i32) -> Result<ActorLocation, String> {
    let plane = u8::try_from(plane)
        .ok()
        .and_then(PlaneIndex::new)
        .ok_or("invalid plane")?;
    if !(0..=32767).contains(&x) || !(0..=32767).contains(&y) {
        return Err("invalid actor location".into());
    }
    Ok(ActorLocation {
        tile: MapTile::new(x, y),
        plane,
    })
}
