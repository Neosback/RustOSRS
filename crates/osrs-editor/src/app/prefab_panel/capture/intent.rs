//! Convert reviewed ownership to the canonical sparse capture intent.
use super::*;
pub(super) fn intent(
    review: &Review,
    document: &MapDocument,
) -> Result<BuildingCaptureSpec, String> {
    let floors: BTreeSet<_> = review
        .rooms
        .iter()
        .filter(|r| r.included && review.planes.contains(&r.plane))
        .flat_map(|r| {
            r.tiles
                .iter()
                .filter_map(|&t| SourcePlane::new(r.plane).map(|p| MapPosition::new(t, p)))
        })
        .collect();
    let included: Vec<_> = review
        .included
        .iter()
        .enumerate()
        .map(|(i, &yes)| {
            yes && review
                .description
                .members
                .get(i)
                .is_some_and(|m| review.options.includes(m.role))
        })
        .collect();
    let objects: Vec<_> = review
        .objects
        .iter()
        .zip(&included)
        .filter_map(|(&o, &yes)| yes.then_some(o))
        .collect();
    let mut support = BTreeSet::new();
    for (tile, plane) in floors
        .iter()
        .map(|p| (p.tile, p.plane))
        .chain(objects.iter().map(|p| (p.position.tile, p.position.plane)))
    {
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            let coord = HeightVertexCoord::new(MapTile::new(tile.x + dx, tile.y + dy), plane);
            if document.height_at(coord).is_none() {
                return Err(format!(
                    "Missing height at {coord:?}; load the surrounding map."
                ));
            }
            support.insert(coord);
        }
    }
    for object in &objects {
        let &(w, h) = review
            .object_sizes
            .get(&object.object_id)
            .ok_or("Missing object dimensions")?;
        let (w, h) = if object.orientation.get() & 1 == 1 {
            (h, w)
        } else {
            (w, h)
        };
        for dx in 0..=w {
            for dy in 0..=h {
                let c = HeightVertexCoord::new(
                    MapTile::new(
                        object.position.tile.x + i32::from(dx),
                        object.position.tile.y + i32::from(dy),
                    ),
                    object.position.plane,
                );
                if document.height_at(c).is_none() {
                    return Err(format!("Missing model height samples at {c:?}"));
                }
                support.insert(c);
            }
        }
    }
    let mut remap = BTreeMap::new();
    let mut index = 0;
    for (old, &yes) in included.iter().enumerate() {
        if yes {
            remap.insert(old, index);
            index += 1;
        }
    }
    let relations = review
        .description
        .connections
        .iter()
        .filter_map(|r| {
            Some(PrefabRelation {
                from: *remap.get(&r.from)?,
                to: *remap.get(&r.to)?,
                kind: format!("{:?}", r.kind),
                evidence: format!("{:?}", r.evidence),
                human: false,
            })
        })
        .collect();
    let mut labels = review.labels.clone();
    for (old, new) in remap {
        if let Some(m) = review.description.members.get(old) {
            labels.insert(
                format!("inferred:member:{new}:role"),
                format!("{:?}", m.role),
            );
            labels.insert(
                format!("inferred:member:{new}:evidence"),
                format!("{:?}", m.evidence),
            );
        }
    }
    Ok(BuildingCaptureSpec {
        floors,
        support,
        objects,
        object_sizes: review.object_sizes.clone(),
        provenance: review.provenance.clone(),
        relations,
        labels,
        reviewed: review.confirmed,
    })
}
