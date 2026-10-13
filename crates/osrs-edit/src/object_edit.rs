use crate::{MapOp, MapPatch, MapQuery, ObjectPlacement};
use std::collections::HashSet;

/// Why an object transaction cannot be planned against the current semantic map state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectEditError {
    /// A requested removal does not exist in the authoritative document.
    MissingRemoval { placement: ObjectPlacement },
    /// A requested addition already exists in the authoritative document.
    AdditionAlreadyPresent { placement: ObjectPlacement },
}

/// Build one deterministic semantic object transaction from exact placements removed and added.
///
/// The planner is intentionally independent from renderer/world-worker object slots. Exact
/// [`ObjectPlacement`] identity is the contract. Duplicate requests are collapsed because the
/// authoritative document stores placements as a set, and an exact remove+add pair cancels as a
/// semantic no-op.
///
/// Removals are emitted before additions so moves/turns are represented naturally as one reversible
/// transaction. Both groups are canonically ordered, making equivalent input sets produce identical
/// patches regardless of selection/input order.
pub fn build_object_patch(
    query: &impl MapQuery,
    removed: &[ObjectPlacement],
    added: &[ObjectPlacement],
) -> Result<MapPatch, ObjectEditError> {
    let mut removed = canonical_unique(removed);
    let mut added = canonical_unique(added);

    let removed_set: HashSet<_> = removed.iter().copied().collect();
    let added_set: HashSet<_> = added.iter().copied().collect();

    // An exact placement removed and then re-added leaves semantic state unchanged.
    removed.retain(|placement| !added_set.contains(placement));
    added.retain(|placement| !removed_set.contains(placement));

    for placement in &removed {
        if !query.contains_object(*placement) {
            return Err(ObjectEditError::MissingRemoval {
                placement: *placement,
            });
        }
    }
    for placement in &added {
        if query.contains_object(*placement) {
            return Err(ObjectEditError::AdditionAlreadyPresent {
                placement: *placement,
            });
        }
    }

    Ok(MapPatch::new(
        removed
            .into_iter()
            .map(|placement| MapOp::RemoveObject { placement })
            .chain(
                added
                    .into_iter()
                    .map(|placement| MapOp::AddObject { placement }),
            ),
    ))
}

fn canonical_unique(placements: &[ObjectPlacement]) -> Vec<ObjectPlacement> {
    let mut placements = placements.to_vec();
    placements.sort_unstable_by_key(placement_key);
    placements.dedup();
    placements
}

fn placement_key(placement: &ObjectPlacement) -> (u8, i32, i32, u32, u8, u8) {
    (
        placement.position.plane.index().get(),
        placement.position.tile.x,
        placement.position.tile.y,
        placement.object_id.get(),
        placement.kind.get(),
        placement.orientation.get(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LocKind, MapDocument, MapPosition, ObjectOrientation};
    use osrs_core::coords::{MapTile, SourcePlane};
    use osrs_core::ids::ObjectId;

    fn plane(value: u8) -> SourcePlane {
        SourcePlane::new(value).unwrap_or_else(|| unreachable!("test plane is valid"))
    }

    fn placement(
        object_id: u32,
        x: i32,
        y: i32,
        plane_value: u8,
        kind: u8,
        orientation: u8,
    ) -> ObjectPlacement {
        ObjectPlacement {
            object_id: ObjectId::new(object_id),
            position: MapPosition::new(MapTile::new(x, y), plane(plane_value)),
            kind: LocKind::new(kind).unwrap_or_else(|| unreachable!("test kind is valid")),
            orientation: ObjectOrientation::new(orientation)
                .unwrap_or_else(|| unreachable!("test orientation is valid")),
        }
    }

    #[test]
    fn delete_is_one_remove_operation() {
        let object = placement(100, 3200, 3200, 0, 10, 2);
        let document = MapDocument::from_parts([], [], [object]);

        let patch = build_object_patch(&document, &[object], &[])
            .unwrap_or_else(|error| unreachable!("valid delete: {error:?}"));

        assert_eq!(patch.ops(), &[MapOp::RemoveObject { placement: object }]);
    }

    #[test]
    fn duplicate_is_one_add_operation() {
        let source = placement(100, 3200, 3200, 0, 10, 2);
        let copy = placement(100, 3201, 3200, 0, 10, 2);
        let document = MapDocument::from_parts([], [], [source]);

        let patch = build_object_patch(&document, &[], &[copy])
            .unwrap_or_else(|error| unreachable!("valid duplicate: {error:?}"));

        assert_eq!(patch.ops(), &[MapOp::AddObject { placement: copy }]);
    }

    #[test]
    fn move_is_remove_then_add_in_one_patch() {
        let before = placement(100, 3200, 3200, 0, 10, 2);
        let after = placement(100, 3205, 3198, 0, 10, 2);
        let document = MapDocument::from_parts([], [], [before]);

        let patch = build_object_patch(&document, &[before], &[after])
            .unwrap_or_else(|error| unreachable!("valid move: {error:?}"));

        assert_eq!(
            patch.ops(),
            &[
                MapOp::RemoveObject { placement: before },
                MapOp::AddObject { placement: after },
            ]
        );
    }

    #[test]
    fn exact_remove_and_add_cancel_to_semantic_no_op() {
        let object = placement(100, 3200, 3200, 0, 10, 2);
        let document = MapDocument::from_parts([], [], [object]);

        let patch = build_object_patch(&document, &[object], &[object])
            .unwrap_or_else(|error| unreachable!("exact replacement is a no-op: {error:?}"));

        assert!(patch.is_empty());
    }

    #[test]
    fn duplicate_requests_collapse_under_set_semantics() {
        let source = placement(100, 3200, 3200, 0, 10, 2);
        let copy = placement(100, 3201, 3200, 0, 10, 2);
        let document = MapDocument::from_parts([], [], [source]);

        let patch = build_object_patch(&document, &[], &[copy, copy, copy])
            .unwrap_or_else(|error| unreachable!("duplicate requests collapse: {error:?}"));

        assert_eq!(patch.ops(), &[MapOp::AddObject { placement: copy }]);
    }

    #[test]
    fn missing_removal_is_rejected_before_patch_application() {
        let missing = placement(100, 3200, 3200, 0, 10, 2);
        let document = MapDocument::new();

        assert_eq!(
            build_object_patch(&document, &[missing], &[]),
            Err(ObjectEditError::MissingRemoval { placement: missing })
        );
    }

    #[test]
    fn already_present_addition_is_rejected_before_patch_application() {
        let present = placement(100, 3200, 3200, 0, 10, 2);
        let document = MapDocument::from_parts([], [], [present]);

        assert_eq!(
            build_object_patch(&document, &[], &[present]),
            Err(ObjectEditError::AdditionAlreadyPresent { placement: present })
        );
    }

    #[test]
    fn equivalent_input_sets_produce_identical_canonical_operation_order() {
        let a = placement(300, 64, -1, 2, 10, 3);
        let b = placement(100, -64, 128, 0, 22, 1);
        let c = placement(200, 0, 0, 1, 0, 2);
        let a2 = placement(300, 65, -1, 2, 10, 3);
        let b2 = placement(100, -63, 128, 0, 22, 1);
        let c2 = placement(200, 1, 0, 1, 0, 2);
        let document = MapDocument::from_parts([], [], [a, b, c]);

        let first = build_object_patch(&document, &[a, c, b], &[c2, a2, b2])
            .unwrap_or_else(|error| unreachable!("valid object transaction: {error:?}"));
        let second = build_object_patch(&document, &[b, a, c], &[b2, c2, a2])
            .unwrap_or_else(|error| unreachable!("valid object transaction: {error:?}"));

        assert_eq!(first, second);
        assert_eq!(first.len(), 6);
    }

    #[test]
    fn turn_changes_only_exact_orientation_identity() {
        let before = placement(100, 3200, 3200, 0, 10, 0);
        let after = placement(100, 3200, 3200, 0, 10, 1);
        let document = MapDocument::from_parts([], [], [before]);

        let patch = build_object_patch(&document, &[before], &[after])
            .unwrap_or_else(|error| unreachable!("valid turn: {error:?}"));
        let mut applied = document.clone();
        assert!(applied.apply_patch(&patch).is_ok());

        assert!(!applied.contains_object(before));
        assert!(applied.contains_object(after));
    }
}
