//! Real capture settings: source ownership, role filtering, exact indices and sparse round trip.
use super::*;
use osrs_core::ids::{FloorUnderlayId, ObjectId};
use osrs_edit::{LocKind, ObjectOrientation, TerrainTile};
use osrs_scene::{
    building_census::description::{Evidence, Member, MemberRole},
    object_semantics::Placed,
};
fn p(n: u8) -> SourcePlane {
    SourcePlane::new(n).unwrap_or_else(|| unreachable!())
}
fn fixture() -> (MapDocument, Review) {
    let floors: Vec<_> = (1..=3)
        .flat_map(|x| (1..=3).filter_map(move |y| (x != 2 || y != 2).then_some(MapTile::new(x, y))))
        .collect();
    let members: Vec<_> = [
        (1, 0, (0, 2), 0, MemberRole::Boundary),
        (2, 4, (0, 2), 0, MemberRole::Decoration),
        (3, 10, (1, 1), 0, MemberRole::Furniture),
        (4, 12, (2, 2), 1, MemberRole::Roof),
    ]
    .into_iter()
    .map(|(id, kind, tile, plane, role)| Member {
        placement: Placed {
            id: ObjectId::new(id),
            kind,
            tile,
            plane,
            orientation: 2,
        },
        role,
        evidence: Evidence::BoundaryIncident,
    })
    .collect();
    let objects: Vec<_> = members
        .iter()
        .map(|m| ObjectPlacement {
            object_id: m.placement.id,
            position: MapPosition::new(
                MapTile::new(m.placement.tile.0, m.placement.tile.1),
                p(m.placement.plane),
            ),
            kind: LocKind::new(m.placement.kind).unwrap_or_else(|| unreachable!()),
            orientation: ObjectOrientation::new(m.placement.orientation)
                .unwrap_or_else(|| unreachable!()),
        })
        .collect();
    let heights = (0..2).flat_map(|plane| {
        (0..=5).flat_map(move |x| {
            (0..=5).map(move |y| {
                (
                    HeightVertexCoord::new(MapTile::new(x, y), p(plane)),
                    -i32::from(plane) * 240,
                )
            })
        })
    });
    let terrain = (0..=5).flat_map(|x| {
        (0..=5).map(move |y| {
            (
                MapPosition::new(MapTile::new(x, y), p(0)),
                TerrainTile {
                    underlay: Some(FloorUnderlayId::new(if x == 0 { 99 } else { 3 })),
                    ..Default::default()
                },
            )
        })
    });
    let doc = MapDocument::from_parts(terrain, heights, objects.clone());
    let review = Review {
        #[cfg(not(target_arch = "wasm32"))]
        inputs: std::sync::Arc::new(ReviewInputs {
            area: CensusArea::new((0, 0), 6, 6),
            placements: vec![],
            table: ObjectTable::new(),
        }),
        sides: vec![],
        rooms: vec![RoomChoice {
            plane: 0,
            tiles: floors.clone(),
            included: true,
            role: "general".into(),
        }],
        overrides: vec![],
        labels: Default::default(),
        predictions: Default::default(),
        source: 0,
        revision: 0,
        description: BuildingDescription {
            floors: BTreeMap::from([(0, floors.into_iter().collect())]),
            members,
            boundaries: vec![],
            portals: vec![],
            connections: vec![],
            review: vec![],
        },
        included: vec![true; objects.len()],
        objects,
        planes: BTreeSet::from([0, 1]),
        provenance: "capture settings fixture".into(),
        object_sizes: (1..=4).map(|id| (ObjectId::new(id), (1, 1))).collect(),
        confirmed: true,
        options: Options::default(),
    };
    (doc, review)
}
#[test]
fn building_settings_keep_outside_walls_without_ground_and_preserve_courtyard() {
    let (doc, review) = fixture();
    let spec = intent(&review, &doc).unwrap_or_else(|e| panic!("{e:?}"));
    let prefab = osrs_edit::capture_building_prefab(&doc, &spec, "house")
        .unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(prefab.tiles.len(), 8);
    assert_eq!(prefab.objects.len(), 4);
    assert!(prefab.tiles.iter().all(|t| t.underlay == Some(3)));
    assert!(prefab.objects.iter().any(|o| o.x == 0 && o.kind == 0));
    assert!(
        !spec
            .floors
            .contains(&MapPosition::new(MapTile::new(2, 2), p(0)))
    );
    assert!(
        !spec
            .floors
            .contains(&MapPosition::new(MapTile::new(0, 2), p(0)))
    );
    assert!(
        spec.support
            .contains(&HeightVertexCoord::new(MapTile::new(0, 2), p(0)))
    );
}
#[test]
fn disabling_contents_keeps_structural_anchors_and_does_not_widen_terrain() {
    let (doc, mut review) = fixture();
    review.options = options::Options::structure_only();
    let spec = intent(&review, &doc).unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(spec.objects.len(), 1);
    assert_eq!(spec.objects[0].kind.get(), 0);
    assert_eq!(spec.floors.len(), 8);
    assert!(spec.labels.contains_key("inferred:member:0:role"));
    assert!(!spec.labels.contains_key("inferred:member:1:role"));
}

#[test]
fn manual_floor_and_missed_piece_corrections_survive_capture_without_map_edits() {
    let (doc, mut review) = fixture();
    let before = doc.clone();
    refine::change_floor(&mut review, MapTile::new(2, 2), 0, true);
    refine::change_floor(&mut review, MapTile::new(1, 1), 0, false);
    let mut piece = review.objects[2];
    piece.position.tile = MapTile::new(4, 4);
    refine::change_piece(&mut review, piece, (1, 1), true);
    let spec = intent(&review, &doc).unwrap_or_else(|e| panic!("{e}"));
    assert!(
        spec.floors
            .contains(&MapPosition::new(MapTile::new(2, 2), p(0)))
    );
    assert!(
        !spec
            .floors
            .contains(&MapPosition::new(MapTile::new(1, 1), p(0)))
    );
    assert!(
        spec.objects.contains(&piece),
        "missed occurrences need not be in the inferred candidate list"
    );
    assert_eq!(doc, before, "refinement is selection state, never an edit");
    refine::change_piece(&mut review, piece, (1, 1), false);
    assert!(
        !intent(&review, &doc)
            .unwrap_or_else(|e| panic!("{e}"))
            .objects
            .contains(&piece)
    );
}
