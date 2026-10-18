//! Save the scene through the existing prefab library, excluding its pristine pad.
use super::*;
use osrs_edit::{CaptureSpec, capture_prefab_over};
pub(super) fn save(
    studio: &mut Studio,
    parent: &mut EditorApp,
    as_new: bool,
) -> Result<(), String> {
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
        let changed_objects = scene.object_entries().len() != source.objects.len()
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
        parent.prefabs.edit(&studio.file, |p| *p = prefab.clone())?;
    }
    studio.original = prefab;
    studio
        .session
        .mark_saved(studio.stage.viewer().session.edit_revision());
    studio.problem = None;
    Ok(())
}
