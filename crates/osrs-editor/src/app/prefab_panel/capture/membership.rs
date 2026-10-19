//! Re-query ownership after floor corrections, preserving explicit occurrence overrides.
use super::*;
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn refresh(review: &mut Review) {
    let mut active = review.description.clone();
    active.floors.clear();
    for r in review
        .rooms
        .iter()
        .filter(|r| r.included && review.planes.contains(&r.plane))
    {
        active
            .floors
            .entry(r.plane)
            .or_default()
            .extend(r.tiles.iter().copied());
    }
    let active = osrs_scene::building_census::description::describe_floors(
        &review.inputs.area,
        &review.inputs.placements,
        &review.inputs.table,
        active,
    );
    let mut remap = BTreeMap::new();
    for (old, member) in active.members.iter().enumerate() {
        let p = member.placement;
        let Some(plane) = SourcePlane::new(p.plane) else {
            continue;
        };
        let Some(kind) = osrs_edit::LocKind::new(p.kind) else {
            continue;
        };
        let Some(orientation) = osrs_edit::ObjectOrientation::new(p.orientation) else {
            continue;
        };
        let object = ObjectPlacement {
            object_id: p.id,
            kind,
            orientation,
            position: MapPosition::new(MapTile::new(p.tile.0, p.tile.1), plane),
        };
        let index = if let Some(i) = review.objects.iter().position(|o| *o == object) {
            i
        } else {
            review.objects.push(object);
            review.included.push(false);
            review.description.members.push(member.clone());
            review.objects.len() - 1
        };
        remap.insert(old, index);
    }
    for (i, o) in review.objects.iter().enumerate() {
        review.included[i] = review
            .overrides
            .iter()
            .find(|(p, _)| p == o)
            .map_or_else(|| remap.values().any(|n| *n == i), |(_, yes)| *yes);
    }
    review.description.floors = active.floors;
    review.description.connections = active
        .connections
        .into_iter()
        .filter_map(|mut c| {
            c.from = *remap.get(&c.from)?;
            c.to = *remap.get(&c.to)?;
            Some(c)
        })
        .collect();
    review.description.boundaries = active
        .boundaries
        .into_iter()
        .filter_map(|mut b| {
            b.member = *remap.get(&b.member)?;
            Some(b)
        })
        .collect();
    review.description.portals = active
        .portals
        .into_iter()
        .filter_map(|mut p| {
            p.member = *remap.get(&p.member)?;
            Some(p)
        })
        .collect();
}
#[cfg(target_arch = "wasm32")]
pub(super) fn refresh(_: &mut Review) {}
