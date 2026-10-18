//! Destination foundation fitting. The complete result remains one ordinary reversible patch.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn fit(
    document: &MapDocument,
    prefab: &Prefab,
    spec: PlaceSpec,
    mut placed: PlacedPrefab,
) -> Result<PlacedPrefab, PrefabError> {
    let Some(building) = &prefab.building else {
        return Ok(placed);
    };
    let error = |e| PrefabError::Building(e);
    if !building.reviewed {
        return Err(error(BuildingPrefabError::Unreviewed));
    }
    let mut support = BTreeSet::new();
    let mut floor_vertices = BTreeSet::new();
    for tile in building.floors.iter().filter(|t| t.plane == 0) {
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            floor_vertices.insert((tile.x + dx, tile.y + dy));
        }
        let (x, y) = turn(
            i32::from(tile.x),
            i32::from(tile.y),
            prefab,
            spec.quarter_turns,
            false,
        );
        let coord = MapPosition::new(
            MapTile::new(spec.origin.x + x, spec.origin.y + y),
            spec.base_plane,
        );
        let ground = document.terrain_at(coord);
        let source_overlay = prefab
            .tiles
            .iter()
            .find(|t| (t.x, t.y, t.plane) == (tile.x, tile.y, tile.plane))
            .and_then(|t| t.overlay.as_ref().map(|o| o.id));
        if ground.flags & 2 != 0
            || ground
                .overlay
                .is_some_and(|o| Some(o.id.get()) != source_overlay)
        {
            return Err(error(BuildingPrefabError::IncompatibleGround(coord)));
        }
    }
    let source_levels: BTreeSet<_> = prefab
        .heights
        .iter()
        .filter(|h| h.plane == 0 && floor_vertices.contains(&(h.x, h.y)))
        .map(|h| h.value)
        .collect();
    if source_levels.len() != 1 {
        return Err(error(BuildingPrefabError::SteppedFoundation));
    }
    for h in building.support.iter().filter(|h| h.plane == 0) {
        let (x, y) = turn(
            i32::from(h.x),
            i32::from(h.y),
            prefab,
            spec.quarter_turns,
            true,
        );
        support.insert(MapTile::new(spec.origin.x + x, spec.origin.y + y));
    }
    let mut heights = Vec::new();
    for &corner in &support {
        let coord = HeightVertexCoord::new(corner, spec.base_plane);
        heights.push(
            document
                .height_at(coord)
                .ok_or_else(|| error(BuildingPrefabError::MissingHeight(coord)))?,
        );
    }
    heights.sort_unstable();
    let level = heights
        .get(heights.len() / 2)
        .copied()
        .ok_or_else(|| error(BuildingPrefabError::EmptyFloor))?
        .div_euclid(8)
        * 8;
    let delta = level
        .checked_sub(building.source_datum)
        .ok_or_else(|| error(BuildingPrefabError::CoordinateOverflow))?;
    let mut targets = BTreeMap::new();
    for &v in &support {
        for dx in -2..=2 {
            for dy in -2..=2 {
                let tile = MapTile::new(v.x + dx, v.y + dy);
                let distance = dx.abs().max(dy.abs());
                targets
                    .entry(tile)
                    .and_modify(|d: &mut i32| *d = (*d).min(distance))
                    .or_insert(distance);
            }
        }
    }
    let mut ops: Vec<_> = placed
        .patch
        .ops()
        .iter()
        .filter(|o| !matches!(o, MapOp::SetHeight { .. }))
        .cloned()
        .collect();
    for (corner, distance) in targets {
        let coord = HeightVertexCoord::new(corner, spec.base_plane);
        let before = document
            .height_at(coord)
            .ok_or_else(|| error(BuildingPrefabError::MissingHeight(coord)))?;
        let after = match distance {
            0 => level,
            1 => ((i64::from(before) + i64::from(level)) / 2) as i32,
            _ => before,
        };
        let after = if distance == 1 {
            after.div_euclid(8) * 8
        } else {
            after
        };
        if before != after {
            ops.push(MapOp::SetHeight {
                coord,
                before,
                after,
            });
        }
    }
    for h in prefab.heights.iter().filter(|h| h.plane > 0) {
        let (x, y) = turn(
            i32::from(h.x),
            i32::from(h.y),
            prefab,
            spec.quarter_turns,
            true,
        );
        let plane = plane_at(spec.base_plane, h.plane).ok_or(PrefabError::PlaneOverflow {
            base: spec.base_plane.index().get(),
            planes: prefab.planes,
        })?;
        let coord =
            HeightVertexCoord::new(MapTile::new(spec.origin.x + x, spec.origin.y + y), plane);
        let before = document
            .height_at(coord)
            .ok_or_else(|| error(BuildingPrefabError::MissingHeight(coord)))?;
        let after = h
            .value
            .checked_add(delta)
            .ok_or_else(|| error(BuildingPrefabError::CoordinateOverflow))?;
        if before != after {
            ops.push(MapOp::SetHeight {
                coord,
                before,
                after,
            });
        }
    }
    placed.patch = MapPatch::new(ops);
    document
        .validate_patch(&placed.patch)
        .map_err(PrefabError::Patch)?;
    placed.skipped_heights = 0;
    Ok(placed)
}

pub(super) fn validate_conflicts(
    document: &MapDocument,
    patch: &MapPatch,
    size_of: &dyn Fn(ObjectId, LocKind) -> (u8, u8),
) -> Result<(), PrefabError> {
    let removed: std::collections::HashSet<_> = patch
        .ops()
        .iter()
        .filter_map(|op| match op {
            MapOp::RemoveObject { placement } => Some(*placement),
            _ => None,
        })
        .collect();
    let added: Vec<_> = patch
        .ops()
        .iter()
        .filter_map(|op| match op {
            MapOp::AddObject { placement } => Some(*placement),
            _ => None,
        })
        .collect();
    for existing in document.object_entries() {
        if removed.contains(&existing) {
            continue;
        }
        let (w, h) = super::footprint(
            existing.object_id,
            existing.kind.get(),
            existing.orientation.get(),
            size_of,
        );
        let t = existing.position.tile;
        let touches_height=patch.ops().iter().any(|op|matches!(op,MapOp::SetHeight{coord,..} if coord.plane==existing.position.plane && (t.x..=t.x+w).contains(&coord.corner.x) && (t.y..=t.y+h).contains(&coord.corner.y)));
        let overlaps = added.iter().any(|new| {
            if new.position.plane != existing.position.plane {
                return false;
            }
            let (nw, nh) = super::footprint(
                new.object_id,
                new.kind.get(),
                new.orientation.get(),
                size_of,
            );
            let n = new.position.tile;
            t.x < n.x + nw && n.x < t.x + w && t.y < n.y + nh && n.y < t.y + h
        });
        if touches_height || overlaps {
            return Err(PrefabError::Building(BuildingPrefabError::Occupied(
                existing,
            )));
        }
    }
    Ok(())
}

pub(super) fn turn(mut x: i32, mut y: i32, prefab: &Prefab, turns: u8, vertex: bool) -> (i32, i32) {
    let (mut w, mut h) = (i32::from(prefab.width), i32::from(prefab.height));
    for _ in 0..turns & 3 {
        (x, y) = (y, w - i32::from(!vertex) - x);
        (w, h) = (h, w);
    }
    (x, y)
}
