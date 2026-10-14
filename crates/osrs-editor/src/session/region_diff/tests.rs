use super::*;
use osrs_cache::decode::{EncodedTerrainTile, EncodedTileHeight};
use osrs_core::ids::{FloorOverlayId, FloorUnderlayId, ObjectId};
use osrs_edit::{LocKind, ObjectOrientation, OverlayRotation, OverlayShape, TerrainOverlay};

fn region() -> RegionCoord {
    RegionCoord::new(50, 50)
}

fn plane(value: u8) -> SourcePlane {
    SourcePlane::new(value).unwrap_or_else(|| unreachable!("test plane is valid"))
}

fn blank_terrain() -> DecodedTerrain {
    let mut tiles = Vec::new();
    for p in 0..SCENE_PLANES {
        for x in 0..REGION_TILES {
            for y in 0..REGION_TILES {
                tiles.push(EncodedTerrainTile {
                    tile: RegionTile::new(x as u8, y as u8)
                        .unwrap_or_else(|| unreachable!("valid tile")),
                    source_plane: plane(p as u8),
                    height: EncodedTileHeight::Default,
                    overlay: None,
                    settings: 0,
                    underlay_id: 0,
                });
            }
        }
    }
    DecodedTerrain::from_parts(region(), tiles, 0)
}

fn placement(id: u32, x: i32, y: i32) -> ObjectPlacement {
    ObjectPlacement {
        object_id: ObjectId::new(id),
        position: MapPosition::new(MapTile::new(x, y), plane(0)),
        kind: LocKind::new(10).unwrap_or_else(|| unreachable!("valid kind")),
        orientation: ObjectOrientation::new(1).unwrap_or_else(|| unreachable!("valid")),
    }
}

fn source_locations() -> DecodedLocations {
    let existing = placement(1500, 3210, 3210);
    let (_, tile) = existing.position.tile.split_region();
    DecodedLocations::with_locations(
        region(),
        vec![osrs_cache::decode::DecodedLocation {
            object_id: existing.object_id,
            tile,
            source_plane: plane(0),
            loc_type: existing.kind.get(),
            orientation: existing.orientation.get(),
        }],
    )
}

fn edited(document: &mut MapDocument) -> RegionDiffInputs {
    let tile = MapTile::new(3205, 3207);
    let position = MapPosition::new(tile, plane(0));
    let vertex = HeightVertexCoord::new(tile, plane(0));
    let before_height = document
        .height_at(vertex)
        .unwrap_or_else(|| unreachable!("loaded"));
    let painted = TerrainTile {
        underlay: Some(FloorUnderlayId::new(12)),
        overlay: Some(TerrainOverlay {
            id: FloorOverlayId::new(7),
            shape: OverlayShape::new(4).unwrap_or_else(|| unreachable!("valid")),
            rotation: OverlayRotation::new(2).unwrap_or_else(|| unreachable!("valid")),
        }),
        flags: 5,
    };
    let removed = placement(1500, 3210, 3210);
    let added = placement(2600, 3220, 3221);
    document
        .apply_patch(&MapPatch::new(vec![
            MapOp::SetTerrain {
                coord: position,
                before: TerrainTile::default(),
                after: painted,
            },
            MapOp::SetHeight {
                coord: vertex,
                before: before_height,
                after: -40,
            },
            MapOp::RemoveObject { placement: removed },
            MapOp::AddObject { placement: added },
        ]))
        .unwrap_or_else(|e| unreachable!("valid patch: {e:?}"));
    RegionDiffInputs {
        position,
        vertex,
        painted,
        removed,
        added,
    }
}

struct RegionDiffInputs {
    position: MapPosition,
    vertex: HeightVertexCoord,
    painted: TerrainTile,
    removed: ObjectPlacement,
    added: ObjectPlacement,
}

#[test]
fn an_unedited_document_has_an_empty_diff() {
    let (terrain, locations) = (blank_terrain(), source_locations());
    let baseline =
        baseline_document(&terrain, &locations).unwrap_or_else(|e| unreachable!("{e:?}"));
    let diff =
        diff_region(&terrain, &locations, &baseline).unwrap_or_else(|e| unreachable!("{e:?}"));
    assert!(diff.is_empty());
}

#[test]
fn the_diff_holds_exactly_the_edits_in_canonical_order() {
    let (terrain, locations) = (blank_terrain(), source_locations());
    let mut document =
        baseline_document(&terrain, &locations).unwrap_or_else(|e| unreachable!("{e:?}"));
    let edit = edited(&mut document);
    let diff =
        diff_region(&terrain, &locations, &document).unwrap_or_else(|e| unreachable!("{e:?}"));
    assert_eq!(diff.tiles, vec![(edit.position, edit.painted)]);
    assert_eq!(diff.heights, vec![(edit.vertex, -40)]);
    assert_eq!(diff.objects_added, vec![edit.added]);
    assert_eq!(diff.objects_removed, vec![edit.removed]);
}

#[test]
fn a_missing_region_is_not_a_mass_deletion() {
    let (terrain, locations) = (blank_terrain(), source_locations());
    assert_eq!(
        diff_region(&terrain, &locations, &MapDocument::new()),
        Err(DiffError::RegionNotLoaded(region()))
    );
}

#[test]
fn applying_a_diff_to_a_fresh_baseline_reproduces_the_edited_document() {
    let (terrain, locations) = (blank_terrain(), source_locations());
    let mut document =
        baseline_document(&terrain, &locations).unwrap_or_else(|e| unreachable!("{e:?}"));
    let edit = edited(&mut document);
    let diff =
        diff_region(&terrain, &locations, &document).unwrap_or_else(|e| unreachable!("{e:?}"));

    let mut fresh =
        baseline_document(&terrain, &locations).unwrap_or_else(|e| unreachable!("{e:?}"));
    let patch = diff.to_patch(&fresh);
    assert_eq!(patch.len(), 4);
    fresh
        .apply_patch(&patch)
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    assert_eq!(fresh.terrain_at(edit.position), edit.painted);
    assert_eq!(fresh.height_at(edit.vertex), Some(-40));
    assert!(fresh.contains_object(edit.added));
    assert!(!fresh.contains_object(edit.removed));

    // Idempotent: the diff of the rebuilt document is the same diff, and re-applying is a no-op.
    assert_eq!(
        diff_region(&terrain, &locations, &fresh).unwrap_or_else(|e| unreachable!("{e:?}")),
        diff
    );
    assert!(diff.to_patch(&fresh).is_empty());
}
