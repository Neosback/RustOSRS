//! Write the OpenRune NPC spawns inside a tile box as a spawn-edits sidecar the web viewer can
//! fetch (`?npcs=<url>`), because the browser cannot read an OpenRune folder.
//!
//! usage: viewer_spawns <OpenRune root> <out.json> <min_x> <min_y> <max_x> <max_y>
use osrs_cache::openrune::{import_spawns, installation::Installation};
use osrs_core::scenario::{NpcBehavior, ScenarioDefinition};
use serde::Serialize;

/// Same shape as the editor's sidecar (`npc_panel::Sidecar`).
#[derive(Serialize)]
struct Sidecar {
    format: &'static str,
    scenario: ScenarioDefinition,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [root, out, min_x, min_y, max_x, max_y] = args.as_slice() else {
        return Err(
            "usage: viewer_spawns <OpenRune root> <out.json> <min_x> <min_y> <max_x> <max_y>"
                .into(),
        );
    };
    let (min_x, min_y, max_x, max_y): (i32, i32, i32, i32) = (
        min_x.parse()?,
        min_y.parse()?,
        max_x.parse()?,
        max_y.parse()?,
    );
    let installation = Installation::read(root)?;
    let report = import_spawns(
        &installation.spawn_files,
        &installation.server_files,
        &installation.symbols,
    );
    let total = report.spawns.len();
    let fingerprint = installation
        .profile(false)?
        .cache_fingerprint
        .value
        .ok_or("the installed cache has no fingerprint")?;
    let seed = u64::from_str_radix(fingerprint.get(..16).unwrap_or("0"), 16).unwrap_or(0);
    let mut scenario = report
        .into_scenario(installation.revision, fingerprint, seed)
        .map_err(|e| e.join("; "))?;
    scenario.spawns.retain(|spawn| {
        let tile = spawn.location.tile;
        (min_x..=max_x).contains(&tile.x) && (min_y..=max_y).contains(&tile.y)
    });
    // A still demo: nobody walks.
    for spawn in &mut scenario.spawns {
        spawn.behavior = NpcBehavior::Stationary;
    }
    println!("{} of {total} spawns inside the box", scenario.spawns.len());
    let sidecar = Sidecar {
        format: "rustosrs-npc-scenario",
        scenario,
    };
    std::fs::write(out, serde_json::to_vec(&sidecar)?)?;
    Ok(())
}
