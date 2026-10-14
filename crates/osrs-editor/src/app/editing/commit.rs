//! Finishing a stroke: plan, commit once to the semantic session, and project.

use super::*;

/// A committed stroke's routing marker for the global history stack, or `None` when there is
/// nothing to record (rejected or no-op; the notice is already set).
type StrokeOutcome = Option<EditStep>;

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
        match self.session.commit(patch) {
            Ok(Some(_)) => {
                if let Err(message) = self.project_height_stroke(stroke) {
                    let rolled_back = matches!(self.session.undo(), Ok(Some(_)));
                    let text = if rolled_back {
                        format!("Height commit was rolled back: {message}")
                    } else {
                        format!("Height projection failed and semantic rollback failed: {message}")
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
