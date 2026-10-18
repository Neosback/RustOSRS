//! Rectangle capture and patch conversion for portable prefabs.
use super::*;

/// Copy a rectangle of the map into a prefab. Terrain tiles equal to the default are left out,
/// and a height vertex the document has no value for is left out; objects are taken by the tile
/// their placement is stored on.
pub fn capture_prefab(
    document: &MapDocument,
    spec: CaptureSpec,
    name: impl Into<String>,
    source: PrefabSource,
) -> Result<Prefab, PrefabError> {
    let side = 1..=MAX_PREFAB_SIDE;
    if !side.contains(&spec.width)
        || !side.contains(&spec.height)
        || !(1..=4).contains(&spec.planes)
    {
        return Err(PrefabError::BadSize {
            width: spec.width,
            height: spec.height,
            planes: spec.planes,
        });
    }
    if plane_at(spec.base_plane, spec.planes - 1).is_none() {
        return Err(PrefabError::PlaneOverflow {
            base: spec.base_plane.index().get(),
            planes: spec.planes,
        });
    }
    let (min_x, min_y) = (spec.origin.x, spec.origin.y);
    let (max_x, max_y) = (
        min_x + i32::from(spec.width),
        min_y + i32::from(spec.height),
    );
    let mut prefab = Prefab {
        format: PREFAB_FORMAT.to_owned(),
        version: PREFAB_VERSION,
        name: name.into(),
        tags: Vec::new(),
        notes: String::new(),
        source,
        width: spec.width,
        height: spec.height,
        planes: spec.planes,
        tiles: Vec::new(),
        heights: Vec::new(),
        objects: Vec::new(),
        building: None,
    };
    for offset in 0..spec.planes {
        let Some(plane) = plane_at(spec.base_plane, offset) else {
            continue;
        };
        if spec.terrain {
            for y in 0..spec.height {
                for x in 0..spec.width {
                    let tile = MapTile::new(min_x + i32::from(x), min_y + i32::from(y));
                    let terrain = document.terrain_at(MapPosition::new(tile, plane));
                    if terrain == TerrainTile::default() {
                        continue;
                    }
                    prefab.tiles.push(PrefabTile {
                        x,
                        y,
                        plane: offset,
                        underlay: terrain.underlay.map(FloorUnderlayId::get),
                        overlay: terrain.overlay.map(|overlay| PrefabOverlay {
                            id: overlay.id.get(),
                            shape: overlay.shape.get(),
                            rotation: overlay.rotation.get(),
                        }),
                        flags: terrain.flags,
                    });
                }
            }
        }
        if spec.heights {
            for y in 0..=spec.height {
                for x in 0..=spec.width {
                    let corner = MapTile::new(min_x + i32::from(x), min_y + i32::from(y));
                    if let Some(value) = document.height_at(HeightVertexCoord::new(corner, plane)) {
                        prefab.heights.push(PrefabHeight {
                            x,
                            y,
                            plane: offset,
                            value,
                        });
                    }
                }
            }
        }
        if spec.objects {
            for placement in objects_in_rect(document, (min_x, min_y), (max_x, max_y), plane) {
                let tile = placement.position.tile;
                prefab.objects.push(PrefabObject {
                    id: placement.object_id.get(),
                    x: (tile.x - min_x) as u16,
                    y: (tile.y - min_y) as u16,
                    plane: offset,
                    kind: placement.kind.get(),
                    orientation: placement.orientation.get(),
                });
            }
        }
    }
    prefab.tiles.sort_by_key(|t| (t.plane, t.y, t.x));
    prefab.heights.sort_by_key(|h| (h.plane, h.y, h.x));
    prefab
        .objects
        .sort_by_key(|o| (o.plane, o.y, o.x, o.kind, o.id, o.orientation));
    if prefab.tiles.is_empty() && prefab.heights.is_empty() && prefab.objects.is_empty() {
        return Err(PrefabError::Nothing);
    }
    prefab.validate()?;
    Ok(prefab)
}

/// Build a prefab from a generated or hand-made patch: the patch's added objects and terrain
/// results, relative to `origin`, with the planes it touches. The patch is not applied.
pub fn prefab_from_patch(
    patch: &MapPatch,
    origin: MapTile,
    base_plane: SourcePlane,
    width: u16,
    height: u16,
    name: impl Into<String>,
    source: PrefabSource,
) -> Result<Prefab, PrefabError> {
    let mut planes = 1u8;
    let rel = |tile: MapTile, plane: SourcePlane| -> Option<(u16, u16, u8)> {
        let x = u16::try_from(tile.x - origin.x).ok()?;
        let y = u16::try_from(tile.y - origin.y).ok()?;
        let p = plane.index().get().checked_sub(base_plane.index().get())?;
        Some((x, y, p))
    };
    let mut prefab = Prefab {
        format: PREFAB_FORMAT.to_owned(),
        version: PREFAB_VERSION,
        name: name.into(),
        tags: Vec::new(),
        notes: String::new(),
        source,
        width,
        height,
        planes: 1,
        tiles: Vec::new(),
        heights: Vec::new(),
        objects: Vec::new(),
        building: None,
    };
    let mut removed = HashSet::<ObjectPlacement>::new();
    for op in patch.ops() {
        if let MapOp::RemoveObject { placement } = op {
            removed.insert(*placement);
        }
    }
    for op in patch.ops() {
        match *op {
            MapOp::SetTerrain { coord, after, .. } => {
                let (x, y, plane) = rel(coord.tile, coord.plane).ok_or(PrefabError::OutOfBox {
                    what: "a tile",
                    x: 0,
                    y: 0,
                    plane: 0,
                })?;
                planes = planes.max(plane + 1);
                prefab.tiles.push(PrefabTile {
                    x,
                    y,
                    plane,
                    underlay: after.underlay.map(FloorUnderlayId::get),
                    overlay: after.overlay.map(|overlay: TerrainOverlay| PrefabOverlay {
                        id: overlay.id.get(),
                        shape: overlay.shape.get(),
                        rotation: overlay.rotation.get(),
                    }),
                    flags: after.flags,
                });
            }
            MapOp::SetHeight { coord, after, .. } => {
                let (x, y, plane) =
                    rel(coord.corner, coord.plane).ok_or(PrefabError::OutOfBox {
                        what: "a height",
                        x: 0,
                        y: 0,
                        plane: 0,
                    })?;
                planes = planes.max(plane + 1);
                prefab.heights.push(PrefabHeight {
                    x,
                    y,
                    plane,
                    value: after,
                });
            }
            MapOp::AddObject { placement } if !removed.contains(&placement) => {
                let (x, y, plane) = rel(placement.position.tile, placement.position.plane).ok_or(
                    PrefabError::OutOfBox {
                        what: "an object",
                        x: 0,
                        y: 0,
                        plane: 0,
                    },
                )?;
                planes = planes.max(plane + 1);
                prefab.objects.push(PrefabObject {
                    id: placement.object_id.get(),
                    x,
                    y,
                    plane,
                    kind: placement.kind.get(),
                    orientation: placement.orientation.get(),
                });
            }
            MapOp::AddObject { .. } | MapOp::RemoveObject { .. } => {}
        }
    }
    prefab.planes = planes;
    prefab.tiles.sort_by_key(|t| (t.plane, t.y, t.x));
    prefab.heights.sort_by_key(|h| (h.plane, h.y, h.x));
    prefab
        .objects
        .sort_by_key(|o| (o.plane, o.y, o.x, o.kind, o.id, o.orientation));
    if prefab.tiles.is_empty() && prefab.heights.is_empty() && prefab.objects.is_empty() {
        return Err(PrefabError::Nothing);
    }
    prefab.validate()?;
    Ok(prefab)
}
