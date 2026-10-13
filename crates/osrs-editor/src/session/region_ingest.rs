use osrs_core::{
    coords::{MapTile, RegionCoord, RegionTile, SourcePlane},
    ids::{FloorOverlayId, FloorUnderlayId},
};
use osrs_edit::{
    HeightVertexCoord, LocKind, MapPosition, MapRegionSnapshot, ObjectOrientation, ObjectPlacement,
    OverlayRotation, OverlayShape, RegionSnapshotError, TerrainOverlay, TerrainTile,
};
use osrs_scene::terrain_load::{REGION_TILES, SCENE_PLANES, TerrainLoadGrid};
use osrs_world::RegionMap;

const FLOOR_ID_MASK: u16 = 0x7fff;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionAdaptError {
    WorldCoordinateOverflow {
        region: RegionCoord,
        tile: RegionTile,
    },
    InvalidOverlayShape {
        position: MapPosition,
        shape: u8,
    },
    InvalidOverlayRotation {
        position: MapPosition,
        rotation: u8,
    },
    InvalidLocKind {
        position: MapPosition,
        kind: u8,
    },
    InvalidObjectOrientation {
        position: MapPosition,
        orientation: u8,
    },
    Snapshot(RegionSnapshotError),
}

impl From<RegionSnapshotError> for RegionAdaptError {
    fn from(error: RegionSnapshotError) -> Self {
        Self::Snapshot(error)
    }
}

/// Convert one decoded source region into the editor's canonical semantic snapshot.
///
/// Final height values are resolved with the same cache-agnostic terrain-load semantics used by
/// scene construction. No renderer geometry, picker state, or GPU data participates in this
/// conversion.
pub fn adapt_region_map(map: &RegionMap) -> Result<MapRegionSnapshot, RegionAdaptError> {
    let region = map.locations.region();
    let origin = world_tile(
        region,
        RegionTile::new(0, 0).unwrap_or_else(|| unreachable!("region-local origin is valid")),
    )?;
    let mut grid = TerrainLoadGrid::new(REGION_TILES, REGION_TILES)
        .unwrap_or_else(|error| unreachable!("64x64 terrain grid is valid: {error:?}"));
    grid.load_region(&map.terrain, (0, 0), (origin.x, origin.y), 0);

    let mut terrain = Vec::with_capacity(SCENE_PLANES * REGION_TILES * REGION_TILES);
    let mut heights = Vec::with_capacity(SCENE_PLANES * REGION_TILES * REGION_TILES);

    for plane_index in 0..SCENE_PLANES {
        let plane = SourcePlane::new(plane_index as u8)
            .unwrap_or_else(|| unreachable!("scene source plane is canonical"));
        for x in 0..REGION_TILES {
            for y in 0..REGION_TILES {
                let local = RegionTile::new(x as u8, y as u8)
                    .unwrap_or_else(|| unreachable!("terrain loop stays inside region"));
                let tile = world_tile(region, local)?;
                let position = MapPosition::new(tile, plane);
                let source = map.terrain.tile(plane_index, x, y);

                let underlay_id = source.underlay & FLOOR_ID_MASK;
                let underlay =
                    (underlay_id != 0).then(|| FloorUnderlayId::new(u32::from(underlay_id)));
                let overlay = match source.overlay {
                    None => None,
                    Some(source_overlay) => {
                        let id = source_overlay.raw_id & FLOOR_ID_MASK;
                        if id == 0 {
                            None
                        } else {
                            let Some(shape) = OverlayShape::new(source_overlay.shape) else {
                                return Err(RegionAdaptError::InvalidOverlayShape {
                                    position,
                                    shape: source_overlay.shape,
                                });
                            };
                            let Some(rotation) = OverlayRotation::new(source_overlay.rotation)
                            else {
                                return Err(RegionAdaptError::InvalidOverlayRotation {
                                    position,
                                    rotation: source_overlay.rotation,
                                });
                            };
                            Some(TerrainOverlay {
                                id: FloorOverlayId::new(u32::from(id)),
                                shape,
                                rotation,
                            })
                        }
                    }
                };

                terrain.push((
                    position,
                    TerrainTile {
                        underlay,
                        overlay,
                        flags: source.settings.unwrap_or(0),
                    },
                ));
                heights.push((
                    HeightVertexCoord::new(tile, plane),
                    grid.height(plane_index, x, y),
                ));
            }
        }
    }

    let mut objects = Vec::with_capacity(map.locations.len());
    for location in map.locations.locations() {
        let tile = world_tile(region, location.tile)?;
        let position = MapPosition::new(tile, location.source_plane);
        let Some(kind) = LocKind::new(location.loc_type) else {
            return Err(RegionAdaptError::InvalidLocKind {
                position,
                kind: location.loc_type,
            });
        };
        let Some(orientation) = ObjectOrientation::new(location.orientation) else {
            return Err(RegionAdaptError::InvalidObjectOrientation {
                position,
                orientation: location.orientation,
            });
        };
        objects.push(ObjectPlacement {
            object_id: location.object_id,
            position,
            kind,
            orientation,
        });
    }

    Ok(MapRegionSnapshot::from_parts(
        region, terrain, heights, objects,
    )?)
}

fn world_tile(region: RegionCoord, tile: RegionTile) -> Result<MapTile, RegionAdaptError> {
    MapTile::from_region(region, tile)
        .ok_or(RegionAdaptError::WorldCoordinateOverflow { region, tile })
}

#[cfg(test)]
mod tests {
    use super::*;
    use osrs_cache::decode::{DecodedLocation, DecodedLocations};
    use osrs_core::ids::ObjectId;
    use osrs_scene::terrain_load::{
        RegionTerrain, RegionTerrainTile, RegionTileHeight, RegionTileOverlay,
    };

    fn source_tile() -> RegionTerrainTile {
        RegionTerrainTile {
            height: RegionTileHeight::Default,
            overlay: None,
            settings: None,
            underlay: 0,
        }
    }

    fn terrain_with(edits: &[(usize, usize, usize, RegionTerrainTile)]) -> RegionTerrain {
        let mut tiles = vec![source_tile(); SCENE_PLANES * REGION_TILES * REGION_TILES];
        for &(plane, x, y, tile) in edits {
            let index = (plane * REGION_TILES + x) * REGION_TILES + y;
            tiles[index] = tile;
        }
        RegionTerrain::new(tiles)
            .unwrap_or_else(|error| unreachable!("complete region terrain: {error:?}"))
    }

    fn position(x: i32, y: i32, plane: u8) -> MapPosition {
        MapPosition::new(
            MapTile::new(x, y),
            SourcePlane::new(plane).unwrap_or_else(|| unreachable!("test plane is valid")),
        )
    }

    #[test]
    fn adapter_preserves_source_channels_and_resolves_heights_without_render_data() {
        let region = RegionCoord::new(50, 50);
        let plane_zero = RegionTerrainTile {
            height: RegionTileHeight::Explicit(7),
            overlay: Some(RegionTileOverlay {
                raw_id: 0x8007,
                shape: 4,
                rotation: 2,
            }),
            settings: Some(3),
            underlay: 5,
        };
        let plane_one = RegionTerrainTile {
            height: RegionTileHeight::Explicit(2),
            ..source_tile()
        };
        let map = RegionMap {
            terrain: terrain_with(&[(0, 0, 0, plane_zero), (1, 0, 0, plane_one)]),
            locations: DecodedLocations::with_locations(
                region,
                vec![DecodedLocation {
                    object_id: ObjectId::new(100),
                    tile: RegionTile::new(1, 2)
                        .unwrap_or_else(|| unreachable!("test tile is valid")),
                    source_plane: SourcePlane::new(1)
                        .unwrap_or_else(|| unreachable!("test plane is valid")),
                    loc_type: 10,
                    orientation: 3,
                }],
            ),
        };

        let snapshot = adapt_region_map(&map)
            .unwrap_or_else(|error| unreachable!("valid decoded region: {error:?}"));
        assert_eq!(snapshot.region(), region);
        assert_eq!(
            snapshot.height_vertex_count(),
            SCENE_PLANES * REGION_TILES * REGION_TILES
        );

        let mut document = osrs_edit::MapDocument::new();
        assert!(document.replace_region(&snapshot));

        let terrain = document.terrain_at(position(3200, 3200, 0));
        assert_eq!(terrain.underlay, Some(FloorUnderlayId::new(5)));
        let overlay = terrain
            .overlay
            .unwrap_or_else(|| unreachable!("overlay should be retained"));
        assert_eq!(overlay.id, FloorOverlayId::new(7));
        assert_eq!(overlay.shape.get(), 4);
        assert_eq!(overlay.rotation.get(), 2);
        assert_eq!(terrain.flags, 3);
        assert_eq!(
            document.height_at(HeightVertexCoord::new(
                MapTile::new(3200, 3200),
                SourcePlane::new(0).unwrap_or_else(|| unreachable!("plane zero is valid")),
            )),
            Some(-56)
        );
        assert_eq!(
            document.height_at(HeightVertexCoord::new(
                MapTile::new(3200, 3200),
                SourcePlane::new(1).unwrap_or_else(|| unreachable!("plane one is valid")),
            )),
            Some(-72)
        );

        assert!(document.contains_object(ObjectPlacement {
            object_id: ObjectId::new(100),
            position: position(3201, 3202, 1),
            kind: LocKind::new(10).unwrap_or_else(|| unreachable!("loc kind is valid")),
            orientation:
                ObjectOrientation::new(3).unwrap_or_else(|| unreachable!("orientation is valid")),
        }));
    }

    #[test]
    fn invalid_location_kind_is_rejected_instead_of_clamped() {
        let region = RegionCoord::new(50, 50);
        let map = RegionMap {
            terrain: terrain_with(&[]),
            locations: DecodedLocations::with_locations(
                region,
                vec![DecodedLocation {
                    object_id: ObjectId::new(100),
                    tile: RegionTile::new(0, 0)
                        .unwrap_or_else(|| unreachable!("test tile is valid")),
                    source_plane: SourcePlane::new(0)
                        .unwrap_or_else(|| unreachable!("test plane is valid")),
                    loc_type: 23,
                    orientation: 0,
                }],
            ),
        };

        assert_eq!(
            adapt_region_map(&map),
            Err(RegionAdaptError::InvalidLocKind {
                position: position(3200, 3200, 0),
                kind: 23,
            })
        );
    }
}
