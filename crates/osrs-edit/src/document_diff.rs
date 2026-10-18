//! Infrastructure snapshot projection, independent of tool actions or history ownership.
use crate::{HeightVertexCoord, MapDocument, MapOp, MapPatch};
use std::collections::{BTreeSet, HashSet};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeightDomainChanged(pub HeightVertexCoord);
/// Plan an exact snapshot transition where both documents have the same explicit height domain.
/// This is an infrastructure adapter; the Lab's agent action enum deliberately does not expose it.
pub fn plan_document_sync(
    before: &MapDocument,
    after: &MapDocument,
) -> Result<MapPatch, HeightDomainChanged> {
    let mut ops = Vec::new();
    let tiles: BTreeSet<_> = before
        .terrain_entries()
        .chain(after.terrain_entries())
        .map(|(c, _)| c)
        .collect();
    for coord in tiles {
        let a = before.terrain_at(coord);
        let b = after.terrain_at(coord);
        if a != b {
            ops.push(MapOp::SetTerrain {
                coord,
                before: a,
                after: b,
            });
        }
    }
    let vertices: BTreeSet<_> = before
        .height_entries()
        .chain(after.height_entries())
        .map(|(c, _)| c)
        .collect();
    for coord in vertices {
        match (before.height_at(coord), after.height_at(coord)) {
            (Some(a), Some(b)) => {
                if a != b {
                    ops.push(MapOp::SetHeight {
                        coord,
                        before: a,
                        after: b,
                    });
                }
            }
            _ => return Err(HeightDomainChanged(coord)),
        }
    }
    let a = before.object_entries();
    let b = after.object_entries();
    let aset: HashSet<_> = a.iter().copied().collect();
    let bset: HashSet<_> = b.iter().copied().collect();
    ops.extend(
        a.into_iter()
            .filter(|o| !bset.contains(o))
            .map(|placement| MapOp::RemoveObject { placement }),
    );
    ops.extend(
        b.into_iter()
            .filter(|o| !aset.contains(o))
            .map(|placement| MapOp::AddObject { placement }),
    );
    Ok(MapPatch::new(ops))
}
#[cfg(test)]
mod tests {
    use super::*;
    use osrs_core::coords::{MapTile, SourcePlane};
    #[test]
    fn snapshot_sync_is_exact_and_reversible_and_rejects_missing_height_domains() {
        let p = SourcePlane::new(0).unwrap_or_else(|| unreachable!());
        let c = HeightVertexCoord::new(MapTile::new(1, 1), p);
        let mut a = MapDocument::from_parts([], [(c, 0)], []);
        let b = MapDocument::from_parts([], [(c, 8)], []);
        let patch = plan_document_sync(&a, &b).unwrap_or_else(|e| panic!("{e:?}"));
        let original = a.clone();
        a.apply_patch(&patch).unwrap_or_else(|e| panic!("{e:?}"));
        assert_eq!(a, b);
        a.apply_patch(&patch.inverse())
            .unwrap_or_else(|e| panic!("{e:?}"));
        assert_eq!(a, original);
        assert_eq!(
            plan_document_sync(&a, &MapDocument::new()),
            Err(HeightDomainChanged(c))
        );
    }
}
