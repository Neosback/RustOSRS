//! Fixture tests: a failed setup step should abort the test, so unwrap is intentional here.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use osrs_cache::{
    decode::{ArchiveFileProvenance, DecodeErrorKind, DecoderContext, decode_npc_definition},
    openrune::{GamevalSymbols, SourceText, export_scenario, import_spawns},
};
use osrs_core::{ids::NpcId, scenario::NpcBehavior};
fn symbols() -> GamevalSymbols {
    let mut symbols = GamevalSymbols::default();
    symbols.declare("npc", "hans", 0).unwrap();
    symbols
}
fn source(path: &str, text: &str) -> SourceText {
    SourceText {
        path: path.into(),
        text: text.into(),
    }
}
#[test]
fn npc_decode_rejects_every_truncation_unknown_opcode_and_trailing_bytes() {
    let profile = osrs_cache::registry::bundled_profiles().unwrap().remove(0);
    let context = DecoderContext::from_profile(&profile).unwrap();
    let source = ArchiveFileProvenance::new(2, 9, Some(5));
    let bytes = [
        1, 1, 0, 42, 2, b'H', b'i', 0, 12, 2, 17, 0, 1, 0, 2, 0, 3, 0, 4, 40, 1, 0, 5, 0, 6, 101,
        255, 0,
    ];
    let npc = decode_npc_definition(NpcId::new(5), &bytes, &context, &source).unwrap();
    assert_eq!(npc.size, 2);
    assert_eq!(npc.body_models[0].get(), 42);
    assert_eq!(npc.contrast, -5);
    assert_eq!(npc.recolors[0].to, 6);
    for end in 0..bytes.len() {
        assert!(
            decode_npc_definition(NpcId::new(5), &bytes[..end], &context, &source).is_err(),
            "truncation at {end}"
        );
    }
    let error = decode_npc_definition(NpcId::new(5), &[250, 0], &context, &source).unwrap_err();
    assert!(matches!(
        error.kind(),
        DecodeErrorKind::UnsupportedOpcode { opcode: 250, .. }
    ));
    assert!(decode_npc_definition(NpcId::new(5), &[0, 0], &context, &source).is_err());
}
#[test]
fn patrol_duplicates_and_last_override_survive_export_and_reimport() {
    let spawns = vec![source(
        "a.toml",
        "[[spawn]]\nnpc='npc.hans'\ncoords='0_50_50_7_33'\n[[spawn]]\nnpc='npc.hans'\ncoords='0_50_50_7_33'\n",
    )];
    let server = vec![source(
        "server.toml",
        "[[npc]]\nid='npc.hans'\nwanderRange=7\n[[npc]]\nid='npc.hans'\ndefaultMode='Patrol'\n[[npc.waypoints]]\ndestination={x=3207,z=3233,level=0}\npause_delay=10\n",
    )];
    let report = import_spawns(&spawns, &server, &symbols());
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    assert_eq!(report.warnings.len(), 1);
    let scenario = report.into_scenario(240, "a".repeat(64), 17).unwrap();
    assert_eq!(scenario.spawns.len(), 2);
    assert_ne!(scenario.spawns[0].id, scenario.spawns[1].id);
    assert_eq!(scenario.spawns[0].location.tile.x, 3207);
    let NpcBehavior::Patrol { waypoints } = &scenario.spawns[0].behavior else {
        panic!("expected patrol")
    };
    assert_eq!(waypoints[0].pause_ticks, 10);
    let exported = export_scenario(&scenario).unwrap();
    let report = import_spawns(
        &[source("export.toml", &exported.spawns_toml)],
        &[source("behaviors.toml", &exported.behaviors_toml)],
        &symbols(),
    );
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    assert_eq!(report.spawns[0].behavior, scenario.spawns[0].behavior);
    let mut conflicting = scenario.clone();
    conflicting.spawns[1].facing = 512;
    assert!(export_scenario(&conflicting).is_err());
}
#[test]
fn unresolved_names_invalid_coords_and_bad_server_files_block_installation() {
    let report = import_spawns(
        &[source(
            "a.toml",
            "[[spawn]]\nnpc='npc.missing'\ncoords='0_50_50_7_33'\n",
        )],
        &[source("bad.toml", "[[npc")],
        &symbols(),
    );
    assert_eq!(report.errors.len(), 2);
    assert!(report.into_scenario(240, "a".repeat(64), 0).is_err());
    let report = import_spawns(
        &[source(
            "a.toml",
            "[[spawn]]\nnpc='npc.hans'\ncoords='0_50_50_64_33'\n",
        )],
        &[],
        &symbols(),
    );
    assert_eq!(report.errors.len(), 1);
}
#[test]
fn gameval_binary_is_bounded_and_custom_assignments_are_atomic() {
    fn string(out: &mut Vec<u8>, text: &str) {
        out.extend((text.len() as i16).to_be_bytes());
        out.extend(text.as_bytes());
    }
    let mut bytes = Vec::new();
    bytes.extend(1_i32.to_be_bytes());
    string(&mut bytes, "npc");
    bytes.extend(1_i32.to_be_bytes());
    string(&mut bytes, "hans=0");
    let mut symbols = GamevalSymbols::default();
    symbols.load_dat(&bytes, true).unwrap();
    assert!(symbols.declare("npc", "other", 0).is_err());
    assert!(symbols.load_rscm("npc", "custom=1\nother=1").is_err());
    assert!(symbols.resolve("npc.custom").is_err());
    for end in 0..bytes.len() {
        let mut symbols = GamevalSymbols::default();
        assert!(symbols.load_dat(&bytes[..end], true).is_err());
    }
    bytes.push(0);
    assert!(symbols.load_dat(&bytes, true).is_err());
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "requires OPENRUNE_ROOT installed assets"]
fn installed_openrune_spawns_resolve_and_all_client_npcs_decode() {
    use osrs_cache::{openrune::installation::Installation, transport::CacheRepository};
    let installation =
        Installation::read(std::env::var("OPENRUNE_ROOT").expect("set OPENRUNE_ROOT")).unwrap();
    let report = import_spawns(
        &installation.spawn_files,
        &installation.server_files,
        &installation.symbols,
    );
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    assert!(report.spawns.len() > 25000);
    let profile = installation.profile(false).unwrap();
    assert_eq!(profile.cache_source.build, installation.revision);
    let context = DecoderContext::from_profile(&profile).unwrap();
    let repo = CacheRepository::open(installation.root.join(".data/cache/LIVE"), &profile).unwrap();
    let mut ids = std::collections::BTreeSet::new();
    for (id, file) in repo.read_group_files(2, 9).unwrap() {
        decode_npc_definition(NpcId::new(id), &file.bytes, &context, &file.provenance).unwrap();
        ids.insert(id);
    }
    assert!(
        report
            .spawns
            .iter()
            .all(|spawn| ids.contains(&spawn.npc.get()))
    );
}
