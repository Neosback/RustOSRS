//! Finishing a stroke: plan, commit once to the semantic session, and project.

use super::*;
use crate::app::edit_runtime::{EditExecution, EditExecutionError};

/// A committed stroke's routing marker for the global history stack, or `None` when there is
/// nothing to record (rejected or no-op; the notice is already set).
type StrokeOutcome = Option<EditStep>;

/// One already-validated Height projection from authoritative semantic state into the transitional
/// preview/worker mirrors.
type HeightProjection = Vec<(usize, TileKey, TileValues)>;

/// Prepare the complete Height projection from the authoritative post-commit document.
///
/// This is deliberately pure. Any missing semantic height fails while `EditRuntime` still owns the
/// active checkpoint, so the gateway can restore the exact pre-stroke semantic state. Applying a
/// successful plan later cannot fail.
fn plan_height_projection(
    document: &osrs_edit::MapDocument,
    current: &[(usize, TileKey, TileValues)],
) -> Result<HeightProjection, &'static str> {
    let mut projected = Vec::with_capacity(current.len());
    for (index, key, after) in current {
        let values = project_height_values(document, *key, *after)
            .ok_or("a Height preview vertex is missing from semantic state")?;
        projected.push((*index, *key, values));
    }
    Ok(projected)
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

    /// Apply an already-validated Height projection to the visible preview and worker mirror.
    ///
    /// All fallible semantic reads happened inside `EditRuntime::execute`; from this point the
    /// semantic transaction is finalized and projection application is mechanical.
    fn apply_height_projection(&mut self, stroke: &mut Stroke, projection: HeightProjection) {
        let mut edits = Vec::new();
        for (index, key, values) in projection {
            let after = &mut stroke.step.changes[index].2;
            self.height_edited.insert(key);
            if values != *after {
                self.edited.insert(key, values);
                edits.push(self.edit_for(key, values));
            }
            *after = values;
        }
        self.send_edits(edits);
        stroke
            .step
            .changes
            .retain(|(_, before, after)| before != after);
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
        let projection_input: Vec<_> = stroke
            .step
            .changes
            .iter()
            .enumerate()
            .map(|(index, (key, _, after))| (index, *key, *after))
            .collect();
        let execution = self
            .session
            .execute(patch, |document| plan_height_projection(document, &projection_input));
        match execution {
            Ok(EditExecution::Committed { projection, .. }) => {
                self.apply_height_projection(stroke, projection);
                Some(EditStep::semantic_height(Self::touched_keys(stroke)))
            }
            Ok(EditExecution::NoChange) => {
                // The gateway intentionally skips projection for an empty patch. There is no
                // semantic checkpoint to protect here, but the already-visible Height preview still
                // needs reconciliation against the unchanged authoritative document.
                let projection = match plan_height_projection(
                    self.session.document(),
                    &projection_input,
                ) {
                    Ok(projection) => projection,
                    Err(message) => {
                        return self.abandon_stroke(
                            stroke,
                            format!("Height no-op projection failed: {message}"),
                        );
                    }
                };
                self.apply_height_projection(stroke, projection);
                self.notice = Some((
                    "Height stroke made no semantic changes".to_owned(),
                    Instant::now(),
                ));
                None
            }
            Err(EditExecutionError::Semantic(error)) => self.abandon_stroke(
                stroke,
                format!("Height stroke was rejected by the semantic session: {error:?}"),
            ),
            Err(EditExecutionError::Projection(message)) => self.abandon_stroke(
                stroke,
                format!("Height commit was rolled back: {message}"),
            ),
            Err(EditExecutionError::Rollback {
                projection,
                rollback,
            }) => self.abandon_stroke(
                stroke,
                format!(
                    "Height projection failed and semantic rollback failed: {projection} ({rollback:?})"
                ),
            ),
            Err(EditExecutionError::Finalize(error)) => self.abandon_stroke(
                stroke,
                format!("Height projection succeeded but semantic finalize failed: {error:?}"),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::edit_runtime::{EditExecution, EditExecutionError, EditRuntime};
    use crate::session::EditorSession;
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

    fn runtime_with_redo() -> (EditRuntime, HeightVertexCoord) {
        let prior = vertex((3200, 3200));
        let edited = vertex((3264, 3200));
        let mut runtime = EditRuntime::new(EditorSession::new(MapDocument::from_parts(
            [],
            [(prior, 0), (edited, 0)],
            [],
        )));
        runtime
            .commit(height_patch(prior, 0, 32))
            .unwrap_or_else(|error| unreachable!("valid setup commit: {error:?}"));
        runtime
            .undo()
            .unwrap_or_else(|error| unreachable!("valid setup undo: {error:?}"));
        assert!(runtime.can_redo());
        (runtime, edited)
    }

    #[test]
    fn failed_height_projection_restores_exact_pre_stroke_session() {
        let (mut runtime, edited) = runtime_with_redo();
        let before = runtime.clone();
        let missing_key = (0, edited.corner.x + 1, edited.corner.y);
        let current = TileValues::default();
        let result = runtime.execute(height_patch(edited, 0, 64), |document| {
            assert_eq!(document.height_at(edited), Some(64));
            plan_height_projection(document, &[(0, missing_key, current)])
        });

        assert_eq!(
            result,
            Err(EditExecutionError::Projection(
                "a Height preview vertex is missing from semantic state"
            ))
        );
        assert_eq!(runtime, before);
    }

    #[test]
    fn successful_height_projection_uses_post_commit_state_and_finalizes_once() {
        let (mut runtime, edited) = runtime_with_redo();
        let revision_before = runtime.revision();
        let key = (0, edited.corner.x, edited.corner.y);
        let current = TileValues::default();
        let result = runtime
            .execute(height_patch(edited, 0, 64), |document| {
                assert_eq!(document.height_at(edited), Some(64));
                plan_height_projection(document, &[(0, key, current)])
            })
            .unwrap_or_else(|error| unreachable!("Height gateway execution succeeds: {error:?}"));

        let EditExecution::Committed { projection, .. } = result else {
            unreachable!("non-empty Height patch commits")
        };
        assert_eq!(
            projection,
            vec![(
                0,
                key,
                TileValues {
                    height: 64,
                    ..current
                }
            )]
        );
        assert_eq!(runtime.revision(), revision_before + 1);
        assert_eq!(runtime.document().height_at(edited), Some(64));
        assert!(runtime.can_undo());
        assert!(!runtime.can_redo());
    }
}
