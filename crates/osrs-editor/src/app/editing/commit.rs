//! Finishing a stroke: plan, commit once to the semantic session, and project.

use super::*;
use crate::session::{EditCheckpoint, EditCheckpointError, EditorSession};

/// A committed stroke's routing marker for the global history stack, or `None` when there is
/// nothing to record (rejected or no-op; the notice is already set).
type StrokeOutcome = Option<EditStep>;

#[derive(Debug, PartialEq, Eq)]
enum HeightProjectionError {
    Projection(&'static str),
    Rollback {
        projection: &'static str,
        rollback: EditCheckpointError,
    },
    Finalize(EditCheckpointError),
}

/// Resolve one Height semantic checkpoint only after worker projection finishes.
///
/// Projection failure restores the exact pre-stroke semantic state. Projection success finalizes
/// the transaction, which is the point where any pre-existing redo branch is invalidated.
fn finish_height_projection(
    session: &mut EditorSession,
    checkpoint: EditCheckpoint,
    projection: Result<(), &'static str>,
) -> Result<(), HeightProjectionError> {
    match projection {
        Ok(()) => session
            .finalize_checkpoint(checkpoint)
            .map_err(HeightProjectionError::Finalize),
        Err(projection) => match session.restore_checkpoint(checkpoint) {
            Ok(()) => Err(HeightProjectionError::Projection(projection)),
            Err(rollback) => Err(HeightProjectionError::Rollback {
                projection,
                rollback,
            }),
        },
    }
}

impl EditorApp {
    /// Finish the open stroke. Paint and Height commit exactly once to the semantic session and
    /// leave only lightweight routing markers in the global mixed-history stack. Terrain paths
    /// commit separately in `editing/path.rs`. There is no legacy fallback: a stroke without a
    /// semantic planner is reverted, so every terrain mutation enters through `EditorSession`.
    pub(in crate::app) fn end_stroke(&mut self) {
        let Some(mut stroke) = self.stroke.take() else {
            return;
        };
        if stroke.is_empty() {
            return;
        }

        let outcome = if let Some(brush) = stroke.paint_brush.clone() {
            self.commit_paint_stroke(&mut stroke, brush)
        } else if let Some(spec) = stroke.height_spec.clone() {
            self.commit_height_stroke(&mut stroke, &spec)
        } else {
            // Every terrain stroke carries a Paint brush or a Height spec. Refuse rather than
            // recreate legacy tile-history authority outside `EditorSession`.
            debug_assert!(false, "terrain stroke without a semantic planner");
            self.abandon_stroke(
                &stroke,
                "Edit was not committed: no semantic planner for this stroke".to_owned(),
            )
        };
        let Some(marker) = outcome else {
            return;
        };
        // Projection may have reconciled every tile back to its original value.
        if stroke.is_empty() {
            return;
        }
        let count = stroke.step.changes.len();
        self.push_step(marker);
        self.notice = Some((
            format!("Changed {count} tile{}", if count == 1 { "" } else { "s" }),
            Instant::now(),
        ));
    }

    /// Undo the preview and report `message`.
    fn abandon_stroke(&mut self, stroke: &Stroke, message: String) -> StrokeOutcome {
        self.revert(&stroke.step, false);
        self.notice = Some((message, Instant::now()));
        None
    }

    fn touched_keys(stroke: &Stroke) -> Vec<TileKey> {
        stroke.step.changes.iter().map(|(key, _, _)| *key).collect()
    }

    fn commit_paint_stroke(&mut self, stroke: &mut Stroke, brush: Brush) -> StrokeOutcome {
        let patch =
            match build_paint_stroke_patch(self.session.document(), &brush, &stroke.step.changes) {
                Ok(patch) => patch,
                Err(error) => {
                    return self.abandon_stroke(
                        stroke,
                        format!("Paint stroke was not committed: {error:?}"),
                    );
                }
            };
        match self.session.commit(patch) {
            Ok(Some(_)) => {
                self.project_paint_stroke(stroke, &brush);
                Some(EditStep::semantic_paint(brush, Self::touched_keys(stroke)))
            }
            Ok(None) => {
                self.project_paint_stroke(stroke, &brush);
                self.notice = Some((
                    "Paint stroke made no semantic changes".to_owned(),
                    Instant::now(),
                ));
                None
            }
            Err(error) => self.abandon_stroke(
                stroke,
                format!("Paint stroke was rejected by the semantic session: {error:?}"),
            ),
        }
    }

    fn commit_height_stroke(
        &mut self,
        stroke: &mut Stroke,
        spec: &HeightStrokeSpec,
    ) -> StrokeOutcome {
        let missing = stroke.step.changes.iter().find_map(|(key, _, _)| {
            let plane = SourcePlane::new(key.0)?;
            let coord = HeightVertexCoord::new(MapTile::new(key.1, key.2), plane);
            self.session
                .document()
                .height_at(coord)
                .is_none()
                .then_some(coord)
        });
        if let Some(coord) = missing {
            return self.abandon_stroke(
                stroke,
                format!("Height stroke was not committed: semantic height is missing at {coord:?}"),
            );
        }
        let patch =
            match build_terrain_height_patch(self.session.document(), spec, &stroke.height_centers)
            {
                Ok(patch) => patch,
                Err(error) => {
                    return self.abandon_stroke(
                        stroke,
                        format!("Height stroke was not committed: {error:?}"),
                    );
                }
            };
        match self.session.commit_checkpointed(patch) {
            Ok(Some((_, checkpoint))) => {
                let projection = self.project_height_stroke(stroke);
                if let Err(error) =
                    finish_height_projection(&mut self.session, checkpoint, projection)
                {
                    let text = match error {
                        HeightProjectionError::Projection(message) => {
                            format!("Height commit was rolled back: {message}")
                        }
                        HeightProjectionError::Rollback {
                            projection,
                            rollback,
                        } => format!(
                            "Height projection failed and semantic rollback failed: {projection} ({rollback:?})"
                        ),
                        HeightProjectionError::Finalize(error) => format!(
                            "Height projection succeeded but semantic finalize failed: {error:?}"
                        ),
                    };
                    return self.abandon_stroke(stroke, text);
                }
                Some(EditStep::semantic_height(Self::touched_keys(stroke)))
            }
            Ok(None) => {
                let text = match self.project_height_stroke(stroke) {
                    Ok(()) => {
                        self.notice = Some((
                            "Height stroke made no semantic changes".to_owned(),
                            Instant::now(),
                        ));
                        return None;
                    }
                    Err(message) => format!("Height no-op projection failed: {message}"),
                };
                self.abandon_stroke(stroke, text)
            }
            Err(error) => self.abandon_stroke(
                stroke,
                format!("Height stroke was rejected by the semantic session: {error:?}"),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use osrs_core::coords::{MapTile, SourcePlane};
    use osrs_edit::{HeightVertexCoord, MapDocument, MapOp, MapPatch};

    fn plane() -> SourcePlane {
        SourcePlane::new(0).unwrap_or_else(|| unreachable!("plane zero is valid"))
    }

    fn vertex(tile: (i32, i32)) -> HeightVertexCoord {
        HeightVertexCoord::new(MapTile::new(tile.0, tile.1), plane())
    }

    fn height_patch(coord: HeightVertexCoord, before: i32, after: i32) -> MapPatch {
        MapPatch::new([MapOp::SetHeight {
            coord,
            before,
            after,
        }])
    }

    fn session_with_redo() -> (EditorSession, HeightVertexCoord) {
        let prior = vertex((3200, 3200));
        let edited = vertex((3264, 3200));
        let mut session =
            EditorSession::new(MapDocument::from_parts([], [(prior, 0), (edited, 0)], []));
        session
            .commit(height_patch(prior, 0, 32))
            .unwrap_or_else(|error| unreachable!("valid setup commit: {error:?}"));
        session
            .undo()
            .unwrap_or_else(|error| unreachable!("valid setup undo: {error:?}"));
        assert!(session.can_redo());
        (session, edited)
    }

    #[test]
    fn failed_height_projection_restores_exact_pre_stroke_session() {
        let (mut session, edited) = session_with_redo();
        let before = session.clone();
        let (_, checkpoint) = session
            .commit_checkpointed(height_patch(edited, 0, 64))
            .unwrap_or_else(|error| unreachable!("valid Height commit: {error:?}"))
            .unwrap_or_else(|| unreachable!("Height changes semantic state"));

        let result = finish_height_projection(
            &mut session,
            checkpoint,
            Err("injected Height projection failure"),
        );

        assert_eq!(
            result,
            Err(HeightProjectionError::Projection(
                "injected Height projection failure"
            ))
        );
        assert_eq!(session, before);
    }

    #[test]
    fn successful_height_projection_finalizes_once_and_invalidates_prior_redo() {
        let (mut session, edited) = session_with_redo();
        let revision_before = session.revision();
        let (_, checkpoint) = session
            .commit_checkpointed(height_patch(edited, 0, 64))
            .unwrap_or_else(|error| unreachable!("valid Height commit: {error:?}"))
            .unwrap_or_else(|| unreachable!("Height changes semantic state"));

        finish_height_projection(&mut session, checkpoint, Ok(()))
            .unwrap_or_else(|error| unreachable!("Height finalizes: {error:?}"));

        assert_eq!(session.revision(), revision_before + 1);
        assert_eq!(session.document().height_at(edited), Some(64));
        assert!(session.can_undo());
        assert!(!session.can_redo());
    }
}
