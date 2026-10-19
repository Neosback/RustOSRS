#![cfg_attr(not(test), allow(dead_code))]

use super::*;
use osrs_edit::MapReplayFootprint;

/// Immutable application-facing projection requirements for the next semantic replay.
///
/// The descriptor is derived from the exact authoritative history patch. It deliberately exposes
/// neither `PatchHistory` nor the raw `EditorPatch`, and it carries no UI/tool provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EditReplayDescriptor {
    Map(MapReplayFootprint),
    Scenario,
    Appearance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReplayDirection {
    Undo,
    Redo,
}

impl EditorSession {
    /// Projection requirements for the exact semantic edit that would be undone next.
    ///
    /// Map replay uses the inverse patch because undo applies inverse mutation polarity. The raw
    /// history patch remains private to the session.
    pub(crate) fn next_undo_replay_descriptor(&self) -> Option<EditReplayDescriptor> {
        self.history
            .next_undo()
            .map(|patch| replay_descriptor(patch, ReplayDirection::Undo))
    }

    /// Projection requirements for the exact semantic edit that would be redone next.
    ///
    /// Redo reapplies the original patch, so object mutation polarity remains unchanged.
    pub(crate) fn next_redo_replay_descriptor(&self) -> Option<EditReplayDescriptor> {
        self.history
            .next_redo()
            .map(|patch| replay_descriptor(patch, ReplayDirection::Redo))
    }
}

fn replay_descriptor(patch: &EditorPatch, direction: ReplayDirection) -> EditReplayDescriptor {
    match patch {
        EditorPatch::Workspace(WorkspacePatch::Map(patch)) => {
            let footprint = match direction {
                ReplayDirection::Undo => patch.inverse().replay_footprint(),
                ReplayDirection::Redo => patch.replay_footprint(),
            };
            EditReplayDescriptor::Map(footprint)
        }
        EditorPatch::Workspace(WorkspacePatch::Scenario(_)) => EditReplayDescriptor::Scenario,
        EditorPatch::Appearance(_) => EditReplayDescriptor::Appearance,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use osrs_core::{
        coords::{MapTile, SourcePlane},
        ids::ObjectId,
    };
    use osrs_edit::{
        LocKind, MapDocument, MapOp, MapPatch, MapPosition, ObjectOrientation, ObjectPlacement,
        ScenarioPatch,
    };
    use osrs_hd::theme::{ThemePack, ThemePatch};

    fn position() -> MapPosition {
        let plane = SourcePlane::new(0).unwrap_or_else(|| unreachable!("plane zero is valid"));
        MapPosition::new(MapTile::new(3200, 3200), plane)
    }

    fn placement() -> ObjectPlacement {
        ObjectPlacement {
            object_id: ObjectId::new(100),
            position: position(),
            kind: LocKind::new(10).unwrap_or_else(|| unreachable!("loc kind is valid")),
            orientation: ObjectOrientation::new(1)
                .unwrap_or_else(|| unreachable!("orientation is valid")),
        }
    }

    fn map_footprint(descriptor: EditReplayDescriptor) -> MapReplayFootprint {
        match descriptor {
            EditReplayDescriptor::Map(footprint) => footprint,
            other => unreachable!("expected map replay descriptor, got {other:?}"),
        }
    }

    #[test]
    fn map_descriptor_tracks_exact_authoritative_history_direction() {
        let object = placement();
        let patch = MapPatch::new([MapOp::AddObject { placement: object }]);
        let mut session = EditorSession::new(MapDocument::new());
        session
            .commit(patch)
            .unwrap_or_else(|error| unreachable!("valid object commit: {error:?}"))
            .unwrap_or_else(|| unreachable!("non-empty patch creates history"));

        let undo = map_footprint(
            session
                .next_undo_replay_descriptor()
                .unwrap_or_else(|| unreachable!("undo descriptor exists")),
        );
        assert_eq!(undo.removed_objects(), &[object]);
        assert!(undo.added_objects().is_empty());

        session
            .undo()
            .unwrap_or_else(|error| unreachable!("valid undo: {error:?}"))
            .unwrap_or_else(|| unreachable!("undo entry exists"));

        let redo = map_footprint(
            session
                .next_redo_replay_descriptor()
                .unwrap_or_else(|| unreachable!("redo descriptor exists")),
        );
        assert_eq!(redo.added_objects(), &[object]);
        assert!(redo.removed_objects().is_empty());
    }

    #[test]
    fn rolled_back_replay_restores_descriptor_order_exactly() {
        let object = placement();
        let mut session = EditorSession::new(MapDocument::new());
        session
            .commit(MapPatch::new([MapOp::AddObject { placement: object }]))
            .unwrap_or_else(|error| unreachable!("valid object commit: {error:?}"));
        let before = session
            .next_undo_replay_descriptor()
            .unwrap_or_else(|| unreachable!("undo descriptor exists"));

        let (_, checkpoint) = session
            .undo_checkpointed()
            .unwrap_or_else(|error| unreachable!("valid checkpointed undo: {error:?}"))
            .unwrap_or_else(|| unreachable!("undo entry exists"));
        session
            .restore_checkpoint(checkpoint)
            .unwrap_or_else(|error| unreachable!("fresh replay checkpoint restores: {error:?}"));

        assert_eq!(session.next_undo_replay_descriptor(), Some(before));
        assert_eq!(session.next_redo_replay_descriptor(), None);
    }

    #[test]
    fn non_map_patch_kinds_are_structural_descriptors() {
        let scenario = EditorPatch::Workspace(WorkspacePatch::Scenario(ScenarioPatch::Replace {
            before: None,
            after: None,
        }));
        assert_eq!(
            replay_descriptor(&scenario, ReplayDirection::Undo),
            EditReplayDescriptor::Scenario
        );
        assert_eq!(
            replay_descriptor(&scenario, ReplayDirection::Redo),
            EditReplayDescriptor::Scenario
        );

        let theme = ThemePack::default();
        let appearance = EditorPatch::Appearance(Box::new(
            ThemePatch::plan(&theme, theme.clone())
                .unwrap_or_else(|error| unreachable!("stock theme is valid: {error}")),
        ));
        assert_eq!(
            replay_descriptor(&appearance, ReplayDirection::Undo),
            EditReplayDescriptor::Appearance
        );
        assert_eq!(
            replay_descriptor(&appearance, ReplayDirection::Redo),
            EditReplayDescriptor::Appearance
        );
    }
}
