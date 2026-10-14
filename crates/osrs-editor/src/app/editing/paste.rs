//! Copy and paste of the selected tiles (terrain channels and heights).
//!
//! A paste plans one Paint-style patch against the authoritative session, holds an exact semantic
//! checkpoint while projecting the result into worker mirrors, and finalizes only after projection
//! succeeds. It is recorded with the semantic path-history route (terrain plus heights from the
//! document), so undo and redo reuse the existing projection.

use super::*;
use crate::app::paint_commit::build_terrain_paint_patch;
use crate::app::tile_clipboard::TileClipboard;
use crate::session::{EditCheckpoint, EditCheckpointError, EditorSession};

#[derive(Debug, PartialEq, Eq)]
enum PasteProjectionError {
    Projection(String),
    Rollback {
        projection: String,
        rollback: EditCheckpointError,
    },
    Finalize(EditCheckpointError),
}

/// Resolve the semantic checkpoint only after Paste projection has finished.
///
/// A projection error restores the exact pre-Paste semantic state. A successful projection
/// finalizes the history transaction, which is the point where any pre-existing redo branch is
/// invalidated.
fn finish_paste_projection<T, E>(
    session: &mut EditorSession,
    checkpoint: EditCheckpoint,
    projection: Result<T, E>,
) -> Result<T, PasteProjectionError>
where
    E: ToString,
{
    match projection {
        Ok(value) => {
            session
                .finalize_checkpoint(checkpoint)
                .map_err(PasteProjectionError::Finalize)?;
            Ok(value)
        }
        Err(error) => {
            let projection = error.to_string();
            match session.restore_checkpoint(checkpoint) {
                Ok(()) => Err(PasteProjectionError::Projection(projection)),
                Err(rollback) => Err(PasteProjectionError::Rollback {
                    projection,
                    rollback,
                }),
            }
        }
    }
}

impl EditorApp {
    /// Cmd/Ctrl+C and Cmd/Ctrl+V.
    pub(in crate::app) fn clipboard_actions(&mut self, ctx: &egui::Context) {
        if self.keys_blocked(ctx)
            || self.quick_open.is_some()
            || !matches!(self.prefs.tool, Tool::Select | Tool::Paint | Tool::Height)
        {
            return;
        }
        let (copy, paste) = ctx.input(|i| {
            (
                self.keymap.pressed(i, "edit.copy_tiles"),
                self.keymap.pressed(i, "edit.paste_tiles"),
            )
        });
        if copy {
            self.copy_tile_selection();
        } else if paste {
            self.paste_tiles();
        }
    }

    /// Remember the selected tiles of the viewed plane.
    pub(in crate::app) fn copy_tile_selection(&mut self) {
        let plane = self.interaction.view_plane;
        let tiles: Vec<_> = self
            .interaction
            .selection_tiles
            .iter()
            .filter_map(|&tile| Some((tile, self.current_tile((plane, tile.0, tile.1))?)))
            .collect();
        match TileClipboard::from_tiles(tiles) {
            Some(clipboard) => {
                let count = clipboard.len();
                self.clipboard = Some(clipboard);
                self.notice = Some((
                    format!("Copied {count} tile{}", if count == 1 { "" } else { "s" }),
                    Instant::now(),
                ));
            }
            None => {
                self.notice = Some((
                    "Select tiles first (Select tool, tile mode) to copy them".to_owned(),
                    Instant::now(),
                ));
            }
        }
    }

    /// Paste the copied tiles with their south-west corner on the hovered tile.
    pub(in crate::app) fn paste_tiles(&mut self) {
        if let Some(origin) = self.interaction.hover.tile.map(|pick| pick.tile) {
            self.paste_tiles_at(origin);
        }
    }

    /// Paste the copied tiles with their south-west corner on `origin`.
    pub(in crate::app) fn paste_tiles_at(&mut self, origin: (i32, i32)) {
        let Some(clipboard) = self.clipboard.clone() else {
            self.notice = Some(("Nothing copied yet".to_owned(), Instant::now()));
            return;
        };
        let plane_index = self.interaction.view_plane;
        let source_plane = SourcePlane::new(plane_index)
            .unwrap_or_else(|| unreachable!("editor view planes are always validated"));
        let intents = clipboard.intents_at(origin, |tile, height| {
            self.quantize_height((plane_index, tile.0, tile.1), height)
        });
        let patch = match build_terrain_paint_patch(self.session.document(), source_plane, &intents)
        {
            Ok(patch) => patch,
            Err(error) => {
                self.notice = Some((
                    format!("Paste was not committed: {error:?}"),
                    Instant::now(),
                ));
                return;
            }
        };
        let checkpoint = match self.session.commit_checkpointed(patch) {
            Ok(Some((_, checkpoint))) => checkpoint,
            Ok(None) => {
                self.notice = Some((
                    "Paste made no changes: the tiles already match".to_owned(),
                    Instant::now(),
                ));
                return;
            }
            Err(error) => {
                self.notice = Some((
                    format!("Paste was rejected by the semantic session: {error:?}"),
                    Instant::now(),
                ));
                return;
            }
        };
        let tiles = clipboard.tiles_at(origin);
        let keys: Vec<TileKey> = tiles.iter().map(|&(x, y)| (plane_index, x, y)).collect();
        let projection = self.project_document_tiles(&[], &keys);
        if let Err(error) = finish_paste_projection(&mut self.session, checkpoint, projection) {
            let message = match error {
                PasteProjectionError::Projection(message) => {
                    format!("Paste was rolled back: {message}")
                }
                PasteProjectionError::Rollback {
                    projection,
                    rollback,
                } => format!(
                    "Paste projection failed and semantic rollback failed: {projection} ({rollback:?})"
                ),
                PasteProjectionError::Finalize(error) => {
                    format!("Paste projection succeeded but semantic finalize failed: {error:?}")
                }
            };
            self.notice = Some((message, Instant::now()));
            return;
        }
        let count = keys.len();
        self.push_step(EditStep::semantic_path(keys, true));
        self.interaction.selection_tiles = tiles.into_iter().collect();
        self.notice = Some((
            format!("Pasted {count} tile{}", if count == 1 { "" } else { "s" }),
            Instant::now(),
        ));
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
        let pasted = vertex((3264, 3200));
        let mut session =
            EditorSession::new(MapDocument::from_parts([], [(prior, 0), (pasted, 0)], []));
        session
            .commit(height_patch(prior, 0, 32))
            .unwrap_or_else(|error| unreachable!("valid setup commit: {error:?}"));
        session
            .undo()
            .unwrap_or_else(|error| unreachable!("valid setup undo: {error:?}"));
        assert!(session.can_redo());
        (session, pasted)
    }

    #[test]
    fn failed_paste_projection_restores_exact_pre_paste_session() {
        let (mut session, pasted) = session_with_redo();
        let before = session.clone();
        let (_, checkpoint) = session
            .commit_checkpointed(height_patch(pasted, 0, 64))
            .unwrap_or_else(|error| unreachable!("valid Paste commit: {error:?}"))
            .unwrap_or_else(|| unreachable!("Paste changes semantic state"));

        let result = finish_paste_projection(
            &mut session,
            checkpoint,
            Result::<(), _>::Err("injected Paste projection failure"),
        );

        assert_eq!(
            result,
            Err(PasteProjectionError::Projection(
                "injected Paste projection failure".to_owned()
            ))
        );
        assert_eq!(session, before);
    }

    #[test]
    fn successful_paste_projection_finalizes_once_and_invalidates_prior_redo() {
        let (mut session, pasted) = session_with_redo();
        let revision_before = session.revision();
        let (_, checkpoint) = session
            .commit_checkpointed(height_patch(pasted, 0, 64))
            .unwrap_or_else(|error| unreachable!("valid Paste commit: {error:?}"))
            .unwrap_or_else(|| unreachable!("Paste changes semantic state"));

        finish_paste_projection(&mut session, checkpoint, Result::<(), &str>::Ok(()))
            .unwrap_or_else(|error| unreachable!("Paste finalizes: {error:?}"));

        assert_eq!(session.revision(), revision_before + 1);
        assert_eq!(session.document().height_at(pasted), Some(64));
        assert!(session.can_undo());
        assert!(!session.can_redo());
    }
}
