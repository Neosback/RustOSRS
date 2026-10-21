use super::*;
use crate::MapOp;
use osrs_core::{
    coords::{MapTile, SourcePlane},
    ids::ObjectId,
};
use serde_json::{Value, json};

// Hand-authored data; expected hashes are the independent struct/hashlib v1 byte vectors.
const SNAPSHOT: &str = r#"{
 "header":{"format":"rustosrs-map-snapshot-v1","digest_algorithm":"rustosrs-map-document-v1","digest":"4275357880ccc389af2c5edcadc1eed2976eacf4464d463d669620dcba3fe2d3"},
 "terrain":[{"position":{"x":-65,"y":128,"plane":3},"value":{"underlay":0,"overlay":{"id":4294967295,"shape":11,"rotation":3},"flags":255}}],
 "heights":[{"vertex":{"x":-2147483648,"y":2147483647,"plane":2},"value":-2147483648}],
 "objects":[
 {"id":0,"position":{"x":-1,"y":-64,"plane":1},"kind":0,"orientation":3},
 {"id":4294967295,"position":{"x":-1,"y":-64,"plane":1},"kind":22,"orientation":2}]
}"#;
const PATCH: &str = r#"{
 "header":{"format":"rustosrs-map-patch-json-v1","digest_algorithm":"rustosrs-map-patch-v1","digest":"7b87e40f378dda152affe42713e4762ca1241fe2417f1d003aa51d645e1524d0"},
 "operations":[
 {"op":"set_terrain","position":{"x":-65,"y":128,"plane":3},"before":{"underlay":null,"overlay":null,"flags":0},"after":{"underlay":0,"overlay":{"id":4294967295,"shape":11,"rotation":3},"flags":255}},
 {"op":"set_height","vertex":{"x":-2147483648,"y":2147483647,"plane":2},"before":-2147483648,"after":2147483647},
 {"op":"add_object","placement":{"id":4294967295,"position":{"x":-1,"y":-64,"plane":1},"kind":22,"orientation":2}},
 {"op":"remove_object","placement":{"id":0,"position":{"x":-1,"y":-64,"plane":1},"kind":0,"orientation":3}}]
}"#;
fn value(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|e| panic!("{e}"))
}
fn snapshot(text: &str) -> MapDocument {
    MapDocument::from_snapshot_json_v1(text).unwrap_or_else(|e| panic!("{e}"))
}
fn patch(text: &str) -> MapPatch {
    MapPatch::from_patch_json_v1(text).unwrap_or_else(|e| panic!("{e}"))
}
fn plane(p: u8) -> SourcePlane {
    SourcePlane::new(p).unwrap_or_else(|| unreachable!())
}
fn changed(text: &str, path: &str, replacement: Value) -> String {
    let mut v = value(text);
    *v.pointer_mut(path).unwrap_or_else(|| panic!("{path}")) = replacement;
    v.to_string()
}

fn invalid_record(error: Option<SemanticTransportError>, path: &str, field: &str, value: i32) {
    let error = error.unwrap_or_else(|| panic!("Accepted invalid {path}"));
    assert!(std::error::Error::source(&error).is_some());
    let message = error.to_string();
    let SemanticTransportError::Record {
        collection,
        index,
        error,
    } = error
    else {
        panic!("Expected indexed value error: {error:?}")
    };
    let parts: Vec<_> = path.split('/').collect();
    assert_eq!(collection, parts[1]);
    assert_eq!(index.to_string(), parts[2]);
    assert!(message.starts_with(&format!("{collection}[{index}]:")));
    assert!(
        matches!(*error,SemanticTransportError::InvalidValue {field:f,value:v} if f==field && i32::from(v)==value)
    );
}

#[test]
fn hand_authored_full_width_signed_snapshots_and_patches_round_trip_exactly() {
    let doc = snapshot(SNAPSHOT);
    assert_eq!(doc.object_count(), 2);
    assert_eq!(
        doc.height_at(HeightVertexCoord::new(
            MapTile::new(i32::MIN, i32::MAX),
            plane(2)
        )),
        Some(i32::MIN)
    );
    let tile = doc.terrain_at(MapPosition::new(MapTile::new(-65, 128), plane(3)));
    assert_eq!(tile.underlay.map(|id| id.get()), Some(0));
    assert_eq!(tile.overlay.map(|o| o.id.get()), Some(u32::MAX));
    assert!(
        doc.object_entries()
            .iter()
            .any(|p| p.object_id == ObjectId::new(u32::MAX))
    );
    let encoded = doc.to_snapshot_json_v1().unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(snapshot(&encoded), doc);
    assert_eq!(
        snapshot(&encoded)
            .to_snapshot_json_v1()
            .unwrap_or_else(|e| panic!("{e}")),
        encoded
    );
    let p = patch(PATCH);
    assert_eq!(p.len(), 4);
    assert_eq!(
        p.content_digest_v1().to_string(),
        "7b87e40f378dda152affe42713e4762ca1241fe2417f1d003aa51d645e1524d0"
    );
    let encoded = p.to_patch_json_v1().unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(patch(&encoded), p);
    assert_eq!(patch(&encoded).impact(), p.impact());
    assert_eq!(
        MapDocument::from_snapshot_json_v1(
            &MapDocument::new()
                .to_snapshot_json_v1()
                .unwrap_or_else(|e| panic!("{e}"))
        )
        .unwrap_or_else(|e| panic!("{e}")),
        MapDocument::new()
    );
    assert_eq!(
        patch(
            &MapPatch::default()
                .to_patch_json_v1()
                .unwrap_or_else(|e| panic!("{e}"))
        ),
        MapPatch::default()
    );
}
#[test]
fn snapshot_record_order_is_irrelevant_but_duplicate_records_are_rejected() {
    let mut v = value(SNAPSHOT);
    v["objects"]
        .as_array_mut()
        .unwrap_or_else(|| unreachable!())
        .reverse();
    assert_eq!(snapshot(&v.to_string()), snapshot(SNAPSHOT));
    assert_eq!(
        snapshot(&v.to_string())
            .to_snapshot_json_v1()
            .unwrap_or_else(|e| panic!("{e}")),
        snapshot(SNAPSHOT)
            .to_snapshot_json_v1()
            .unwrap_or_else(|e| panic!("{e}"))
    );
    for field in ["terrain", "heights", "objects"] {
        let mut v = value(SNAPSHOT);
        let rows = v[field].as_array_mut().unwrap_or_else(|| unreachable!());
        rows.push(rows[0].clone());
        let error = MapDocument::from_snapshot_json_v1(&v.to_string());
        assert!(matches!(
            (field, error),
            ("terrain", Err(SemanticTransportError::DuplicateTerrain(_)))
                | ("heights", Err(SemanticTransportError::DuplicateHeight(_)))
                | ("objects", Err(SemanticTransportError::DuplicateObject(_)))
        ));
    }
    let mut v = value(SNAPSHOT);
    let mut other = v["heights"][0].clone();
    other["value"] = json!(0);
    v["heights"]
        .as_array_mut()
        .unwrap_or_else(|| unreachable!())
        .push(other);
    assert!(matches!(
        MapDocument::from_snapshot_json_v1(&v.to_string()),
        Err(SemanticTransportError::DuplicateHeight(_))
    ));
    let mut v = value(SNAPSHOT);
    let mut other = v["terrain"][0].clone();
    other["value"]["flags"] = json!(0);
    v["terrain"]
        .as_array_mut()
        .unwrap_or_else(|| unreachable!())
        .push(other);
    assert!(matches!(
        MapDocument::from_snapshot_json_v1(&v.to_string()),
        Err(SemanticTransportError::DuplicateTerrain(_))
    ));
}
#[test]
fn snapshot_defaults_normalize_but_missing_height_is_never_filled_with_zero() {
    let mut v = value(
        &MapDocument::new()
            .to_snapshot_json_v1()
            .unwrap_or_else(|e| panic!("{e}")),
    );
    v["terrain"] = json!([{"position":{"x":-64,"y":64,"plane":0},"value":{"underlay":null,"overlay":null,"flags":0}}]);
    assert_eq!(snapshot(&v.to_string()), MapDocument::new());
    v["heights"] = json!([{"vertex":{"x":-64,"y":64,"plane":0},"value":0}]);
    assert!(matches!(
        MapDocument::from_snapshot_json_v1(&v.to_string()),
        Err(SemanticTransportError::DigestMismatch { .. })
    ));
    let mut v = value(SNAPSHOT);
    v["heights"] = json!([]);
    assert!(matches!(
        MapDocument::from_snapshot_json_v1(&v.to_string()),
        Err(SemanticTransportError::DigestMismatch { .. })
    ));
}
#[test]
fn invalid_semantic_domains_are_rejected_before_digest_comparison() {
    for (path, n, field) in [
        ("/terrain/0/position/plane", 4, "source plane"),
        ("/heights/0/vertex/plane", 255, "source plane"),
        ("/objects/0/position/plane", 4, "source plane"),
        ("/objects/0/kind", 23, "loc kind"),
        ("/objects/0/orientation", 4, "object orientation"),
        ("/terrain/0/value/overlay/shape", 12, "overlay shape"),
        ("/terrain/0/value/overlay/rotation", 4, "overlay rotation"),
    ] {
        invalid_record(
            MapDocument::from_snapshot_json_v1(&changed(SNAPSHOT, path, json!(n))).err(),
            path,
            field,
            n,
        );
    }
    for (path, n, field) in [
        ("/operations/0/position/plane", 4, "source plane"),
        ("/operations/1/vertex/plane", 4, "source plane"),
        ("/operations/0/before/overlay", 0, ""),
        ("/operations/2/placement/kind", 23, "loc kind"),
        (
            "/operations/3/placement/orientation",
            4,
            "object orientation",
        ),
        ("/operations/0/after/overlay/shape", 12, "overlay shape"),
        (
            "/operations/0/after/overlay/rotation",
            4,
            "overlay rotation",
        ),
    ] {
        let result = MapPatch::from_patch_json_v1(&changed(PATCH, path, json!(n)));
        if field.is_empty() {
            assert!(matches!(result, Err(SemanticTransportError::Json(_))));
        } else {
            invalid_record(result.err(), path, field, n);
        }
    }
}
#[test]
fn numeric_ranges_optional_fields_unknown_fields_and_duplicate_json_keys_fail_closed() {
    for (path, v) in [
        ("/terrain/0/position/x", json!(-2147483649_i64)),
        ("/terrain/0/position/y", json!(2147483648_i64)),
        ("/heights/0/value", json!(2147483648_i64)),
        ("/terrain/0/value/underlay", json!(-1)),
        ("/terrain/0/value/overlay/id", json!(4294967296_u64)),
        ("/terrain/0/value/flags", json!(256)),
        ("/objects/0/id", json!(-1)),
        ("/objects/0/kind", json!(256)),
        ("/objects/0/orientation", json!(1.5)),
        ("/objects/0/position/plane", json!(null)),
    ] {
        assert!(
            matches!(
                MapDocument::from_snapshot_json_v1(&changed(SNAPSHOT, path, v)),
                Err(SemanticTransportError::Json(_))
            ),
            "{path}"
        );
    }
    for path in [
        "",
        "/header",
        "/terrain/0",
        "/terrain/0/position",
        "/terrain/0/value",
        "/terrain/0/value/overlay",
        "/heights/0",
        "/heights/0/vertex",
        "/objects/0",
        "/objects/0/position",
    ] {
        let mut v = value(SNAPSHOT);
        v.pointer_mut(path)
            .unwrap_or_else(|| panic!("{path}"))
            .as_object_mut()
            .unwrap_or_else(|| unreachable!())
            .insert("unsupported".into(), json!(true));
        assert!(
            matches!(
                MapDocument::from_snapshot_json_v1(&v.to_string()),
                Err(SemanticTransportError::Json(_))
            ),
            "{path}"
        );
    }
    for key in ["underlay", "overlay", "flags"] {
        let mut v = value(SNAPSHOT);
        v["terrain"][0]["value"]
            .as_object_mut()
            .unwrap_or_else(|| unreachable!())
            .remove(key);
        assert!(
            matches!(
                MapDocument::from_snapshot_json_v1(&v.to_string()),
                Err(SemanticTransportError::Json(_))
            ),
            "{key}"
        );
    }
    let text = SNAPSHOT.replace("\"flags\":255", "\"flags\":0,\"flags\":255");
    assert!(matches!(
        MapDocument::from_snapshot_json_v1(&text),
        Err(SemanticTransportError::Json(_))
    ));
    for path in [
        "",
        "/header",
        "/operations/0",
        "/operations/0/position",
        "/operations/0/before",
        "/operations/0/after/overlay",
        "/operations/1/vertex",
        "/operations/2/placement",
    ] {
        let mut v = value(PATCH);
        v.pointer_mut(path)
            .unwrap_or_else(|| panic!("{path}"))
            .as_object_mut()
            .unwrap_or_else(|| unreachable!())
            .insert("unsupported".into(), json!(true));
        assert!(
            matches!(
                MapPatch::from_patch_json_v1(&v.to_string()),
                Err(SemanticTransportError::Json(_))
            ),
            "{path}"
        );
    }
    assert!(matches!(
        MapPatch::from_patch_json_v1(&changed(
            PATCH,
            "/operations/0/op",
            json!("replace_document")
        )),
        Err(SemanticTransportError::Json(_))
    ));
    for text in [
        PATCH.replace(
            "\"before\":-2147483648",
            "\"before\":0,\"before\":-2147483648",
        ),
        PATCH.replace(
            "\"op\":\"set_height\"",
            "\"op\":\"set_height\",\"op\":\"set_height\"",
        ),
    ] {
        assert!(matches!(
            MapPatch::from_patch_json_v1(&text),
            Err(SemanticTransportError::Json(_))
        ));
    }
    assert!(MapDocument::from_snapshot_json_v1(&SNAPSHOT[..SNAPSHOT.len() - 2]).is_err());
    assert!(MapPatch::from_patch_json_v1(&PATCH[..PATCH.len() - 2]).is_err());
}
#[test]
fn wrong_format_algorithms_hashes_and_mutated_content_never_enter_semantics() {
    for (text, is_patch) in [(SNAPSHOT, false), (PATCH, true)] {
        let decode = |text: &str| {
            if is_patch {
                MapPatch::from_patch_json_v1(text).map(|_| ())
            } else {
                MapDocument::from_snapshot_json_v1(text).map(|_| ())
            }
        };
        assert!(matches!(
            decode(&changed(text, "/header/format", json!("future-v2"))),
            Err(SemanticTransportError::UnsupportedFormat { .. })
        ));
        assert!(matches!(
            decode(&changed(text, "/header/digest_algorithm", json!("unknown"))),
            Err(SemanticTransportError::UnsupportedDigest { .. })
        ));
        assert!(matches!(
            decode(&changed(text, "/header/digest", json!("A".repeat(64)))),
            Err(SemanticTransportError::InvalidDigest(_))
        ));
        assert!(matches!(
            decode(&changed(text, "/header/digest", json!("00"))),
            Err(SemanticTransportError::InvalidDigest(_))
        ));
        assert!(matches!(
            decode(&changed(text, "/header/digest", json!("0".repeat(64)))),
            Err(SemanticTransportError::DigestMismatch { .. })
        ));
    }
    for path in [
        "/terrain/0/value/flags",
        "/heights/0/value",
        "/objects/0/id",
    ] {
        assert!(matches!(
            MapDocument::from_snapshot_json_v1(&changed(SNAPSHOT, path, json!(99))),
            Err(SemanticTransportError::DigestMismatch { .. })
        ));
    }
    let mut v = value(PATCH);
    v["operations"]
        .as_array_mut()
        .unwrap_or_else(|| unreachable!())
        .reverse();
    assert!(matches!(
        MapPatch::from_patch_json_v1(&v.to_string()),
        Err(SemanticTransportError::DigestMismatch { .. })
    ));
    assert!(matches!(
        MapPatch::from_patch_json_v1(&changed(PATCH, "/operations/1/before", json!(0))),
        Err(SemanticTransportError::DigestMismatch { .. })
    ));
}
#[test]
fn decoded_order_noops_and_derived_impact_apply_and_undo_through_normal_history() {
    let encoded = patch(PATCH);
    let mut ops = encoded.ops().to_vec();
    ops.push(MapOp::SetHeight {
        coord: HeightVertexCoord::new(MapTile::new(i32::MIN, i32::MAX), plane(2)),
        before: i32::MAX,
        after: i32::MAX,
    });
    let original = MapPatch::new(ops);
    let decoded = patch(
        &original
            .to_patch_json_v1()
            .unwrap_or_else(|e| panic!("{e}")),
    );
    assert_eq!(decoded, original);
    assert_eq!(decoded.len(), 5);
    let loaded = snapshot(SNAPSHOT);
    let remove = loaded.object_entries()[0];
    let mut document = MapDocument::from_parts([], loaded.height_entries(), [remove]);
    let before = document.clone();
    let mut history = crate::EditHistory::new();
    history
        .commit(&mut document, decoded)
        .unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(
        document.terrain_at(MapPosition::new(MapTile::new(-65, 128), plane(3))),
        loaded.terrain_at(MapPosition::new(MapTile::new(-65, 128), plane(3)))
    );
    assert_eq!(
        document.height_at(HeightVertexCoord::new(
            MapTile::new(i32::MIN, i32::MAX),
            plane(2)
        )),
        Some(i32::MAX)
    );
    history
        .undo(&mut document)
        .unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(document, before);
    assert!(history.can_redo());
    let invalid = MapPatch::new([MapOp::AddObject { placement: remove }]);
    let decoded = patch(&invalid.to_patch_json_v1().unwrap_or_else(|e| panic!("{e}")));
    assert!(
        history.commit(&mut document, decoded).is_err(),
        "integrity does not authorize an inapplicable transaction"
    );
    assert_eq!(document, before);
    assert!(history.can_redo());
    let inverse = patch(
        &original
            .inverse()
            .to_patch_json_v1()
            .unwrap_or_else(|e| panic!("{e}")),
    );
    assert_eq!(inverse, original.inverse());
}
