//! Audit installed NPC assets without modifying OpenRune or a RustOSRS project.
use osrs_cache::{
    decode::{DecoderContext, decode_npc_definition},
    openrune::{import_spawns, installation::Installation},
    transport::CacheRepository,
};
use osrs_core::ids::NpcId;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args()
        .nth(1)
        .ok_or("usage: openrune_npcs <installed OpenRune root>")?;
    let installation = Installation::read(&root)?;
    println!(
        "manifest_revision={} spawn_files={} server_files={}",
        installation.revision,
        installation.spawn_files.len(),
        installation.server_files.len()
    );
    let report = import_spawns(
        &installation.spawn_files,
        &installation.server_files,
        &installation.symbols,
    );
    println!(
        "spawns={} import_errors={}",
        report.spawns.len(),
        report.errors.len()
    );
    for warning in &report.warnings {
        eprintln!("warning: {warning}");
    }
    for error in &report.errors {
        eprintln!("{error}");
    }
    let profile = installation.profile(false)?;
    if let Some(path) = std::env::args().nth(2) {
        std::fs::write(path, serde_yaml_ng::to_string(&profile)?)?;
    }
    println!(
        "client_profile={} fingerprint={}",
        profile.id,
        profile
            .cache_fingerprint
            .value
            .as_deref()
            .unwrap_or("missing")
    );
    let context = DecoderContext::from_profile(&profile)?;
    let repository = CacheRepository::open(installation.root.join(".data/cache/LIVE"), &profile)?;
    let files = repository.read_group_files(2, 9)?;
    let mut definitions = std::collections::BTreeMap::new();
    let mut failures = 0;
    for (id, file) in files {
        match decode_npc_definition(NpcId::new(id), &file.bytes, &context, &file.provenance) {
            Ok(npc) => {
                definitions.insert(id, npc);
            }
            Err(e) => {
                failures += 1;
                if failures <= 20 {
                    eprintln!("{e}");
                }
            }
        }
    }
    let missing: std::collections::BTreeSet<_> = report
        .spawns
        .iter()
        .filter(|s| !definitions.contains_key(&s.npc.get()))
        .map(|s| s.symbol.clone())
        .collect();
    println!(
        "decoded_npcs={} decode_errors={failures} missing_spawn_definitions={}",
        definitions.len(),
        missing.len()
    );
    for symbol in missing {
        eprintln!("missing client NPC: {symbol}");
    }
    match installation.profile(true) {
        Ok(server) => println!(
            "server_profile={} fingerprint={}",
            server.id,
            server
                .cache_fingerprint
                .value
                .as_deref()
                .unwrap_or("missing")
        ),
        Err(error) => eprintln!(
            "SERVER cache failed full validation: {error}; behavior import uses source TOMLs"
        ),
    }
    if failures > 0 || !report.errors.is_empty() {
        return Err("NPC audit failed; see diagnostics".into());
    }
    Ok(())
}
