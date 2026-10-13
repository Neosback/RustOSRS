//! Finishing a stroke: plan, commit once to the semantic session, and project.

use super::*;

impl EditorApp {
    /// Finish the open stroke. Paint and Height commit exactly once to the semantic session and
    /// leave only lightweight routing markers in the global mixed-history stack. Terrain paths
    /// commit separately in `editing/path.rs`. There is no legacy fallback: a stroke without a semantic
    /// planner is reverted, so every terrain mutation enters through `EditorSession`.
    pub(in crate::app) fn end_stroke(&mut self) {
        let Some(mut stroke) = self.stroke.take() else {
            return;
        };
        if stroke.step.changes.is_empty()
            && stroke.step.added.is_empty()
            && stroke.step.removed.is_empty()
        {
            return;
        }

        let paint_history = if let Some(brush) = stroke.paint_brush.clone() {
            let patch = match build_paint_stroke_patch(
                self.session.document(),
                &brush,
                &stroke.step.changes,
            ) {
                Ok(patch) => patch,
                Err(error) => {
                    self.revert(&stroke.step, false);
                    self.notice = Some((
                        format!("Paint stroke was not committed: {error:?}"),
                        Instant::now(),
                    ));
                    return;
                }
            };

            match self.session.commit(patch) {
                Ok(Some(_)) => {
                    self.project_paint_stroke(&mut stroke, &brush);
                    Some(SemanticPaintHistory {
                        brush,
                        keys: stroke.step.changes.iter().map(|(key, _, _)| *key).collect(),
                    })
                }
                Ok(None) => {
                    self.project_paint_stroke(&mut stroke, &brush);
                    self.notice = Some((
                        "Paint stroke made no semantic changes".to_owned(),
                        Instant::now(),
                    ));
                    return;
                }
                Err(error) => {
                    self.revert(&stroke.step, false);
                    self.notice = Some((
                        format!("Paint stroke was rejected by the semantic session: {error:?}"),
                        Instant::now(),
                    ));
                    return;
                }
            }
        } else {
            None
        };

        let mut height_history = None;
        if paint_history.is_none()
            && let Some(spec) = stroke.height_spec.clone()
        {
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
                self.revert(&stroke.step, false);
                self.notice = Some((
                    format!(
                        "Height stroke was not committed: semantic height is missing at {coord:?}"
                    ),
                    Instant::now(),
                ));
                return;
            }

            let patch = match build_terrain_height_patch(
                self.session.document(),
                &spec,
                &stroke.height_centers,
            ) {
                Ok(patch) => patch,
                Err(error) => {
                    self.revert(&stroke.step, false);
                    self.notice = Some((
                        format!("Height stroke was not committed: {error:?}"),
                        Instant::now(),
                    ));
                    return;
                }
            };

            match self.session.commit(patch) {
                Ok(Some(_)) => {
                    if let Err(message) = self.project_height_stroke(&mut stroke) {
                        let rollback_succeeded = matches!(self.session.undo(), Ok(Some(_)));
                        self.revert(&stroke.step, false);
                        self.notice = Some((
                            if rollback_succeeded {
                                format!("Height commit was rolled back: {message}")
                            } else {
                                format!(
                                    "Height projection failed and semantic rollback failed: {message}"
                                )
                            },
                            Instant::now(),
                        ));
                        return;
                    }
                    height_history = Some(SemanticHeightHistory {
                        keys: stroke.step.changes.iter().map(|(key, _, _)| *key).collect(),
                    });
                }
                Ok(None) => {
                    if let Err(message) = self.project_height_stroke(&mut stroke) {
                        self.revert(&stroke.step, false);
                        self.notice = Some((
                            format!("Height no-op projection failed: {message}"),
                            Instant::now(),
                        ));
                    } else {
                        self.notice = Some((
                            "Height stroke made no semantic changes".to_owned(),
                            Instant::now(),
                        ));
                    }
                    return;
                }
                Err(error) => {
                    self.revert(&stroke.step, false);
                    self.notice = Some((
                        format!("Height stroke was rejected by the semantic session: {error:?}"),
                        Instant::now(),
                    ));
                    return;
                }
            }
        }

        if stroke.step.changes.is_empty()
            && stroke.step.added.is_empty()
            && stroke.step.removed.is_empty()
        {
            return;
        }
        let count = stroke.step.changes.len();
        if let Some(paint_history) = paint_history {
            self.push_step(EditStep::semantic_paint(
                paint_history.brush,
                paint_history.keys,
            ));
        } else if let Some(height_history) = height_history {
            self.push_step(EditStep::semantic_height(height_history.keys));
        } else {
            // Every terrain stroke carries a Paint brush or a Height spec. Refuse rather than
            // recreate legacy tile-history authority outside `EditorSession`.
            debug_assert!(false, "terrain stroke without a semantic planner");
            self.revert(&stroke.step, false);
            self.notice = Some((
                "Edit was not committed: no semantic planner for this stroke".to_owned(),
                Instant::now(),
            ));
            return;
        }
        self.notice = Some((
            format!("Changed {count} tile{}", if count == 1 { "" } else { "s" }),
            Instant::now(),
        ));
    }
}
