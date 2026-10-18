//! Save the scene through the existing prefab library, excluding its pristine pad.
use super::*;
use osrs_edit::{CaptureSpec, capture_prefab_over};
pub(super) fn expand_bounds_to_fit(studio: &mut Studio) -> Result<bool, String> {
    let pad = studio.pad.as_ref().ok_or("The scene is still loading")?;
    let scene = studio.stage.viewer().session.document().clone();
    let mut min_x = studio.origin.x;
    let mut min_y = studio.origin.y;
    let mut max_x = studio.origin.x + i32::from(studio.original.width);
    let mut max_y = studio.origin.y + i32::from(studio.original.height);
    let mut max_plane = studio.original.planes.max(1);
    for p in scene.object_entries() {
        let size = studio
            .original
            .building
            .as_ref()
            .and_then(|b| b.object_sizes.get(&p.object_id.get()).copied())
            .or_else(|| {
                studio
                    .stage
                    .viewer()
                    .placer
                    .known_object_size(p.object_id.get())
                    .map(|(w, h)| (u16::from(w), u16::from(h)))
            })
            .unwrap_or((1, 1));
        let (w, h) = if p.orientation.get() & 1 == 1 {
            (i32::from(size.1), i32::from(size.0))
        } else {
            (i32::from(size.0), i32::from(size.1))
        };
        min_x = min_x.min(p.position.tile.x);
        min_y = min_y.min(p.position.tile.y);
        max_x = max_x.max(p.position.tile.x + w);
        max_y = max_y.max(p.position.tile.y + h);
        max_plane = max_plane.max(p.position.plane.index().get() + 1);
    }
    for (p, t) in scene.terrain_entries() {
        if t != pad.terrain_at(p) {
            min_x = min_x.min(p.tile.x);
            min_y = min_y.min(p.tile.y);
            max_x = max_x.max(p.tile.x + 1);
            max_y = max_y.max(p.tile.y + 1);
            max_plane = max_plane.max(p.plane.index().get() + 1);
        }
    }
    for (c, h) in scene.height_entries() {
        if Some(h) != pad.height_at(c) {
            min_x = min_x.min(c.corner.x);
            min_y = min_y.min(c.corner.y);
            max_x = max_x.max(c.corner.x);
            max_y = max_y.max(c.corner.y);
            max_plane = max_plane.max(c.plane.index().get() + 1);
        }
    }
    let new_width = u16::try_from((max_x - min_x).max(1))
        .map_err(|_| "Expanded prefab width exceeds limits")?;
    let new_height = u16::try_from((max_y - min_y).max(1))
        .map_err(|_| "Expanded prefab height exceeds limits")?;
    if min_x == studio.origin.x
        && min_y == studio.origin.y
        && new_width == studio.original.width
        && new_height == studio.original.height
        && max_plane == studio.original.planes
    {
        return Ok(false);
    }
    let dx = u16::try_from(studio.origin.x - min_x).unwrap_or(0);
    let dy = u16::try_from(studio.origin.y - min_y).unwrap_or(0);
    if dx > 0 || dy > 0 {
        for t in &mut studio.original.tiles {
            t.x += dx;
            t.y += dy;
        }
        for h in &mut studio.original.heights {
            h.x += dx;
            h.y += dy;
        }
        for o in &mut studio.original.objects {
            o.x += dx;
            o.y += dy;
        }
        if let Some(b) = &mut studio.original.building {
            for f in &mut b.floors {
                f.x += dx;
                f.y += dy;
            }
            for s in &mut b.support {
                s.x += dx;
                s.y += dy;
            }
        }
    }
    studio.origin = MapTile::new(min_x, min_y);
    studio.original.width = new_width;
    studio.original.height = new_height;
    studio.original.planes = max_plane.min(4);
    studio.session.metadata_changed();
    Ok(true)
}

pub(super) fn save(
    studio: &mut Studio,
    parent: &mut EditorApp,
    as_new: bool,
) -> Result<(), String> {
    if studio.auto_own_painted_floors {
        expand_bounds_to_fit(studio)?;
    }
    let pad = studio.pad.as_ref().ok_or("The scene is still loading")?;
    let scene = studio.stage.viewer().session.document().clone();
    let max_x = studio.origin.x + i32::from(studio.original.width);
    let max_y = studio.origin.y + i32::from(studio.original.height);
    if scene.object_entries().iter().any(|p| {
        p.position.tile.x < studio.origin.x
            || p.position.tile.y < studio.origin.y
            || p.position.tile.x >= max_x
            || p.position.tile.y >= max_y
            || p.position.plane.index().get() >= studio.original.planes
    }) {
        return Err(
            "A piece is outside the prefab boundary. Move it inside before saving this scene."
                .into(),
        );
    }
    let outside_tile = |tile: MapTile, plane: SourcePlane, vertex: bool| {
        tile.x < studio.origin.x
            || tile.y < studio.origin.y
            || tile.x >= max_x + i32::from(vertex)
            || tile.y >= max_y + i32::from(vertex)
            || plane.index().get() >= studio.original.planes
    };
    if scene
        .terrain_entries()
        .any(|(p, t)| outside_tile(p.tile, p.plane, false) && t != pad.terrain_at(p))
        || scene
            .height_entries()
            .any(|(c, h)| outside_tile(c.corner, c.plane, true) && Some(h) != pad.height_at(c))
    {
        return Err("Terrain edits extend outside this prefab's saved bounds or planes. Undo those edits before saving.".into());
    }
    let mut source = studio.original.clone();
    if let Some(b) = &mut source.building {
        for object in scene.object_entries() {
            if let std::collections::btree_map::Entry::Vacant(entry) =
                b.object_sizes.entry(object.object_id.get())
            {
                let (w, h) = studio
                    .stage
                    .viewer()
                    .placer
                    .known_object_size(object.object_id.get())
                    .ok_or("Wait for object definitions before saving")?;
                entry.insert((u16::from(w), u16::from(h)));
            }
        }
    }
    if let Some(b) = &mut source.building {
        if studio.auto_own_painted_floors {
            for (p, t) in scene.terrain_entries() {
                if !outside_tile(p.tile, p.plane, false) && t != pad.terrain_at(p) {
                    let cell = osrs_edit::PrefabCell {
                        x: (p.tile.x - studio.origin.x) as u16,
                        y: (p.tile.y - studio.origin.y) as u16,
                        plane: p.plane.index().get(),
                    };
                    if !b.floors.contains(&cell) {
                        b.floors.push(cell);
                    }
                }
            }
            b.floors.sort();
            b.floors.dedup();
        }
        let changed_objects = scene.object_entries().len() != source.objects.len()
            || studio.auto_own_painted_floors
            || source.objects.iter().any(|o| {
                !scene.object_entries().iter().any(|p| {
                    p.object_id.get() == o.id
                        && p.kind.get() == o.kind
                        && p.orientation.get() == o.orientation
                        && p.position.tile
                            == MapTile::new(
                                studio.origin.x + i32::from(o.x),
                                studio.origin.y + i32::from(o.y),
                            )
                        && p.position.plane.index().get() == o.plane
                })
            });
        if changed_objects {
            let mut support = std::collections::BTreeSet::new();
            for f in &b.floors {
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    support.insert(osrs_edit::PrefabCell {
                        x: f.x + dx,
                        y: f.y + dy,
                        plane: f.plane,
                    });
                }
            }
            for o in scene.object_entries() {
                let (w, h) = b.object_sizes[&o.object_id.get()];
                let (w, h) = if o.orientation.get() & 1 == 1 {
                    (h, w)
                } else {
                    (w, h)
                };
                let (x, y) = (
                    (o.position.tile.x - studio.origin.x) as u16,
                    (o.position.tile.y - studio.origin.y) as u16,
                );
                if u32::from(x) + u32::from(w) > u32::from(source.width)
                    || u32::from(y) + u32::from(h) > u32::from(source.height)
                {
                    return Err("An object's model height samples extend outside the prefab boundary. Move it inward before saving.".into());
                }
                for dx in 0..=w {
                    for dy in 0..=h {
                        support.insert(osrs_edit::PrefabCell {
                            x: x + dx,
                            y: y + dy,
                            plane: o.position.plane.index().get(),
                        });
                    }
                }
            }
            b.support = support.into_iter().collect();
        }
        if scene.terrain_entries().any(|(p, t)| {
            let local = (
                (p.tile.x - studio.origin.x) as u16,
                (p.tile.y - studio.origin.y) as u16,
                p.plane.index().get(),
            );
            t != pad.terrain_at(p)
                && !b.floors.iter().any(|f| (f.x, f.y, f.plane) == local)
                && !source.tiles.iter().any(|t| (t.x, t.y, t.plane) == local)
        }) {
            return Err("Painted terrain is outside the building's floor ownership. Include it in Relationships or undo the paint before saving.".into());
        }
    }
    let mut prefab = capture_prefab_over(
        &scene,
        pad,
        CaptureSpec {
            origin: studio.origin,
            width: studio.original.width,
            height: studio.original.height,
            base_plane: SourcePlane::new(0).ok_or("plane")?,
            planes: studio.original.planes,
            terrain: true,
            heights: true,
            objects: true,
        },
        &source,
    )
    .map_err(|e| e.to_string())?;
    // Saving is a human review of the edited layout. Old inferred occurrence links are already
    // removed by capture_prefab_over when object placements changed.
    if let Some(b) = &mut prefab.building {
        b.reviewed = true;
    }
    if as_new {
        prefab.name = format!("{} copy", prefab.name);
        studio.file = parent.prefabs.add(prefab.clone())?;
    } else {
        parent.prefabs.replace_if_unchanged(
            &studio.file,
            &studio.library_baseline,
            prefab.clone(),
        )?;
    }
    studio
        .stage
        .viewer()
        .prefabs
        .sync_entries_from(&parent.prefabs);
    studio.library_baseline = prefab.clone();
    studio.original = prefab;
    studio
        .session
        .mark_saved(studio.stage.viewer().session.edit_revision());
    studio.problem = None;
    Ok(())
}
