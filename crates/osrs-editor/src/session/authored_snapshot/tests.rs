use super::*;
use osrs_core::{
    coords::{MapTile, PlaneIndex, SourcePlane},
    ids::{FloorUnderlayId, NpcId, ObjectId},
    scenario::{
        ActorBlocking, ActorLocation, MovementRestriction, NpcBehavior, NpcSpawn, NpcSpawnId,
        PatrolWaypoint, SpawnSource,
    },
};
use osrs_edit::{
    HeightVertexCoord, LocKind, MapOp, MapPatch, MapPosition, ObjectOrientation, ObjectPlacement,
    TerrainTile,
};
use osrs_hd::theme::{ThemeAsset, ThemeNumber, ThemeOverrides, ThemePatch, ThemeRule, ThemeTarget};

fn plane() -> SourcePlane {
    SourcePlane::new(3).unwrap_or_else(|| unreachable!())
}
fn vertex() -> HeightVertexCoord {
    HeightVertexCoord::new(MapTile::new(i32::MIN, i32::MAX), plane())
}
fn location() -> ActorLocation {
    ActorLocation {
        tile: MapTile::new(0, 32767),
        plane: PlaneIndex::new(3).unwrap_or_else(|| unreachable!()),
    }
}
pub(crate) fn fixture() -> EditorSession {
    let document = MapDocument::from_parts(
        [(
            MapPosition::new(MapTile::new(-65, 128), plane()),
            TerrainTile {
                underlay: Some(FloorUnderlayId::new(u32::MAX)),
                flags: 255,
                ..Default::default()
            },
        )],
        [(vertex(), i32::MIN)],
        [ObjectPlacement {
            object_id: ObjectId::new(u32::MAX),
            position: MapPosition::new(MapTile::new(-1, -64), plane()),
            kind: LocKind::new(22).unwrap_or_else(|| unreachable!()),
            orientation: ObjectOrientation::new(3).unwrap_or_else(|| unreachable!()),
        }],
    );
    let mut session = EditorSession::new(document);
    let spawns = [
        NpcBehavior::Stationary,
        NpcBehavior::Wander { radius: u16::MAX },
        NpcBehavior::Patrol {
            waypoints: vec![PatrolWaypoint {
                destination: location(),
                pause_ticks: u32::MAX,
            }],
        },
    ]
    .into_iter()
    .enumerate()
    .map(|(index, behavior)| NpcSpawn {
        id: NpcSpawnId(u64::MAX - index as u64),
        npc: NpcId::new(u32::MAX),
        symbol: format!("npc.écho{index}"),
        location: location(),
        facing: 2047,
        behavior,
        restriction: MovementRestriction::PassThru,
        blocking: ActorBlocking::All,
        source: SpawnSource {
            file: "authored/源.json".into(),
            entry: u32::MAX,
        },
    })
    .collect();
    let definition = ScenarioDefinition {
        version: 1,
        build: 241,
        client_fingerprint: "ab".repeat(32),
        seed: u64::MAX,
        spawns,
    };
    let patch = osrs_edit::plan_replace_scenario(session.scenario(), Some(definition))
        .unwrap_or_else(|e| panic!("{e}"));
    let (_, checkpoint) = session
        .commit_scenario_checkpointed(patch)
        .unwrap_or_else(|e| panic!("{e:?}"))
        .unwrap_or_else(|| panic!("commit"));
    session
        .finalize_checkpoint(checkpoint)
        .unwrap_or_else(|e| panic!("{e:?}"));
    let mut appearance = session.appearance().clone();
    appearance.definition.name = "Dusk écho".into();
    appearance.definition.provenance = vec!["one".into(), "two".into()];
    appearance.definition.overrides.exposure =
        Some(ThemeNumber::new(1.25).unwrap_or_else(|e| panic!("{e}")));
    appearance.definition.rules = ["second", "first"]
        .map(|id| ThemeRule {
            id: id.into(),
            name: id.into(),
            enabled: true,
            target: ThemeTarget::Global,
            values: ThemeOverrides {
                fog_color: Some([255, 0, 17]),
                ..Default::default()
            },
        })
        .into();
    appearance.assets.insert(
        "evidence/raw.bin".into(),
        ThemeAsset::new(vec![0, 1, 127, 128, 254, 255]),
    );
    let patch =
        ThemePatch::plan(session.appearance(), appearance).unwrap_or_else(|e| panic!("{e}"));
    let (_, checkpoint) = session
        .commit_appearance_checkpointed(patch)
        .unwrap_or_else(|e| panic!("{e:?}"))
        .unwrap_or_else(|| panic!("commit"));
    session
        .finalize_checkpoint(checkpoint)
        .unwrap_or_else(|e| panic!("{e:?}"));
    session
}
fn encode(session: &EditorSession) -> String {
    session
        .freeze_authored_snapshot()
        .encode()
        .unwrap_or_else(|e| panic!("{e}"))
}
fn record(text: &str) -> Record {
    serde_json::from_str(text).unwrap_or_else(|e| panic!("{e}"))
}
fn altered(text: &str, edit: impl FnOnce(&mut Record)) -> String {
    let mut value = record(text);
    edit(&mut value);
    serde_json::to_string(&value).unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn authored_snapshot_round_trip_is_lossless_for_all_domains_and_does_not_copy_history_or_observations()
 {
    let session = fixture();
    let before = session.clone();
    let text = encode(&session);
    if let Some(path) = std::env::var_os("RUSTOSRS_AUTHORED_SNAPSHOT_FIXTURE") {
        std::fs::write(path, &text).unwrap_or_else(|e| panic!("{e}"));
    }
    let restored =
        EditorSession::from_authored_snapshot_json_v1(&text).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(restored.document(), session.document());
    assert_eq!(restored.scenario(), session.scenario());
    assert_eq!(restored.appearance(), session.appearance());
    assert_eq!(restored.revision(), 0);
    assert_eq!(restored.edit_revision(), 0);
    assert_eq!(restored.map_revision(), 0);
    assert_eq!(restored.scenario().revision(), 0);
    assert!(!restored.can_undo() && !restored.can_redo());
    assert_eq!(restored.available_regions().count(), 0);
    assert_eq!(restored.dirty_regions().count(), 0);
    assert_eq!(session, before);
    assert!(session.can_undo());
    let report = verify(&text).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        (report.object_count, report.spawn_count, report.asset_count),
        (1, 3, 1)
    );
}

#[test]
fn independent_python_vectors_pin_empty_authored_identity_and_absent_scenario() {
    // Independent Python hashlib + hand-authored stock ThemePack manifest, using the published grammar.
    let id = EditorSession::default()
        .authored_identity_v1()
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        id.map,
        "cdbefee4f3b86a8c36b1ba25a013cf41296e0ecb209a43d52976f10b0febb8d0"
    );
    assert_eq!(
        id.scenario,
        "4f2c15a490a6577f6dde5d41f82db68ea7f8e0a68f82f963d7cf30e018f3608c"
    );
    assert_eq!(
        id.appearance,
        "2c812c28347924ad9facfd7ffe77d497954a922b4bebb69221eae94934c655fe"
    );
    assert_eq!(
        id.authored,
        "7589b196d77acca3aef86c799935bc033d28484c93891261434f443e25f0c25c"
    );
    assert_eq!(
        fixture()
            .authored_identity_v1()
            .unwrap_or_else(|e| panic!("{e}"))
            .scenario,
        "cfd3f3f3a468e07e1d960b13fc33b2f346bde0ff4af8d55bb27ca46139872abf"
    );
}

#[test]
fn frozen_content_survives_later_edits_and_identity_ignores_revisions_dirty_sets_and_history() {
    let mut session = fixture();
    let initial = encode(&session);
    let frozen = session.freeze_authored_snapshot();
    let patch = MapPatch::new([MapOp::SetHeight {
        coord: vertex(),
        before: i32::MIN,
        after: i32::MAX,
    }]);
    session.commit(patch).unwrap_or_else(|e| panic!("{e:?}"));
    assert_ne!(encode(&session), initial);
    assert_eq!(frozen.encode().unwrap_or_else(|e| panic!("{e}")), initial);
    session.undo().unwrap_or_else(|e| panic!("{e:?}"));
    assert!(session.can_redo());
    assert_eq!(session.dirty_regions().count(), 1);
    assert!(session.revision() > 2 && session.map_revision() > 0);
    assert_eq!(encode(&session), initial);
}

#[test]
fn absent_and_present_empty_scenarios_and_ordered_authored_data_keep_distinct_identities() {
    let empty = EditorSession::default()
        .authored_identity_v1()
        .unwrap_or_else(|e| panic!("{e}"));
    let mut session = fixture();
    let mut definition = session
        .scenario()
        .definition()
        .cloned()
        .unwrap_or_else(|| panic!("scenario"));
    definition.spawns.clear();
    let patch = osrs_edit::plan_replace_scenario(session.scenario(), Some(definition))
        .unwrap_or_else(|e| panic!("{e}"));
    let (_, c) = session
        .commit_scenario_checkpointed(patch)
        .unwrap_or_else(|e| panic!("{e:?}"))
        .unwrap_or_else(|| panic!("commit"));
    session
        .finalize_checkpoint(c)
        .unwrap_or_else(|e| panic!("{e:?}"));
    assert_ne!(
        session
            .authored_identity_v1()
            .unwrap_or_else(|e| panic!("{e}"))
            .scenario,
        empty.scenario
    );
    let original = fixture();
    let text = encode(&original);
    let reordered = altered(&text, |r| {
        let mut definition: Option<ScenarioDefinition> =
            serde_json::from_str(&r.scenario_json).unwrap_or_else(|e| panic!("{e}"));
        definition
            .as_mut()
            .unwrap_or_else(|| panic!("scenario"))
            .spawns
            .reverse();
        r.scenario_json = serde_json::to_string(&definition).unwrap_or_else(|e| panic!("{e}"));
    });
    assert!(matches!(
        decode(&reordered),
        Err(Error::Mismatch {
            domain: "scenario",
            ..
        })
    ));
    let changed = altered(&text, |r| {
        let mut pack = ThemePack::from_zip(&r.appearance_zip).unwrap_or_else(|e| panic!("{e}"));
        pack.definition.rules.reverse();
        r.appearance_zip = pack.to_zip().unwrap_or_else(|e| panic!("{e}"));
    });
    assert!(matches!(
        decode(&changed),
        Err(Error::Mismatch {
            domain: "appearance",
            ..
        })
    ));
}

#[test]
fn every_declared_identity_and_each_payload_domain_detects_changed_content() {
    let text = encode(&fixture());
    for domain in ["map", "scenario", "appearance", "authored"] {
        let changed = altered(&text, |r| {
            let value = match domain {
                "map" => &mut r.identity.map,
                "scenario" => &mut r.identity.scenario,
                "appearance" => &mut r.identity.appearance,
                _ => &mut r.identity.authored,
            };
            *value = "0".repeat(64);
        });
        assert!(
            matches!(decode(&changed), Err(Error::Mismatch { domain: actual, .. }) if actual == domain)
        );
    }
    let changed = altered(&text, |r| {
        r.map_json = MapDocument::default()
            .to_snapshot_json_v1()
            .unwrap_or_else(|e| panic!("{e}"))
    });
    assert!(matches!(
        decode(&changed),
        Err(Error::Mismatch { domain: "map", .. })
    ));
    let changed = altered(&text, |r| r.scenario_json = "null".into());
    assert!(matches!(
        decode(&changed),
        Err(Error::Mismatch {
            domain: "scenario",
            ..
        })
    ));
    let changed = altered(&text, |r| r.appearance_zip[0] ^= 255);
    assert!(matches!(decode(&changed), Err(Error::Appearance(_))));
    let changed = altered(&text, |r| {
        let mut pack = ThemePack::from_zip(&r.appearance_zip).unwrap_or_else(|e| panic!("{e}"));
        pack.assets
            .insert("evidence/raw.bin".into(), ThemeAsset::new(vec![1, 2, 3]));
        r.appearance_zip = pack.to_zip().unwrap_or_else(|e| panic!("{e}"));
    });
    assert!(matches!(
        decode(&changed),
        Err(Error::Mismatch {
            domain: "appearance",
            ..
        })
    ));
}

#[test]
fn closed_wrapper_and_canonical_decoders_reject_missing_duplicate_invalid_and_unsupported_data() {
    let text = encode(&fixture());
    for name in ["scenario_json", "appearance_zip", "identity", "map_json"] {
        let mut value: serde_json::Value =
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("{e}"));
        value
            .as_object_mut()
            .unwrap_or_else(|| panic!("record"))
            .remove(name);
        assert!(matches!(decode(&value.to_string()), Err(Error::Schema(_))));
    }
    let duplicate = text.replacen(
        "\"format\":",
        "\"format\": \"rustosrs-authored-snapshot-v1\", \"format\":",
        1,
    );
    assert!(matches!(decode(&duplicate), Err(Error::Schema(_))));
    let unknown = text.replacen('{', "{\"unknown\":0,", 1);
    assert!(matches!(decode(&unknown), Err(Error::Schema(_))));
    for header in ["format", "algorithm"] {
        let changed = altered(&text, |r| {
            if header == "format" {
                r.format = "unknown".into();
            } else {
                r.algorithm = "unknown".into();
            }
        });
        assert!(matches!(decode(&changed), Err(Error::Format(_))));
    }
    let changed = altered(&text, |r| r.identity.authored = "A".repeat(64));
    assert!(matches!(decode(&changed), Err(Error::Digest(_))));
    let changed = altered(&text, |r| {
        r.scenario_json = r
            .scenario_json
            .replacen("\"version\":", "\"version\":1,\"version\":", 1)
    });
    assert!(matches!(decode(&changed), Err(Error::Schema(_))));
    let changed = altered(&text, |r| {
        r.map_json = r
            .map_json
            .replacen("\"format\":", "\"format\":\"duplicate\",\"format\":", 1)
    });
    assert!(matches!(decode(&changed), Err(Error::Map(_))));
}

#[test]
fn canonical_scenario_validation_and_editor_appearance_admission_remain_mandatory() {
    let text = encode(&fixture());
    for field in ["duplicate", "facing", "location", "patrol", "build"] {
        let changed = altered(&text, |r| {
            let mut definition: Option<ScenarioDefinition> =
                serde_json::from_str(&r.scenario_json).unwrap_or_else(|e| panic!("{e}"));
            let d = definition.as_mut().unwrap_or_else(|| panic!("scenario"));
            match field {
                "duplicate" => d.spawns[1].id = d.spawns[0].id,
                "facing" => d.spawns[0].facing = 2048,
                "location" => d.spawns[0].location.tile.x = -1,
                "patrol" => d.spawns[0].behavior = NpcBehavior::Patrol { waypoints: vec![] },
                _ => d.build = 1,
            }
            r.scenario_json = serde_json::to_string(&definition).unwrap_or_else(|e| panic!("{e}"));
        });
        assert!(
            matches!(decode(&changed), Err(Error::Scenario(_))),
            "{field}"
        );
    }
    let changed = altered(&text, |r| {
        let mut pack = ThemePack::from_zip(&r.appearance_zip).unwrap_or_else(|e| panic!("{e}"));
        pack.definition.rules[0].target = ThemeTarget::Object(ObjectId::new(1));
        r.appearance_zip = pack.to_zip().unwrap_or_else(|e| panic!("{e}"));
    });
    assert!(matches!(decode(&changed), Err(Error::Appearance(_))));
}

#[test]
fn native_authored_snapshot_checker_rejects_wrong_usage_directories_and_oversized_files() {
    let dir = crate::export_flow::test_support::temp("authored-snapshot-cli");
    assert!(native::run_cli(&[]).is_err());
    assert!(native::run_cli(&[dir.display().to_string()]).is_err());
    let file = dir.join("oversized.json");
    std::fs::File::create(&file)
        .unwrap_or_else(|e| panic!("{e}"))
        .set_len(MAX_BYTES as u64 + 1)
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(native::run_cli(&[file.display().to_string()]).is_err());
    std::fs::remove_file(&file).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(&file, encode(&fixture())).unwrap_or_else(|e| panic!("{e}"));
    assert!(native::run_cli(&[file.display().to_string()]).is_ok());
    std::fs::remove_dir_all(dir).unwrap_or_else(|e| panic!("{e}"));
}
