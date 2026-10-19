//! Persist reviewed room structure in prefab-local coordinates, with occurrence remapping.
use super::*;
use osrs_edit::{Prefab, PrefabCell, PrefabPortal, PrefabRoom};

pub(super) fn apply(review: &Review, prefab: &mut Prefab) -> Result<(), String> {
    let osrs_edit::PrefabSource::Captured {
        tile_x,
        tile_y,
        plane: base,
    } = prefab.source
    else {
        return Ok(());
    };
    let local = |tile: MapTile, plane: u8| -> Option<PrefabCell> {
        Some(PrefabCell {
            x: u16::try_from(tile.x - tile_x).ok()?,
            y: u16::try_from(tile.y - tile_y).ok()?,
            plane: plane.checked_sub(base)?,
        })
    };
    let index_of = |index: usize| -> Option<usize> {
        let p = review.objects.get(index)?;
        let c = local(p.position.tile, p.position.plane.index().get())?;
        prefab.objects.iter().position(|o| {
            (o.id, o.kind, o.orientation, o.x, o.y, o.plane)
                == (
                    p.object_id.get(),
                    p.kind.get(),
                    p.orientation.get(),
                    c.x,
                    c.y,
                    c.plane,
                )
        })
    };
    let rooms: Vec<_> = review
        .rooms
        .iter()
        .filter(|r| r.included && review.planes.contains(&r.plane))
        .enumerate()
        .map(|(i, r)| {
            let cells = r
                .tiles
                .iter()
                .map(|t| local(*t, r.plane).ok_or("Room outside captured bounds"))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(PrefabRoom {
                id: i as u16,
                role: r.role.clone(),
                cells,
                human: r.tiles.first().is_some_and(|t| {
                    review
                        .labels
                        .contains_key(&format!("human:room:p{}:{}:{}", r.plane, t.x, t.y))
                }),
            })
        })
        .collect::<Result<_, String>>()?;
    let mut portals = Vec::new();
    for portal in &review.description.portals {
        let Some(object) = index_of(portal.member) else {
            continue;
        };
        let plane = review.objects[portal.member].position.plane.index().get();
        let Some(a) = local(portal.approaches[0], plane) else {
            continue;
        };
        let Some(b) = local(portal.approaches[1], plane) else {
            continue;
        };
        if [a, b]
            .iter()
            .any(|c| c.x >= prefab.width || c.y >= prefab.height)
        {
            continue;
        }
        let sides = [a, b].map(|c| rooms.iter().find(|r| r.cells.contains(&c)).map(|r| r.id));
        if sides == [None, None] || sides[0] == sides[1] {
            continue;
        }
        portals.push(PrefabPortal {
            object: Some(object),
            rooms: sides,
            approaches: [a, b],
            human: false,
        });
    }
    let relations = review
        .description
        .connections
        .iter()
        .filter_map(|r| {
            Some(PrefabRelation {
                from: index_of(r.from)?,
                to: index_of(r.to)?,
                kind: format!("{:?}", r.kind),
                evidence: format!("{:?}", r.evidence),
                human: false,
            })
        })
        .collect();
    if let Some(building) = &mut prefab.building {
        building.semantics.rooms = rooms;
        building.semantics.portals = portals;
        building.relations = relations;
    }
    prefab.validate().map_err(|e| e.to_string())
}
