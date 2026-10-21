use super::*;
use crate::app::edit_runtime::EditRuntime;
use osrs_core::coords::{RegionCoord, SourcePlane};
use osrs_edit::{CaptureSpec, HeightVertexCoord, MapDocument, MapRegionSnapshot, PrefabSource};

fn fixture() -> (MapDocument, Armed, Prefab, HeightVertexCoord) {
    let plane = SourcePlane::new(0).unwrap_or_else(|| panic!("plane"));
    let coord = HeightVertexCoord::new(MapTile::new(0, 0), plane);
    let document = MapDocument::from_parts([], [(coord, 8)], []);
    let prefab = osrs_edit::capture_prefab(
        &document,
        CaptureSpec {
            origin: MapTile::new(0, 0),
            width: 1,
            height: 1,
            base_plane: plane,
            planes: 1,
            terrain: false,
            heights: true,
            objects: false,
        },
        "test",
        PrefabSource::Authored,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let armed = Armed {
        file: "test.prefab.json".into(),
        turns: 0,
        terrain: true,
        heights: true,
        objects: true,
        replace_objects: true,
    };
    (document, armed, prefab, coord)
}

fn review(armed: &Armed, prefab: &Prefab, patch: MapPatch, revision: u64) -> PlacementReview {
    PlacementReview::new(
        patch,
        revision,
        2,
        "test".into(),
        PlacementInput {
            armed: armed.clone(),
            gesture: Some(armed.clone()),
            prefab: prefab.clone(),
            plane: 0,
            origin: MapTile::new(0, 0),
            definitions: Default::default(),
        },
    )
}

#[test]
fn cancellation_and_stale_replacement_leave_authority_and_history_unchanged() {
    let (document, armed, prefab, _) = fixture();
    let before = document.clone();
    let pending = review(&armed, &prefab, MapPatch::new(vec![]), 1);
    drop(pending);
    assert_eq!(document, before);
    let pending = review(&armed, &prefab, MapPatch::new(vec![]), 1);
    assert!(
        pending
            .approve(&Current {
                revision: 2,
                generation: 2,
                armed: Some(&armed),
                prefab: Some(&prefab),
                plane: 0,
                size_of: &|_, _| None
            })
            .is_err()
    );
    assert_eq!(document, before);
}

#[test]
fn hydration_and_eviction_revoke_review_at_an_unchanged_authored_revision() {
    for evict in [false, true] {
        let (document, armed, prefab, coord) = fixture();
        let mut session = EditRuntime::default();
        let region = RegionCoord::new(0, 0);
        session.ingest_region(
            &MapRegionSnapshot::from_parts(region, [], document.height_entries(), [])
                .unwrap_or_else(|e| panic!("{e:?}")),
        );
        let old = session.revision();
        let authored = session.map_revision();
        let pending = review(
            &armed,
            &prefab,
            MapPatch::new(vec![MapOp::SetHeight {
                coord,
                before: 8,
                after: 16,
            }]),
            old,
        );
        if evict {
            session.unload_region(region);
        } else {
            session.ingest_region(
                &MapRegionSnapshot::from_parts(region, [], [(coord, 24)], [])
                    .unwrap_or_else(|e| panic!("{e:?}")),
            );
        }
        assert_eq!(session.map_revision(), authored);
        assert_ne!(session.revision(), old);
        let before = session.document().clone();
        assert!(
            pending
                .approve(&Current {
                    revision: session.revision(),
                    generation: 2,
                    armed: Some(&armed),
                    prefab: Some(&prefab),
                    plane: 0,
                    size_of: &|_, _| None
                })
                .is_err()
        );
        assert_eq!(session.document(), &before);
        assert!(!session.can_undo());
    }
}

#[test]
fn source_gesture_asset_plane_and_catalog_changes_revoke_admission() {
    let (_, armed, prefab, _) = fixture();
    let id = ObjectId::new(5);
    let kind = LocKind::new(10).unwrap_or_else(|| panic!("kind"));
    for change in ["source", "gesture", "asset", "removed", "plane", "catalog"] {
        let mut pending = review(&armed, &prefab, MapPatch::new(vec![]), 1);
        pending.input.definitions.insert((id, kind), None);
        let mut changed_armed = armed.clone();
        let mut changed_prefab = prefab.clone();
        if change == "gesture" {
            changed_armed.turns = 1;
        }
        if change == "asset" {
            changed_prefab.name.push_str(" edited");
        }
        let current = Current {
            revision: 1,
            generation: if change == "source" { 3 } else { 2 },
            armed: Some(&changed_armed),
            prefab: (change != "removed").then_some(&changed_prefab),
            plane: u8::from(change == "plane"),
            size_of: &|_, _| (change == "catalog").then_some((2, 3)),
        };
        assert!(!pending.is_current(&current), "{change}");
        assert!(pending.approve(&current).is_err(), "{change}");
    }
    assert!(matches!(
        resolve_size(&prefab, id, kind, None),
        Err(PrefabError::MissingFootprint { .. })
    ));
    assert_eq!(resolve_size(&prefab, id, kind, Some((2, 3))), Ok((2, 3)));
}

#[test]
fn current_review_commits_through_the_gateway_and_undoes_exactly() {
    let (document, armed, prefab, coord) = fixture();
    let mut session = EditRuntime::default();
    session.ingest_region(
        &MapRegionSnapshot::from_parts(RegionCoord::new(0, 0), [], document.height_entries(), [])
            .unwrap_or_else(|e| panic!("{e:?}")),
    );
    let before = session.document().clone();
    let pending = review(
        &armed,
        &prefab,
        MapPatch::new(vec![MapOp::SetHeight {
            coord,
            before: 8,
            after: 16,
        }]),
        session.revision(),
    );
    let (patch, _) = pending
        .approve(&Current {
            revision: session.revision(),
            generation: 2,
            armed: Some(&armed),
            prefab: Some(&prefab),
            plane: 0,
            size_of: &|_, _| None,
        })
        .unwrap_or_else(|e| panic!("{e}"));
    session.commit(patch).unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(session.document().height_at(coord), Some(16));
    assert!(session.can_undo());
    session.undo().unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(session.document(), &before);
    assert!(session.can_redo());
}

#[test]
fn script_reviews_preserve_an_unarmed_gesture_and_portable_sizes_remain_explicit() {
    let (_, armed, mut prefab, _) = fixture();
    let mut pending = review(&armed, &prefab, MapPatch::new(vec![]), 1);
    pending.input.gesture = None;
    assert!(
        pending
            .approve(&Current {
                revision: 1,
                generation: 2,
                armed: None,
                prefab: Some(&prefab),
                plane: 0,
                size_of: &|_, _| None
            })
            .is_ok()
    );
    let id = ObjectId::new(5);
    let kind = LocKind::new(10).unwrap_or_else(|| panic!("kind"));
    prefab.building = Some(osrs_edit::PrefabBuilding {
        floors: vec![],
        support: vec![],
        source_datum: 0,
        object_sizes: [(5, (2, 3))].into(),
        provenance: "portable dimensions".into(),
        relations: vec![],
        labels: Default::default(),
        reviewed: true,
        semantics: Default::default(),
    });
    assert_eq!(resolve_size(&prefab, id, kind, None), Ok((2, 3)));
    assert_eq!(resolve_size(&prefab, id, kind, Some((4, 1))), Ok((4, 1)));
    prefab
        .building
        .as_mut()
        .unwrap_or_else(|| panic!("building"))
        .object_sizes
        .insert(5, (256, 3));
    assert!(matches!(
        resolve_size(&prefab, id, kind, None),
        Err(PrefabError::MissingFootprint { .. })
    ));
}
