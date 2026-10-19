//! Finishing a stroke: plan, commit once to the semantic session, and project.

use super::*;
use crate::app::edit_runtime::{EditExecution, EditExecutionError};

/// Whether a stroke committed a non-empty semantic transaction.
type StrokeOutcome = bool;

/// One already-validated Paint projection from authoritative semantic state into the transitional
/// preview/worker mirrors.
type PaintProjection = Vec<(usize, TileKey, TileValues)>;

/// One already-validated Height projection from authoritative semantic state into the transitional
/// preview/worker mirrors.
type HeightProjection = Vec<(usize, TileKey, TileValues)>;

/// Prepare the complete Paint projection from the authoritative post-commit document.
///
/// This is deliberately pure. Any unprojectable semantic value fails while `EditRuntime` still owns
/// the active checkpoint, so the gateway can restore the exact pre-stroke semantic state. Applying
/// a successful plan later cannot fail.
fn plan_paint_projection(
    document: &osrs_edit::MapDocument,
    brush: &Brush,
    current: &[(usize, TileKey, TileValues)],
) -> Result<PaintProjection, &'static str> {
    let mut projected = Vec::with_capacity(current.len());
    for (index, key, after) in current {
        let values = project_paint_values(document, brush, *key, *after)
            .ok_or("a Paint preview tile could not be projected from semantic state")?;
        projected.push((*index, *key, values));
    }
    Ok(projected)
}

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
    /// Finish the open stroke. Paint and Height commit exactly once to the semantic session.
    /// Terrain paths commit separately in `editing/path.rs`. There is no legacy fallback: a stroke
    /// without a semantic planner is reverted, so every terrain mutation enters through
    /// `EditorSession`.
    pub(in crate::app) fn end_stroke(&mut self) {
        let Some(mut stroke) = self.terrain.stroke.take() else {
            return;
        };
        if stroke.is_empty() {
            return;
        }

        let committed = if let Some(brush) = stroke.paint_brush.clone() {
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
        if !committed {
            return;
        }
        // Projection may have reconciled every tile back to its original value.
        if stroke.is_empty() {
            return;
        }
        let count = stroke.changes.len();
        self.ui.notify(format!(
            "Changed {count} tile{}",
            if count == 1 { "" } else { "s" }
        ));
    }

    /// Undo the preview and report `message`.
    fn abandon_stroke(&mut self, stroke: &Stroke, message: String) -> StrokeOutcome {
        let edits = self
            .terrain
            .restore_stroke_preview(self.world.infos(), stroke);
        self.send_edits(edits);
        self.ui.notify(message);
        false
    }

    /// Apply an already-validated Paint projection to the visible preview and worker mirror.
    ///
    /// All fallible semantic reads happened inside `EditRuntime::execute`; from this point the
    /// semantic transaction is finalized and projection application is mechanical. Rebuild scope
    /// comes separately from the finalized semantic receipt.
    fn apply_paint_projection(&mut self, stroke: &mut Stroke, projection: PaintProjection) {
        let mut edits = Vec::new();
        for (index, key, values) in projection {
            let after = &mut stroke.changes[index].2;
            if values != *after {
                self.terrain.edited.insert(key, values);
                edits.push(self.edit_for(key, values));
            }
            *after = values;
        }
        self.world.project_tile_edits(edits);
        stroke
            .changes
            .retain(|(_, before, after)| before != after);
    }

    fn commit_paint_stroke(&mut self, stroke: &mut Stroke, brush: Brush) -> StrokeOutcome {
        let patch = match build_paint_stroke_patch(self.session.document(), &brush, &stroke.changes) {
            Ok(patch) => patch,
            Err(error) => {
                return self.abandon_stroke(
                    stroke,
                    format!("Paint stroke was not committed: {error:?}"),
                );
            }
        };
        let projection_input: Vec<_> = stroke
            .changes
            .iter()
            .enumerate()
            .map(|(index, (key, _, after))| (index, *key, *after))
            .collect();
        let execution = self.session.execute_named("paint", patch, |document| {
            plan_paint_projection(document, &brush, &projection_input)
        });
        match execution {
            Ok(EditExecution::Committed {
                receipt,
                projection,
            }) => {
                self.apply_paint_projection(stroke, projection);
                self.world.invalidate_semantic_edit(&receipt);
                true
            }
            Ok(EditExecution::NoChange) => {
                // The gateway intentionally skips projection for an empty patch. There is no
                // semantic checkpoint to protect here, so preserve the existing preview adapter.
                self.project_paint_stroke(stroke, &brush);
                self.ui
                    .notify("Paint stroke made no semantic changes".to_owned());
                false
            }
            Err(EditExecutionError::Semantic(error)) => self.abandon_stroke(
                stroke,
                format!("Paint stroke was rejected by the semantic session: {error:?}"),
            ),
            Err(EditExecutionError::Projection(message)) => self.abandon_stroke(
                stroke,
                format!("Paint commit was rolled back: {message}"),
            ),
            Err(EditExecutionError::Rollback {
                projection,
                rollback,
            }) => self.abandon_stroke(
                stroke,
                format!(
                    "Paint projection failed and semantic rollback failed: {projection} ({rollback:?})"
                ),
            ),
            Err(EditExecutionError::Finalize(error)) => self.abandon_stroke(
                stroke,
                format!("Paint projection succeeded but semantic finalize failed: {error:?}"),
            ),
        }
    }

    /// Apply an already-validated Height projection to the visible preview and worker mirror.
    ///
    /// All fallible semantic reads happened inside `EditRuntime::execute`; from this point the
    /// semantic transaction is finalized and projection application is mechanical. Rebuild scope
    /// comes separately from the finalized semantic receipt.
    fn apply_height_projection(&mut self, stroke: &mut Stroke, projection: HeightProjection) {
        let mut edits = Vec::new();
        for (index, key, values) in projection {
            let after = &mut stroke.changes[index].2;
            self.terrain.height_edited.insert(key);
            if values != *after {
                self.terrain.edited.insert(key, values);
                edits.push(self.edit_for(key, values));
            }
            *after = values;
        }
        self.world.project_tile_edits(edits);
        stroke
            .changes
            .retain(|(_, before, after)| before != after);
    }

    fn commit_height_stroke(
        &mut self,
        stroke: &mut Stroke,
        spec: &HeightStrokeSpec,
    ) -> StrokeOutcome {
        let missing = stroke.changes.iter().find_map(|(key, _, _)| {
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
            .changes
            .iter()
            .enumerate()
            .map(|(index, (key, _, after))| (index, *key, *after))
            .collect();
        let execution = self.session.execute_named("height", patch, |document| {
            plan_height_projection(document, &projection_input)
        });
        match execution {
            Ok(EditExecution::Committed {
                receipt,
                projection,
            }) => {
                self.apply_height_projection(stroke, projection);
                self.world.invalidate_semantic_edit(&receipt);
                true
            }
            Ok(EditExecution::NoChange) => {
                // The gateway intentionally skips projection for an empty patch. There is no
                // semantic checkpoint to protect here, so preserve the existing preview adapter.
                let text = match self.project_height_stroke(stroke) {
                    Ok(()) => {
                        self.ui
                            .notify("Height stroke made no semantic changes".to_owned());
                        return false;
                    }
                    Err(message) => format!("Height no-op projection failed: {message}"),
                };
                self.abandon_stroke(stroke, text)
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
mod tests;
