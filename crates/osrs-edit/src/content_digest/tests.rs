use super::*;
use crate::{
    HeightVertexCoord, LocKind, MapPosition, ObjectOrientation, OverlayRotation, OverlayShape,
    TerrainOverlay,
};
use osrs_core::ids::{FloorOverlayId, FloorUnderlayId, ObjectId};
fn plane(n: u8) -> SourcePlane {
    SourcePlane::new(n).unwrap_or_else(|| unreachable!())
}
fn tile() -> TerrainTile {
    TerrainTile {
        underlay: Some(FloorUnderlayId::new(0)),
        overlay: Some(TerrainOverlay {
            id: FloorOverlayId::new(u32::MAX),
            shape: OverlayShape::new(11).unwrap_or_else(|| unreachable!()),
            rotation: OverlayRotation::new(3).unwrap_or_else(|| unreachable!()),
        }),
        flags: 255,
    }
}
fn objects() -> [ObjectPlacement; 2] {
    [(0, 0, 3), (u32::MAX, 22, 2)].map(|(id, kind, rotation)| ObjectPlacement {
        object_id: ObjectId::new(id),
        position: MapPosition::new(MapTile::new(-1, -64), plane(1)),
        kind: LocKind::new(kind).unwrap_or_else(|| unreachable!()),
        orientation: ObjectOrientation::new(rotation).unwrap_or_else(|| unreachable!()),
    })
}
fn coord() -> MapPosition {
    MapPosition::new(MapTile::new(-65, 128), plane(3))
}
fn vertex() -> HeightVertexCoord {
    HeightVertexCoord::new(MapTile::new(i32::MIN, i32::MAX), plane(2))
}
fn document() -> MapDocument {
    MapDocument::from_parts([(coord(), tile())], [(vertex(), i32::MIN)], objects())
}
fn patch() -> MapPatch {
    let [remove, add] = objects();
    MapPatch::new([
        MapOp::SetTerrain {
            coord: coord(),
            before: TerrainTile::default(),
            after: tile(),
        },
        MapOp::SetHeight {
            coord: vertex(),
            before: i32::MIN,
            after: i32::MAX,
        },
        MapOp::AddObject { placement: add },
        MapOp::RemoveObject { placement: remove },
    ])
}

#[test]
fn independent_sha256_vectors_pin_empty_and_signed_full_width_content_protocol() {
    // Golden preimages built from the published byte grammar with Python struct/hashlib,
    // without calling the Rust encoder. Sizes: 49, 30, 114 and 106 bytes respectively.
    assert_eq!(
        MapDocument::new().content_digest_v1().to_string(),
        "cdbefee4f3b86a8c36b1ba25a013cf41296e0ecb209a43d52976f10b0febb8d0"
    );
    assert_eq!(
        MapPatch::default().content_digest_v1().to_string(),
        "1aa689026d259fcb52024246336d5d947016fe7c4a5c2e5b38d1f1900743a7d6"
    );
    assert_eq!(
        document().content_digest_v1().to_string(),
        "4275357880ccc389af2c5edcadc1eed2976eacf4464d463d669620dcba3fe2d3"
    );
    assert_eq!(
        patch().content_digest_v1().to_string(),
        "7b87e40f378dda152affe42713e4762ca1241fe2417f1d003aa51d645e1524d0"
    );
}

#[test]
fn canonical_insertion_order_defaults_duplicates_and_negative_region_boundaries() {
    let a = coord();
    let b = MapPosition::new(MapTile::new(-64, -65), plane(0));
    let h = vertex();
    let k = HeightVertexCoord::new(b.tile, plane(0));
    let [x, y] = objects();
    let first = MapDocument::from_parts([(a, tile()), (b, tile())], [(h, -8), (k, 0)], [x, y]);
    let second = MapDocument::from_parts(
        [
            (b, tile()),
            (a, tile()),
            (coord_at(400, 4, 1), TerrainTile::default()),
        ],
        [(k, 0), (h, -8)],
        [y, x, y],
    );
    assert_eq!(first, second);
    assert_eq!(first.content_digest_v1(), second.content_digest_v1());
    assert_eq!(
        MapDocument::new().content_digest_v1(),
        MapDocument::from_parts([(a, TerrainTile::default())], [], []).content_digest_v1()
    );
    assert_ne!(
        MapDocument::new().content_digest_v1(),
        MapDocument::from_parts([], [(h, 0)], []).content_digest_v1(),
        "unknown height is not zero"
    );
}
fn coord_at(x: i32, y: i32, p: u8) -> MapPosition {
    MapPosition::new(MapTile::new(x, y), plane(p))
}

#[test]
fn every_map_channel_identity_signed_value_and_optional_zero_is_observable() {
    let original = document();
    let mut variants = Vec::new();
    for changed in [
        TerrainTile {
            underlay: None,
            ..tile()
        },
        TerrainTile {
            underlay: Some(FloorUnderlayId::new(u32::MAX)),
            ..tile()
        },
        TerrainTile {
            overlay: None,
            ..tile()
        },
        TerrainTile { flags: 0, ..tile() },
    ] {
        variants.push(MapDocument::from_parts(
            [(coord(), changed)],
            [(vertex(), i32::MIN)],
            objects(),
        ));
    }
    for (id, shape, rotation) in [(0, 11, 3), (u32::MAX, 0, 3), (u32::MAX, 11, 0)] {
        let changed = TerrainTile {
            overlay: Some(TerrainOverlay {
                id: FloorOverlayId::new(id),
                shape: OverlayShape::new(shape).unwrap_or_else(|| unreachable!()),
                rotation: OverlayRotation::new(rotation).unwrap_or_else(|| unreachable!()),
            }),
            ..tile()
        };
        variants.push(MapDocument::from_parts(
            [(coord(), changed)],
            [(vertex(), i32::MIN)],
            objects(),
        ));
    }
    for changed in [
        coord_at(-64, 128, 3),
        coord_at(-65, 129, 3),
        coord_at(-65, 128, 2),
    ] {
        variants.push(MapDocument::from_parts(
            [(changed, tile())],
            [(vertex(), i32::MIN)],
            objects(),
        ));
    }
    for changed in [-1, 0, i32::MAX] {
        variants.push(MapDocument::from_parts(
            [(coord(), tile())],
            [(vertex(), changed)],
            objects(),
        ));
    }
    for changed in [
        HeightVertexCoord::new(MapTile::new(i32::MIN + 1, i32::MAX), plane(2)),
        HeightVertexCoord::new(MapTile::new(i32::MIN, i32::MAX - 1), plane(2)),
        HeightVertexCoord::new(vertex().corner, plane(1)),
    ] {
        variants.push(MapDocument::from_parts(
            [(coord(), tile())],
            [(changed, i32::MIN)],
            objects(),
        ));
    }
    let [a, b] = objects();
    for changed in [
        ObjectPlacement {
            object_id: ObjectId::new(1),
            ..a
        },
        ObjectPlacement {
            kind: LocKind::new(1).unwrap_or_else(|| unreachable!()),
            ..a
        },
        ObjectPlacement {
            orientation: ObjectOrientation::new(1).unwrap_or_else(|| unreachable!()),
            ..a
        },
        ObjectPlacement {
            position: coord_at(-2, -64, 1),
            ..a
        },
        ObjectPlacement {
            position: coord_at(-1, -63, 1),
            ..a
        },
        ObjectPlacement {
            position: coord_at(-1, -64, 2),
            ..a
        },
    ] {
        variants.push(MapDocument::from_parts(
            [(coord(), tile())],
            [(vertex(), i32::MIN)],
            [changed, b],
        ));
    }
    for (i, changed) in variants.iter().enumerate() {
        assert_ne!(original, *changed);
        assert_ne!(
            original.content_digest_v1(),
            changed.content_digest_v1(),
            "variant {i}"
        );
    }
}

#[test]
fn patch_hash_preserves_order_polarity_before_state_and_no_ops_without_applying() {
    let p = patch();
    let mut reversed = p.ops().to_vec();
    reversed.reverse();
    assert_ne!(
        p.content_digest_v1(),
        MapPatch::new(reversed).content_digest_v1()
    );
    assert_ne!(p.content_digest_v1(), p.inverse().content_digest_v1());
    for i in 0..p.len() {
        let mut ops = p.ops().to_vec();
        ops[i] = ops[i].inverse();
        assert_ne!(
            p.content_digest_v1(),
            MapPatch::new(ops).content_digest_v1()
        );
    }
    for changed in [
        MapOp::SetTerrain {
            coord: coord(),
            before: tile(),
            after: tile(),
        },
        MapOp::SetHeight {
            coord: vertex(),
            before: 0,
            after: 0,
        },
    ] {
        assert_ne!(
            MapPatch::default().content_digest_v1(),
            MapPatch::new([changed]).content_digest_v1()
        );
    }
    let a = MapOp::SetHeight {
        coord: vertex(),
        before: 1,
        after: 8,
    };
    let b = MapOp::SetHeight {
        coord: vertex(),
        before: 2,
        after: 8,
    };
    assert_ne!(
        MapPatch::new([a]).content_digest_v1(),
        MapPatch::new([b]).content_digest_v1()
    );
    let mut doc = MapDocument::from_parts([], [(vertex(), 1)], []);
    let original = doc.content_digest_v1();
    let p = MapPatch::new([a]);
    doc.apply_patch(&p).unwrap_or_else(|e| panic!("{e:?}"));
    assert_ne!(doc.content_digest_v1(), original);
    doc.apply_patch(&p.inverse())
        .unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(doc.content_digest_v1(), original);
}
