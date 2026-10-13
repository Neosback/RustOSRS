use super::*;
use osrs_cache::decode::EncodedTerrainTile;
use osrs_core::ids::{FloorOverlayId, FloorUnderlayId, ObjectId};
use osrs_edit::{
    LocKind, MapOp, MapPatch, ObjectOrientation, OverlayRotation, OverlayShape, TerrainOverlay,
    TerrainTile,
};

const PROFILE_YAML: &str =
    include_str!("../../../../../profiles/osrs-live-241-2026-09-30-openrs2-2727.yaml");

fn decoder_context() -> osrs_cache::decode::DecoderContext {
    let profile = osrs_cache::profile::TargetProfile::from_yaml_str(PROFILE_YAML)
        .unwrap_or_else(|e| unreachable!("pinned profile parses: {e:?}"));
    osrs_cache::decode::DecoderContext::from_profile(&profile)
        .unwrap_or_else(|e| unreachable!("pinned profile builds a context: {e:?}"))
}

fn region() -> RegionCoord {
    RegionCoord::new(50, 50)
}

fn plane(value: u8) -> SourcePlane {
    SourcePlane::new(value).unwrap_or_else(|| unreachable!("test plane is valid"))
}

fn blank_terrain() -> DecodedTerrain {
    let mut tiles = Vec::with_capacity(SCENE_PLANES * TILES_PER_PLANE);
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

fn placement(id: u32, x: i32, y: i32, p: u8) -> ObjectPlacement {
    ObjectPlacement {
        object_id: ObjectId::new(id),
        position: MapPosition::new(MapTile::new(x, y), plane(p)),
        kind: LocKind::new(10).unwrap_or_else(|| unreachable!("valid kind")),
        orientation: ObjectOrientation::new(1).unwrap_or_else(|| unreachable!("valid rotation")),
    }
}

fn source_locations() -> DecodedLocations {
    DecodedLocations::with_locations(region(), vec![location_of(placement(1500, 3210, 3210, 0))])
}

/// Re-import exported streams the way the editor loads a region.
fn reimport(exported: &ExportedRegion) -> MapDocument {
    let tiles: Vec<EncodedTerrainTile> = blank_terrain().tiles().to_vec();
    // Decode the exported terrain stream with the production decoder semantics by replaying it
    // through the same adapter the loader uses.
    let decoded = decode_for_test(exported.region, &exported.terrain, &tiles);
    let locations = decode_locations_for_test(exported.region, &exported.locations);
    let map = RegionMap {
        terrain: RegionTerrain::new(region_tiles_from_decoded(&decoded))
            .unwrap_or_else(|e| unreachable!("{e:?}")),
        locations,
    };
    let snapshot = adapt_region_map(&map).unwrap_or_else(|e| unreachable!("{e:?}"));
    let mut document = MapDocument::new();
    document.replace_region(&snapshot);
    document
}

fn decode_for_test(
    region: RegionCoord,
    bytes: &[u8],
    template: &[EncodedTerrainTile],
) -> DecodedTerrain {
    use osrs_cache::decode::{ArchiveFileProvenance, decode_terrain};
    let context = decoder_context();
    let source = ArchiveFileProvenance::new(5, 0, Some(0));
    let decoded = decode_terrain(bytes, &context, &source, region)
        .unwrap_or_else(|e| unreachable!("exported terrain decodes: {e:?}"));
    assert_eq!(decoded.len(), template.len());
    decoded
}

fn decode_locations_for_test(region: RegionCoord, bytes: &[u8]) -> DecodedLocations {
    use osrs_cache::decode::{ArchiveFileProvenance, decode_locations};
    let context = decoder_context();
    let source = ArchiveFileProvenance::new(5, 0, Some(1));
    decode_locations(bytes, &context, &source, region)
        .unwrap_or_else(|e| unreachable!("exported locations decode: {e:?}"))
}

#[test]
fn an_unedited_square_is_not_exported() {
    let terrain = blank_terrain();
    let locations = source_locations();
    let baseline =
        baseline_document(&terrain, &locations).unwrap_or_else(|e| unreachable!("{e:?}"));
    assert_eq!(export_region(&terrain, &locations, &baseline), Ok(None));
}

#[test]
fn edited_terrain_heights_and_objects_survive_export_and_reload() {
    let terrain = blank_terrain();
    let locations = source_locations();
    let mut document =
        baseline_document(&terrain, &locations).unwrap_or_else(|e| unreachable!("{e:?}"));

    let tile = MapTile::new(3205, 3207);
    let position = MapPosition::new(tile, plane(0));
    let painted = TerrainTile {
        underlay: Some(FloorUnderlayId::new(12)),
        overlay: Some(TerrainOverlay {
            id: FloorOverlayId::new(7),
            shape: OverlayShape::new(4).unwrap_or_else(|| unreachable!("valid")),
            rotation: OverlayRotation::new(2).unwrap_or_else(|| unreachable!("valid")),
        }),
        flags: 5,
    };
    let vertex = HeightVertexCoord::new(tile, plane(0));
    let before_height = document
        .height_at(vertex)
        .unwrap_or_else(|| unreachable!("loaded"));
    let upper = HeightVertexCoord::new(tile, plane(1));
    let upper_height = document
        .height_at(upper)
        .unwrap_or_else(|| unreachable!("loaded"));
    let new_height = -40;
    let removed = placement(1500, 3210, 3210, 0);
    let added = placement(2600, 3220, 3221, 1);
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
                after: new_height,
            },
            MapOp::RemoveObject { placement: removed },
            MapOp::AddObject { placement: added },
        ]))
        .unwrap_or_else(|e| unreachable!("valid patch: {e:?}"));

    let exported = export_region(&terrain, &locations, &document)
        .unwrap_or_else(|e| unreachable!("{e:?}"))
        .unwrap_or_else(|| unreachable!("edited square exports"));
    assert_eq!(
        exported.changes,
        ExportChanges {
            tiles: 1,
            heights: 2,
            objects_added: 1,
            objects_removed: 1,
        }
    );

    // Reloading the export reproduces the edited document exactly, including the plane above
    // keeping its absolute height even though its stored byte is relative to the edited plane.
    let reloaded = reimport(&exported);
    assert_eq!(reloaded.terrain_at(position), painted);
    assert_eq!(reloaded.height_at(vertex), Some(new_height));
    assert_eq!(reloaded.height_at(upper), Some(upper_height));
    assert!(reloaded.contains_object(added));
    assert!(!reloaded.contains_object(removed));
    for other in [
        HeightVertexCoord::new(MapTile::new(3206, 3207), plane(0)),
        HeightVertexCoord::new(MapTile::new(3205, 3208), plane(0)),
    ] {
        assert_eq!(reloaded.height_at(other), document.height_at(other));
    }
}

#[test]
fn unstorable_heights_are_an_error_not_a_silent_round() {
    let terrain = blank_terrain();
    let locations = source_locations();
    let mut document =
        baseline_document(&terrain, &locations).unwrap_or_else(|e| unreachable!("{e:?}"));
    let vertex = HeightVertexCoord::new(MapTile::new(3201, 3201), plane(0));
    let before = document
        .height_at(vertex)
        .unwrap_or_else(|| unreachable!("loaded"));
    // Positive heights are above the plane base and cannot be stored (bytes only lower it).
    document
        .apply_patch(&MapPatch::new(vec![MapOp::SetHeight {
            coord: vertex,
            before,
            after: 1000,
        }]))
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    assert!(matches!(
        export_region(&terrain, &locations, &document),
        Err(ExportError::HeightNotStorable { height: 1000, .. })
    ));
}

/// Needs the pinned cache (`RUSTOSRS_TARGET_CACHE_DIR`); skipped when it is not configured.
#[test]
fn real_squares_export_as_unedited_and_survive_a_real_edit() {
    use osrs_cache::decode::{decode_locations, decode_terrain};
    use osrs_cache::transport::CacheRepository;
    let Some(dir) = std::env::var_os("RUSTOSRS_TARGET_CACHE_DIR") else {
        return;
    };
    let profile = osrs_cache::profile::TargetProfile::from_yaml_str(PROFILE_YAML)
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let cache = CacheRepository::open(&dir, &profile).unwrap_or_else(|e| unreachable!("{e:?}"));
    let context = decoder_context();
    let mut checked = 0;
    // A spread of squares around Lumbridge/Varrock and the wilderness edge.
    for (x, y) in [
        (50, 50),
        (51, 53),
        (48, 48),
        (49, 54),
        (46, 52),
        (53, 56),
        (40, 50),
    ] {
        let region = RegionCoord::new(x, y);
        let Ok(square) = cache.read_map_square(region) else {
            continue;
        };
        let (Ok(terrain), Ok(locations)) = (
            decode_terrain(
                &square.terrain.bytes,
                &context,
                &square.terrain.provenance,
                region,
            ),
            decode_locations(
                &square.locations.bytes,
                &context,
                &square.locations.provenance,
                region,
            ),
        ) else {
            continue;
        };
        let mut document =
            baseline_document(&terrain, &locations).unwrap_or_else(|e| unreachable!("{e:?}"));
        assert_eq!(
            export_region(&terrain, &locations, &document),
            Ok(None),
            "unedited real square ({x},{y}) must not be exported"
        );

        // Paint a tile, lower a vertex, and drop the first object, then reload the export.
        let origin = MapTile::from_region(
            region,
            RegionTile::new(10, 10).unwrap_or_else(|| unreachable!("valid tile")),
        )
        .unwrap_or_else(|| unreachable!("in range"));
        let position = MapPosition::new(origin, plane(0));
        let vertex = HeightVertexCoord::new(origin, plane(0));
        let before_tile = document.terrain_at(position);
        let painted = TerrainTile {
            underlay: Some(FloorUnderlayId::new(9)),
            ..before_tile
        };
        let before_height = document
            .height_at(vertex)
            .unwrap_or_else(|| unreachable!("loaded"));
        let lowered = (before_height - 64).max(-2040);
        let mut ops = vec![MapOp::SetHeight {
            coord: vertex,
            before: before_height,
            after: lowered,
        }];
        if painted != before_tile {
            ops.push(MapOp::SetTerrain {
                coord: position,
                before: before_tile,
                after: painted,
            });
        }
        let first_object = document.objects_in_region(region).into_iter().next();
        if let Some(object) = first_object {
            ops.push(MapOp::RemoveObject { placement: object });
        }
        document
            .apply_patch(&MapPatch::new(ops))
            .unwrap_or_else(|e| unreachable!("valid patch: {e:?}"));
        let exported = export_region(&terrain, &locations, &document)
            .unwrap_or_else(|e| unreachable!("({x},{y}): {e:?}"))
            .unwrap_or_else(|| unreachable!("edited square exports"));
        let reloaded = reimport(&exported);
        assert_eq!(reloaded.terrain_at(position), painted, "({x},{y}) tile");
        assert_eq!(
            reloaded.height_at(vertex),
            Some(lowered),
            "({x},{y}) height"
        );
        if let Some(object) = first_object {
            assert!(!reloaded.contains_object(object), "({x},{y}) object");
        }
        // Everything else is identical to the source square's semantics.
        let untouched = HeightVertexCoord::new(MapTile::new(origin.x + 5, origin.y + 5), plane(2));
        assert_eq!(reloaded.height_at(untouched), document.height_at(untouched));
        checked += 1;
    }
    assert!(checked > 0, "no real squares were checked");
}
