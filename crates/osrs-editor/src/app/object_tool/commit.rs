use osrs_core::{
    coords::{MapTile, SourcePlane},
    ids::ObjectId,
};
use osrs_edit::{
    LocKind, MapOp, MapPatch, MapPosition, MapQuery, ObjectEditError, ObjectOrientation,
    ObjectPlacement, build_object_patch,
};
use osrs_world::{LocEdit, LocSpec};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ObjectCommitError {
    InvalidPlane { plane: u8, tile: (i32, i32) },
    InvalidLocKind { kind: u8, tile: (i32, i32) },
    InvalidOrientation { orientation: u8, tile: (i32, i32) },
    Semantic(ObjectEditError),
}

impl From<ObjectEditError> for ObjectCommitError {
    fn from(error: ObjectEditError) -> Self {
        Self::Semantic(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ObjectCommitPlan {
    pub patch: MapPatch,
    pub removed: Vec<LocSpec>,
    pub added: Vec<LocSpec>,
}

/// Lightweight global-history route for one semantic Object transaction.
///
/// These exact identities are projection metadata only. `EditorSession::history` remains the
/// authoritative before/after state and is always transitioned before this route is used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ObjectHistoryRoute {
    removed: Vec<LocSpec>,
    added: Vec<LocSpec>,
}

impl ObjectHistoryRoute {
    pub(super) fn new(removed: Vec<LocSpec>, added: Vec<LocSpec>) -> Self {
        Self { removed, added }
    }

    pub(super) fn object_count(&self) -> usize {
        self.removed.len().max(self.added.len())
    }

    #[cfg(test)]
    pub(super) fn removed(&self) -> &[LocSpec] {
        &self.removed
    }

    #[cfg(test)]
    pub(super) fn added(&self) -> &[LocSpec] {
        &self.added
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ObjectHistoryProjectionError {
    Adapter(ObjectCommitError),
    ExpectedAbsent { spec: LocSpec },
    ExpectedPresent { spec: LocSpec },
}

impl From<ObjectCommitError> for ObjectHistoryProjectionError {
    fn from(error: ObjectCommitError) -> Self {
        Self::Adapter(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ObjectHistoryProjection {
    pub edits: Vec<LocEdit>,
    pub reselect: Vec<LocSpec>,
}

pub(super) fn build_object_commit_plan(
    query: &impl MapQuery,
    removed: &[LocSpec],
    added: &[LocSpec],
) -> Result<ObjectCommitPlan, ObjectCommitError> {
    let removed_placements = adapt_specs(removed)?;
    let added_placements = adapt_specs(added)?;
    let patch = build_object_patch(query, &removed_placements, &added_placements)?;

    let mut canonical_removed = Vec::new();
    let mut canonical_added = Vec::new();
    for op in patch.ops() {
        match op {
            MapOp::RemoveObject { placement } => {
                canonical_removed.push(spec_from_placement(*placement));
            }
            MapOp::AddObject { placement } => {
                canonical_added.push(spec_from_placement(*placement));
            }
            MapOp::SetTerrain { .. } | MapOp::SetHeight { .. } => {
                unreachable!("object planner emits only object operations")
            }
        }
    }

    Ok(ObjectCommitPlan {
        patch,
        removed: canonical_removed,
        added: canonical_added,
    })
}

/// Build the worker projection for an already-completed semantic Object undo/redo.
///
/// `forward = false` projects an undo; `forward = true` projects a redo. The semantic document is
/// checked first so a stale/misordered global route cannot mutate the worker independently from the
/// authoritative session.
pub(super) fn build_object_history_projection(
    query: &impl MapQuery,
    route: &ObjectHistoryRoute,
    forward: bool,
) -> Result<ObjectHistoryProjection, ObjectHistoryProjectionError> {
    let (gone, came) = if forward {
        (&route.removed, &route.added)
    } else {
        (&route.added, &route.removed)
    };

    for &spec in gone {
        if query.contains_object(placement_from_spec(spec)?) {
            return Err(ObjectHistoryProjectionError::ExpectedAbsent { spec });
        }
    }
    for &spec in came {
        if !query.contains_object(placement_from_spec(spec)?) {
            return Err(ObjectHistoryProjectionError::ExpectedPresent { spec });
        }
    }

    let mut edits: Vec<LocEdit> = gone.iter().map(|spec| LocEdit::Remove(*spec)).collect();
    edits.extend(came.iter().map(|spec| LocEdit::Add(*spec)));
    Ok(ObjectHistoryProjection {
        edits,
        reselect: came.clone(),
    })
}

fn adapt_specs(specs: &[LocSpec]) -> Result<Vec<ObjectPlacement>, ObjectCommitError> {
    specs.iter().copied().map(placement_from_spec).collect()
}

fn placement_from_spec(spec: LocSpec) -> Result<ObjectPlacement, ObjectCommitError> {
    let Some(plane) = SourcePlane::new(spec.plane) else {
        return Err(ObjectCommitError::InvalidPlane {
            plane: spec.plane,
            tile: spec.tile,
        });
    };
    let Some(kind) = LocKind::new(spec.loc_type) else {
        return Err(ObjectCommitError::InvalidLocKind {
            kind: spec.loc_type,
            tile: spec.tile,
        });
    };
    let Some(orientation) = ObjectOrientation::new(spec.orientation) else {
        return Err(ObjectCommitError::InvalidOrientation {
            orientation: spec.orientation,
            tile: spec.tile,
        });
    };

    Ok(ObjectPlacement {
        object_id: ObjectId::new(spec.object_id),
        position: MapPosition::new(MapTile::new(spec.tile.0, spec.tile.1), plane),
        kind,
        orientation,
    })
}

fn spec_from_placement(placement: ObjectPlacement) -> LocSpec {
    LocSpec {
        object_id: placement.object_id.get(),
        tile: (placement.position.tile.x, placement.position.tile.y),
        plane: placement.position.plane.index().get(),
        loc_type: placement.kind.get(),
        orientation: placement.orientation.get(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::EditorSession;
    use osrs_edit::MapDocument;

    fn spec(object_id: u32, tile: (i32, i32), plane: u8, loc_type: u8, orientation: u8) -> LocSpec {
        LocSpec {
            object_id,
            tile,
            plane,
            loc_type,
            orientation,
        }
    }

    #[test]
    fn loc_spec_round_trip_is_exact_for_valid_semantic_values() {
        let original = spec(42, (-64, 129), 3, 22, 3);
        let placement = placement_from_spec(original)
            .unwrap_or_else(|error| unreachable!("valid LocSpec: {error:?}"));
        assert_eq!(spec_from_placement(placement), original);
    }

    #[test]
    fn invalid_plane_is_rejected_before_semantic_planning() {
        let invalid = spec(42, (10, 20), 4, 10, 0);
        assert_eq!(
            placement_from_spec(invalid),
            Err(ObjectCommitError::InvalidPlane {
                plane: 4,
                tile: (10, 20),
            })
        );
    }

    #[test]
    fn invalid_kind_is_rejected_before_semantic_planning() {
        let invalid = spec(42, (10, 20), 0, 23, 0);
        assert_eq!(
            placement_from_spec(invalid),
            Err(ObjectCommitError::InvalidLocKind {
                kind: 23,
                tile: (10, 20),
            })
        );
    }

    #[test]
    fn invalid_orientation_is_rejected_before_semantic_planning() {
        let invalid = spec(42, (10, 20), 0, 10, 4);
        assert_eq!(
            placement_from_spec(invalid),
            Err(ObjectCommitError::InvalidOrientation {
                orientation: 4,
                tile: (10, 20),
            })
        );
    }

    #[test]
    fn canonical_projection_is_derived_from_the_semantic_patch() {
        let source = spec(100, (3200, 3200), 0, 10, 1);
        let moved = spec(100, (3201, 3200), 0, 10, 1);
        let source_placement = placement_from_spec(source)
            .unwrap_or_else(|error| unreachable!("valid source: {error:?}"));
        let document = MapDocument::from_parts([], [], [source_placement]);

        let plan = build_object_commit_plan(&document, &[source, source], &[moved, moved])
            .unwrap_or_else(|error| unreachable!("valid move: {error:?}"));

        assert_eq!(plan.removed, vec![source]);
        assert_eq!(plan.added, vec![moved]);
        assert_eq!(plan.patch.len(), 2);
    }

    #[test]
    fn exact_remove_add_no_op_produces_no_worker_projection() {
        let object = spec(100, (3200, 3200), 0, 10, 1);
        let placement = placement_from_spec(object)
            .unwrap_or_else(|error| unreachable!("valid object: {error:?}"));
        let document = MapDocument::from_parts([], [], [placement]);

        let plan = build_object_commit_plan(&document, &[object], &[object])
            .unwrap_or_else(|error| unreachable!("semantic no-op: {error:?}"));

        assert!(plan.patch.is_empty());
        assert!(plan.removed.is_empty());
        assert!(plan.added.is_empty());
    }

    #[test]
    fn one_object_action_commits_as_one_session_revision_and_history_entry() {
        let source = spec(100, (3200, 3200), 0, 10, 1);
        let moved = spec(100, (3201, 3200), 0, 10, 1);
        let source_placement = placement_from_spec(source)
            .unwrap_or_else(|error| unreachable!("valid source: {error:?}"));
        let moved_placement = placement_from_spec(moved)
            .unwrap_or_else(|error| unreachable!("valid destination: {error:?}"));
        let document = MapDocument::from_parts([], [], [source_placement]);
        let mut session = EditorSession::new(document);
        let plan = build_object_commit_plan(session.document(), &[source], &[moved])
            .unwrap_or_else(|error| unreachable!("valid object action: {error:?}"));

        let commit = session
            .commit(plan.patch)
            .unwrap_or_else(|error| unreachable!("commit should succeed: {error:?}"))
            .unwrap_or_else(|| unreachable!("move is not a no-op"));

        assert_eq!(commit.revision, 1);
        assert_eq!(session.revision(), 1);
        assert!(session.can_undo());
        assert!(!session.document().contains_object(source_placement));
        assert!(session.document().contains_object(moved_placement));
    }

    #[test]
    fn semantic_object_undo_projection_is_validated_from_session_state() {
        let source = spec(100, (3200, 3200), 0, 10, 1);
        let moved = spec(100, (3201, 3200), 0, 10, 1);
        let source_placement = placement_from_spec(source)
            .unwrap_or_else(|error| unreachable!("valid source: {error:?}"));
        let mut session = EditorSession::new(MapDocument::from_parts([], [], [source_placement]));
        let plan = build_object_commit_plan(session.document(), &[source], &[moved])
            .unwrap_or_else(|error| unreachable!("valid move: {error:?}"));
        let route = ObjectHistoryRoute::new(plan.removed.clone(), plan.added.clone());
        assert!(session.commit(plan.patch).is_ok());
        assert!(session.undo().is_ok());

        let projection = build_object_history_projection(session.document(), &route, false)
            .unwrap_or_else(|error| unreachable!("valid undo projection: {error:?}"));
        assert_eq!(
            projection.edits,
            vec![LocEdit::Remove(moved), LocEdit::Add(source)]
        );
        assert_eq!(projection.reselect, vec![source]);
    }

    #[test]
    fn semantic_object_redo_projection_is_validated_from_session_state() {
        let source = spec(100, (3200, 3200), 0, 10, 1);
        let moved = spec(100, (3201, 3200), 0, 10, 1);
        let source_placement = placement_from_spec(source)
            .unwrap_or_else(|error| unreachable!("valid source: {error:?}"));
        let mut session = EditorSession::new(MapDocument::from_parts([], [], [source_placement]));
        let plan = build_object_commit_plan(session.document(), &[source], &[moved])
            .unwrap_or_else(|error| unreachable!("valid move: {error:?}"));
        let route = ObjectHistoryRoute::new(plan.removed.clone(), plan.added.clone());
        assert!(session.commit(plan.patch).is_ok());
        assert!(session.undo().is_ok());
        assert!(session.redo().is_ok());

        let projection = build_object_history_projection(session.document(), &route, true)
            .unwrap_or_else(|error| unreachable!("valid redo projection: {error:?}"));
        assert_eq!(
            projection.edits,
            vec![LocEdit::Remove(source), LocEdit::Add(moved)]
        );
        assert_eq!(projection.reselect, vec![moved]);
    }

    #[test]
    fn stale_object_history_route_is_rejected_before_worker_projection() {
        let source = spec(100, (3200, 3200), 0, 10, 1);
        let moved = spec(100, (3201, 3200), 0, 10, 1);
        let source_placement = placement_from_spec(source)
            .unwrap_or_else(|error| unreachable!("valid source: {error:?}"));
        let document = MapDocument::from_parts([], [], [source_placement]);
        let route = ObjectHistoryRoute::new(vec![source], vec![moved]);

        assert_eq!(
            build_object_history_projection(&document, &route, true),
            Err(ObjectHistoryProjectionError::ExpectedAbsent { spec: source })
        );
    }
}
