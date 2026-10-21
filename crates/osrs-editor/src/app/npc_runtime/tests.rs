use super::*;

#[test]
fn results_are_current_only_for_the_exact_identity() {
    let current = PlaytestIdentity {
        source_generation: 2,
        map_revision: 5,
        scenario_revision: 3,
        run_generation: 1,
    };
    assert!(identity_current(current, current));
    for stale in [
        PlaytestIdentity {
            source_generation: 1,
            ..current
        },
        PlaytestIdentity {
            map_revision: 4,
            ..current
        },
        PlaytestIdentity {
            scenario_revision: 2,
            ..current
        },
        PlaytestIdentity {
            run_generation: 0,
            ..current
        },
    ] {
        assert!(!identity_current(stale, current));
    }
}

#[test]
fn diagnostics_deduplicate_and_stay_bounded() {
    let mut runtime = NpcRuntime::default();
    for _ in 0..3 {
        runtime.report("same".into());
    }
    assert_eq!(runtime.diagnostics(), ["same".to_string()]);
    for n in 0..(MAX_DIAGNOSTICS * 2) {
        runtime.report(format!("message {n}"));
    }
    assert_eq!(runtime.diagnostics().len(), MAX_DIAGNOSTICS);
    assert_eq!(
        runtime.diagnostics().last().map(String::as_str),
        Some("message 127")
    );
}

fn result(stamp: PlaytestStamp) -> NpcGeometryUpdate {
    NpcGeometryUpdate {
        stamp,
        context: crate::app::world_runtime::test_support::actor_context(),
        geometry: crate::app::world_runtime::test_support::candidate().geometry,
        errors: vec!["obsolete".into()],
    }
}

#[test]
fn old_npc_stamps_cannot_retire_a_newer_inflight_slot() {
    let current = PlaytestStamp {
        identity: PlaytestIdentity::default(),
        server_tick: 2,
        client_cycle: 60,
    };
    let mut npc = NpcRuntime {
        geometry_inflight: Some(current),
        ..Default::default()
    };
    for old in [
        PlaytestStamp {
            server_tick: 1,
            ..current
        },
        PlaytestStamp {
            client_cycle: 59,
            ..current
        },
        PlaytestStamp {
            identity: PlaytestIdentity {
                source_generation: 1,
                ..current.identity
            },
            ..current
        },
    ] {
        npc.geometry_arrived(result(old));
        assert_eq!(npc.geometry_inflight, Some(current));
        assert!(npc.geometry_pending.is_none());
    }
    npc.geometry_arrived(result(current));
    assert_eq!(npc.geometry_inflight, None);
    assert!(
        npc.geometry_pending.is_none(),
        "hidden request drains without publication"
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn gpu_npc_publication_reconciles_current_frame_and_allows_simulation_lag() {
    use crate::app::world_runtime::test_support::*;
    use osrs_cache::decode::{ArchiveFileProvenance, DecoderContext, decode_npc_definition};
    use osrs_core::coords::{MapTile, SourcePlane};
    use osrs_core::{
        coords::PlaneIndex,
        scenario::{
            ActorBlocking, ActorLocation, MovementRestriction, NpcBehavior, NpcSpawn, SpawnSource,
        },
    };
    use osrs_edit::{MapOp, MapPatch, MapPosition, TerrainTile, plan_replace_scenario};
    let Some(mut renderer) = actor_renderer() else {
        return;
    };
    let profiles =
        osrs_cache::registry::bundled_profiles().unwrap_or_else(|error| panic!("{error:?}"));
    let context =
        DecoderContext::from_profile(profiles.first().unwrap_or_else(|| panic!("profile")))
            .unwrap_or_else(|error| panic!("{error:?}"));
    let fingerprint = context.target_provenance().cache_fingerprint().to_string();
    let npc_definition = Arc::new(
        decode_npc_definition(
            NpcId::new(1),
            &[0],
            &context,
            &ArchiveFileProvenance::new(2, 9, Some(1)),
        )
        .unwrap_or_else(|error| panic!("{error:?}")),
    );
    for change in [
        "hide",
        "hidden_arrival",
        "models",
        "remove",
        "scenario",
        "map",
        "restart",
        "ground",
        "camera",
        "range",
        "limit",
        "varps",
        "source",
        "presentation",
        "valid_lag",
        "empty",
    ] {
        let mut world = world();
        seed_installed(&mut world, &candidate());
        let mut session = EditRuntime::default();
        let definition = ScenarioDefinition {
            version: 1,
            build: 241,
            client_fingerprint: fingerprint.clone(),
            seed: 1,
            spawns: vec![NpcSpawn {
                id: NpcSpawnId(1),
                npc: NpcId::new(1),
                symbol: "test.npc".into(),
                location: ActorLocation {
                    tile: MapTile::new(3200, 3200),
                    plane: PlaneIndex::new(0).unwrap_or_else(|| panic!("plane")),
                },
                facing: 0,
                behavior: NpcBehavior::Stationary,
                restriction: MovementRestriction::Normal,
                blocking: ActorBlocking::Npc,
                source: SpawnSource {
                    file: "fixture".into(),
                    entry: 1,
                },
            }],
        };
        let patch = plan_replace_scenario(session.scenario(), Some(definition.clone()))
            .unwrap_or_else(|error| panic!("{error:?}"));
        session
            .execute_scenario(patch, |_| Ok::<_, ()>(()))
            .unwrap_or_else(|error| panic!("{error:?}"));
        let mut npc = NpcRuntime::default();
        npc.source_opened(fingerprint.clone(), 241, &world, &mut renderer);
        npc.assets = Some(Arc::new(RuntimeAssets {
            client_fingerprint: fingerprint.clone(),
            npcs: BTreeMap::from([(NpcId::new(1), Arc::clone(&npc_definition))]),
            ..Default::default()
        }));
        let display = NpcDisplay {
            enabled: true,
            models: true,
            distance_tiles: 50,
            max_drawn: 100,
        };
        let mut display = display;
        {
            let mut host = NpcHost {
                session: &session,
                world: &world,
                renderer: &mut renderer,
                dt: 0.0,
                camera: (3200, 3200),
                display,
                varps: BTreeMap::new(),
            };
            npc.update(&mut host);
            let stamp = npc
                .geometry_inflight
                .unwrap_or_else(|| panic!("initial request: {change}"));
            if change == "hidden_arrival" {
                host.display.enabled = false;
                npc.update(&mut host);
                assert_eq!(
                    npc.geometry_inflight,
                    Some(stamp),
                    "hidden physical request must drain"
                );
            }
            npc.geometry_arrived(result(stamp));
            assert_eq!(host.renderer.resident_vertex_bytes(), 0, "staged: {change}");
            assert!(host.renderer.npc_identity().is_none());
        }
        let issued = npc.geometry_sent.as_ref().map(Arc::downgrade);
        let mut camera = (3200, 3200);
        let mut varps = BTreeMap::new();
        match change {
            "hide" => display.enabled = false,
            "models" => display.models = false,
            "remove" | "scenario" => {
                let mut definition = definition;
                definition.seed = 2;
                let patch = plan_replace_scenario(
                    session.scenario(),
                    (change != "remove").then_some(definition),
                )
                .unwrap_or_else(|error| panic!("{error:?}"));
                session
                    .execute_scenario(patch, |_| Ok::<_, ()>(()))
                    .unwrap_or_else(|error| panic!("{error:?}"));
            }
            "map" => {
                session
                    .commit(MapPatch::new([MapOp::SetTerrain {
                        coord: MapPosition::new(
                            MapTile::new(3200, 3200),
                            SourcePlane::new(0).unwrap_or_else(|| panic!("plane")),
                        ),
                        before: TerrainTile::default(),
                        after: TerrainTile {
                            underlay: Some(osrs_core::ids::FloorUnderlayId::new(2)),
                            ..Default::default()
                        },
                    }]))
                    .unwrap_or_else(|error| panic!("{error:?}"));
            }
            "restart" => npc.queue(RuntimeCommand::Restart),
            "ground" => seed_info(&mut world, &candidate()),
            "camera" => camera.0 += 1,
            "range" => display.distance_tiles = 10,
            "limit" => display.max_drawn = 1,
            "varps" => {
                varps.insert(VarpId::new(1), 2);
            }
            "source" => npc.source_opened(fingerprint.clone(), 241, &world, &mut renderer),
            "presentation" => {
                world.begin_rebuild_all(false, true);
            }
            _ => {}
        }
        let before_map = session.map_revision();
        let before_scenario = session.scenario().clone();
        let mut host = NpcHost {
            session: &session,
            world: &world,
            renderer: &mut renderer,
            dt: 0.2,
            camera,
            display,
            varps,
        };
        npc.update(&mut host);
        if change == "valid_lag" || change == "empty" {
            assert!(host.renderer.resident_vertex_bytes() > 0);
            assert_eq!(npc.diagnostics(), ["obsolete".to_string()]);
            assert!(
                npc.geometry_inflight.is_some(),
                "newer simulation frame scheduled"
            );
            let issued = issued.unwrap_or_else(|| panic!("issued input"));
            let request = issued
                .upgrade()
                .unwrap_or_else(|| panic!("GPU retains issued input"));
            let lines = host
                .renderer
                .npc_identity()
                .unwrap_or_else(|| panic!("GPU identity"))
                .describe(MapTile::new(3200, 3200));
            assert!(lines[0].contains(&format!(
                "tick {}; cycle {}",
                request.key.stamp.server_tick, request.key.stamp.client_cycle
            )));
            assert!(lines[0].contains(&format!("ground version {}", request.key.infos_version)));
            assert!(lines[1].contains("1 selected actors"));
            assert!(lines.iter().any(|line| {
                line == &host
                    .world
                    .info_identity(MapTile::new(3200, 3200))
                    .unwrap_or_else(|| panic!("ground input"))
                    .describe()[0]
            }));
            drop(request);
            if change == "empty" {
                let mut empty = result(
                    npc.geometry_inflight
                        .unwrap_or_else(|| panic!("next stamp")),
                );
                empty.geometry = SceneGeometry::default();
                npc.geometry_arrived(empty);
                host.dt = 0.0;
                npc.update(&mut host);
                assert!(host.renderer.npc_identity().is_none());
                assert_eq!(host.renderer.resident_vertex_bytes(), 0);
                assert!(
                    issued.upgrade().is_none(),
                    "empty upload releases old request"
                );
            }
            host.display.enabled = false;
            npc.update(&mut host);
            assert_eq!(
                host.renderer.resident_vertex_bytes(),
                0,
                "hides installed layer"
            );
            assert!(host.renderer.npc_identity().is_none());
            assert!(
                issued.upgrade().is_none(),
                "hide releases installed request"
            );
        } else {
            assert_eq!(
                host.renderer.resident_vertex_bytes(),
                0,
                "rejected: {change}"
            );
            assert!(npc.diagnostics().is_empty(), "stale diagnostics: {change}");
            assert!(
                host.renderer.npc_identity().is_none(),
                "stale identity: {change}"
            );
            if change == "hidden_arrival"
                || change == "restart"
                || change == "scenario"
                || change == "map"
                || change == "presentation"
            {
                let stamp = npc
                    .geometry_inflight
                    .unwrap_or_else(|| panic!("replacement request"));
                let mut update = result(stamp);
                update.context.presentation_generation = host.world.presentation_generation();
                npc.geometry_arrived(update);
                host.dt = 0.0;
                npc.update(&mut host);
                assert!(
                    host.renderer.resident_vertex_bytes() > 0,
                    "valid replacement: {change}"
                );
                assert!(host.renderer.npc_identity().is_some());
            }
        }
        assert_eq!(session.map_revision(), before_map);
        assert_eq!(*session.scenario(), before_scenario);
    }
    eprintln!("GPU NPC projection lifecycle exercised");
}
