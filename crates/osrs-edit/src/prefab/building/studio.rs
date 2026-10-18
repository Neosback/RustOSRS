//! Sparse scene save relative to a pristine stage baseline.
use super::*;
/// Capture Studio edits against its pristine pad. Original authored channels equal to the pad
/// remain present; unrelated pad channels are excluded. The source prefab supplies ownership.
pub fn capture_prefab_over(
    scene: &MapDocument,
    pad: &MapDocument,
    spec: CaptureSpec,
    original: &Prefab,
) -> Result<Prefab, PrefabError> {
    let mut p = capture_prefab(scene, spec, original.name.clone(), original.source.clone())?;
    let world =
        |x: u16, y: u16| MapTile::new(spec.origin.x + i32::from(x), spec.origin.y + i32::from(y));
    p.tiles.retain(|t| {
        let Some(plane) = plane_at(spec.base_plane, t.plane) else {
            return false;
        };
        let c = MapPosition::new(world(t.x, t.y), plane);
        scene.terrain_at(c) != pad.terrain_at(c)
            || original
                .tiles
                .iter()
                .any(|v| (v.x, v.y, v.plane) == (t.x, t.y, t.plane))
    });
    for original_tile in &original.tiles {
        if !p
            .tiles
            .iter()
            .any(|t| (t.x, t.y, t.plane) == (original_tile.x, original_tile.y, original_tile.plane))
        {
            let plane =
                plane_at(spec.base_plane, original_tile.plane).ok_or(PrefabError::Nothing)?;
            let t = scene.terrain_at(MapPosition::new(
                world(original_tile.x, original_tile.y),
                plane,
            ));
            p.tiles.push(PrefabTile {
                x: original_tile.x,
                y: original_tile.y,
                plane: original_tile.plane,
                underlay: t.underlay.map(FloorUnderlayId::get),
                overlay: t.overlay.map(|o| PrefabOverlay {
                    id: o.id.get(),
                    shape: o.shape.get(),
                    rotation: o.rotation.get(),
                }),
                flags: t.flags,
            });
        }
    }
    p.tiles.sort_by_key(|t| (t.plane, t.y, t.x));
    p.heights.retain(|h| {
        let Some(plane) = plane_at(spec.base_plane, h.plane) else {
            return false;
        };
        let c = HeightVertexCoord::new(world(h.x, h.y), plane);
        scene.height_at(c) != pad.height_at(c)
            || original
                .heights
                .iter()
                .any(|v| (v.x, v.y, v.plane) == (h.x, h.y, h.plane))
    });
    p.tags = original.tags.clone();
    p.notes = original.notes.clone();
    p.building = original.building.clone();
    if let Some(b) = &mut p.building {
        // Object edits can invalidate occurrence indices. Human labels survive; derived relations
        // must be recomputed rather than silently pointing at a different occurrence.
        if p.objects != original.objects {
            b.relations.clear();
            b.labels
                .retain(|key, _| !key.starts_with("inferred:member:"));
            b.reviewed = false;
        }
    }
    if let Some(b) = &mut p.building {
        p.tiles.retain(|t| {
            b.floors
                .iter()
                .any(|f| (f.x, f.y, f.plane) == (t.x, t.y, t.plane))
        });
        for c in &b.support {
            if !p
                .heights
                .iter()
                .any(|h| (h.x, h.y, h.plane) == (c.x, c.y, c.plane))
            {
                let plane = plane_at(spec.base_plane, c.plane).ok_or(PrefabError::Nothing)?;
                let coord = HeightVertexCoord::new(world(c.x, c.y), plane);
                let value = scene
                    .height_at(coord)
                    .ok_or_else(|| building_error(BuildingPrefabError::MissingHeight(coord)))?;
                p.heights.push(PrefabHeight {
                    x: c.x,
                    y: c.y,
                    plane: c.plane,
                    value,
                });
            }
        }
        for c in &b.floors {
            if !p
                .tiles
                .iter()
                .any(|t| (t.x, t.y, t.plane) == (c.x, c.y, c.plane))
            {
                let plane = plane_at(spec.base_plane, c.plane).ok_or(PrefabError::Nothing)?;
                let t = scene.terrain_at(MapPosition::new(world(c.x, c.y), plane));
                p.tiles.push(PrefabTile {
                    x: c.x,
                    y: c.y,
                    plane: c.plane,
                    underlay: t.underlay.map(FloorUnderlayId::get),
                    overlay: t.overlay.map(|o| PrefabOverlay {
                        id: o.id.get(),
                        shape: o.shape.get(),
                        rotation: o.rotation.get(),
                    }),
                    flags: t.flags,
                });
            }
        }
        let corners: BTreeSet<_> = b
            .floors
            .iter()
            .filter(|f| f.plane == 0)
            .flat_map(|f| {
                [
                    (f.x, f.y),
                    (f.x + 1, f.y),
                    (f.x, f.y + 1),
                    (f.x + 1, f.y + 1),
                ]
            })
            .collect();
        let mut levels: Vec<_> = p
            .heights
            .iter()
            .filter(|h| h.plane == 0 && corners.contains(&(h.x, h.y)))
            .map(|h| h.value)
            .collect();
        levels.sort_unstable();
        if let Some(datum) = levels.get(levels.len() / 2) {
            b.source_datum = *datum;
        }
    }
    p.tiles.sort_by_key(|t| (t.plane, t.y, t.x));
    p.heights.sort_by_key(|h| (h.plane, h.y, h.x));
    p.validate()?;
    Ok(p)
}
