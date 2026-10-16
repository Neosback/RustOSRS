//! Fixture tests: a failed setup step should abort the test, so unwrap is intentional here.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use osrs_core::{
    actor_snapshot::{ActorStatus, PlaytestIdentity},
    coords::{MapTile, PlaneIndex},
    definitions::DefinitionIdentity,
    entity::{EntityScale, NpcDefinition, NpcMovementSequences, NpcRenderFlags},
    ids::NpcId,
    provenance::{CacheFingerprint, ProfileDigest, TargetProvenance},
    scenario::{
        ActorBlocking, ActorLocation, MovementRestriction, NpcBehavior, NpcSpawn, NpcSpawnId,
        ScenarioDefinition, SpawnSource,
    },
};
use osrs_playtest::{
    navigation::{Navigation, NavigationCell},
    runtime::{MoveSpeed, PlaytestRuntime, RuntimeAssets, RuntimeCommand},
};
use std::{collections::BTreeMap, sync::Arc};
fn location(x: i32, y: i32) -> ActorLocation {
    ActorLocation {
        tile: MapTile::new(x, y),
        plane: PlaneIndex::new(0).unwrap(),
    }
}
struct Grid {
    loaded: bool,
    blocked: bool,
}
impl Navigation for Grid {
    fn cell(&self, _: ActorLocation) -> Option<NavigationCell> {
        self.loaded.then_some(NavigationCell {
            flags: if self.blocked { 256 } else { 0 },
            roof: false,
        })
    }
}
fn fixture(positions: &[(i32, i32)]) -> PlaytestRuntime {
    let fingerprint = "a".repeat(64);
    let provenance = TargetProvenance::new(
        "test",
        ProfileDigest::from_lower_hex(&"b".repeat(64)).unwrap(),
        CacheFingerprint::from_lower_hex(&fingerprint).unwrap(),
        1,
    )
    .unwrap();
    let npc = NpcDefinition {
        identity: DefinitionIdentity::new(NpcId::new(0), provenance),
        name: Some("test".into()),
        body_models: Vec::new(),
        head_models: Vec::new(),
        size: 1,
        movement_sequences: NpcMovementSequences::default(),
        recolors: Vec::new(),
        retextures: Vec::new(),
        combat_level: None,
        scale: EntityScale::IDENTITY,
        ambient: 0,
        contrast: 0,
        morphs: None,
        render: NpcRenderFlags::default(),
        actions: Default::default(),
        full_recolor: None,
    };
    let assets = Arc::new(RuntimeAssets {
        client_fingerprint: fingerprint.clone(),
        npcs: BTreeMap::from([(NpcId::new(0), Arc::new(npc))]),
        sequences: BTreeMap::new(),
    });
    let scenario = ScenarioDefinition {
        version: 1,
        build: 240,
        client_fingerprint: fingerprint,
        seed: 73,
        spawns: positions
            .iter()
            .enumerate()
            .map(|(i, &(x, y))| NpcSpawn {
                id: NpcSpawnId(i as u64 + 1),
                npc: NpcId::new(0),
                symbol: "npc.test".into(),
                location: location(x, y),
                facing: 0,
                behavior: NpcBehavior::Wander { radius: 3 },
                restriction: MovementRestriction::Normal,
                blocking: ActorBlocking::Npc,
                source: SpawnSource {
                    file: "test".into(),
                    entry: i as u32,
                },
            })
            .collect(),
    };
    PlaytestRuntime::new(scenario, assets, PlaytestIdentity::default()).unwrap()
}
#[test]
fn replay_pause_single_step_and_restart_are_deterministic() {
    let grid = Grid {
        loaded: true,
        blocked: false,
    };
    let mut a = fixture(&[(100, 100)]);
    let mut b = fixture(&[(100, 100)]);
    a.command(RuntimeCommand::Start, &grid).unwrap();
    b.command(RuntimeCommand::Start, &grid).unwrap();
    a.advance_cycles(600, &grid);
    for _ in 0..600 {
        b.advance_cycles(1, &grid);
    }
    assert_eq!(a.snapshot(), b.snapshot());
    a.command(RuntimeCommand::Pause, &grid).unwrap();
    let paused = a.snapshot();
    a.advance_cycles(500, &grid);
    assert_eq!(a.snapshot(), paused);
    a.command(RuntimeCommand::SingleStep, &grid).unwrap();
    assert_eq!(a.snapshot().stamp.client_cycle, 630);
    assert!(!a.running());
    a.command(RuntimeCommand::Restart, &grid).unwrap();
    a.command(RuntimeCommand::Start, &grid).unwrap();
    a.advance_cycles(600, &grid);
    assert_eq!(a.snapshot().actors, b.snapshot().actors);
    assert_eq!(a.snapshot().stamp.identity.run_generation, 1);
}
#[test]
fn unknown_coverage_parks_and_invalidated_map_cannot_keep_running() {
    let grid = Grid {
        loaded: true,
        blocked: false,
    };
    let mut runtime = fixture(&[(100, 100)]);
    runtime
        .command(
            RuntimeCommand::Move {
                actor: NpcSpawnId(1),
                destination: location(105, 100),
                route: false,
                speed: MoveSpeed::Walk,
            },
            &grid,
        )
        .unwrap();
    runtime.command(RuntimeCommand::Start, &grid).unwrap();
    runtime.advance_cycles(
        30,
        &Grid {
            loaded: false,
            blocked: false,
        },
    );
    assert_eq!(runtime.snapshot().actors[0].location, location(100, 100));
    assert_eq!(runtime.snapshot().actors[0].status, ActorStatus::Parked);
    runtime.advance_cycles(30, &grid);
    assert_eq!(runtime.snapshot().actors[0].location, location(101, 100));
    runtime.invalidate_map(2);
    assert!(!runtime.running());
    runtime
        .command(
            RuntimeCommand::Start,
            &Grid {
                loaded: true,
                blocked: true,
            },
        )
        .unwrap();
    runtime.advance_cycles(
        30,
        &Grid {
            loaded: true,
            blocked: true,
        },
    );
    assert_eq!(
        runtime.snapshot().actors[0].status,
        ActorStatus::InvalidPlacement
    );
    assert_eq!(runtime.snapshot().stamp.identity.map_revision, 2);
}
#[test]
fn moving_one_overlapped_actor_does_not_clear_the_other_occupant() {
    let grid = Grid {
        loaded: true,
        blocked: false,
    };
    let mut runtime = fixture(&[(100, 100), (100, 100), (99, 100)]);
    for (actor, x) in [(1, 101), (2, 100), (3, 100)] {
        runtime
            .command(
                RuntimeCommand::Move {
                    actor: NpcSpawnId(actor),
                    destination: location(x, 100),
                    route: false,
                    speed: MoveSpeed::Walk,
                },
                &grid,
            )
            .unwrap();
    }
    runtime.command(RuntimeCommand::SingleStep, &grid).unwrap();
    let snapshot = runtime.snapshot();
    assert_eq!(snapshot.actors[0].location, location(101, 100));
    assert_eq!(snapshot.actors[1].location, location(100, 100));
    assert_eq!(snapshot.actors[2].location, location(99, 100));
    assert_eq!(snapshot.actors[2].status, ActorStatus::Blocked);
}
