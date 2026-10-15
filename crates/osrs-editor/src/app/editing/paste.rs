//! Copy and paste of the selected tiles (terrain channels and heights).
//!
//! A paste plans one Paint-style patch against the authoritative session, executes it through the
//! application edit gateway, and records chronological history only after semantic commit,
//! projection planning, and checkpoint finalization all succeed. The resulting legacy worker
//! projection is already validated and therefore applied infallibly after gateway success.

use super::path::project_semantic_path_values;
use super::*;
use crate::app::edit_runtime::{EditExecution, EditExecutionError};
use crate::app::paint_commit::build_terrain_paint_patch;
use crate::app::tile_clipboard::TileClipboard;

/// One already-validated Paste projection from authoritative semantic state into the transitional
/// tile mirror.
type PasteProjection = Vec<(TileKey, TileValues, TileValues)>;

/// Prepare the complete legacy Paste projection from the authoritative post-commit document.
///
/// This is deliberately pure. Any missing legacy tile or unrepresentable semantic result fails
/// while `EditRuntime` still owns the active semantic checkpoint, so the gateway can restore the
/// exact pre-Paste session state. Applying a successful plan later cannot fail.
fn plan_paste_projection(
    document: &osrs_edit::MapDocument,
    current: &[(TileKey, Option<TileValues>)],
) -> Result<PasteProjection, &'static str> {
    let mut projected = Vec::with_capacity(current.len());
    for (key, current) in current {
        let current =
            current.ok_or("a pasted tile is no longer available to the projection adapter")?;
        let values = project_semantic_path_values(document, *key, current, true)
            .ok_or("semantic Paste state cannot be represented by the legacy projection")?;
        projected.push((*key, current, values));
    }
    Ok(projected)
}

impl EditorApp {
    /// Cmd/Ctrl+C and Cmd/Ctrl+V.
    pub(in crate::app) fn clipboard_actions(&mut self, ctx: &egui::Context) {
        if self.keys_blocked(ctx)
            || self.ui.quick_open.is_some()
            || !matches!(
                self.settings.prefs.tool,
                Tool::Select | Tool::Paint | Tool::Height
            )
        {
            return;
        }
        let (copy, paste) = ctx.input(|i| {
            (
                self.settings.keymap.pressed(i, "edit.copy_tiles"),
                self.settings.keymap.pressed(i, "edit.paste_tiles"),
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
        let plane = self.interaction.view_plane();
        let tiles: Vec<_> = self
            .interaction
            .selection_tiles()
            .iter()
            .filter_map(|&tile| Some((tile, self.current_tile((plane, tile.0, tile.1))?)))
            .collect();
        match TileClipboard::from_tiles(tiles) {
            Some(clipboard) => {
                let count = clipboard.len();
                self.interaction.copy_tiles(clipboard);
                self.ui.notify(format!(
                    "Copied {count} tile{}",
                    if count == 1 { "" } else { "s" }
                ));
            }
            None => {
                self.ui
                    .notify("Select tiles first (Select tool, tile mode) to copy them".to_owned());
            }
        }
    }

    /// Paste the copied tiles with their south-west corner on the hovered tile.
    pub(in crate::app) fn paste_tiles(&mut self) {
        if let Some(origin) = self.interaction.hover().tile.map(|pick| pick.tile) {
            self.paste_tiles_at(origin);
        }
    }

    /// Paste the copied tiles with their south-west corner on `origin`.
    pub(in crate::app) fn paste_tiles_at(&mut self, origin: (i32, i32)) {
        let Some(clipboard) = self.interaction.clipboard().clone() else {
            self.ui.notify("Nothing copied yet".to_owned());
            return;
        };
        let plane_index = self.interaction.view_plane();
        let source_plane = SourcePlane::new(plane_index)
            .unwrap_or_else(|| unreachable!("editor view planes are always validated"));
        let intents = clipboard.intents_at(origin, |tile, height| {
            self.quantize_height((plane_index, tile.0, tile.1), height)
        });
        let patch = match build_terrain_paint_patch(self.session.document(), source_plane, &intents)
        {
            Ok(patch) => patch,
            Err(error) => {
                self.ui
                    .notify(format!("Paste was not committed: {error:?}"));
                return;
            }
        };

        let tiles = clipboard.tiles_at(origin);
        let keys: Vec<TileKey> = tiles.iter().map(|&(x, y)| (plane_index, x, y)).collect();
        let current: Vec<_> = keys
            .iter()
            .copied()
            .map(|key| (key, self.current_tile(key)))
            .collect();

        let execution = self.session.execute_named("paste", patch, |document| {
            plan_paste_projection(document, &current)
        });
        let (receipt, projection) = match execution {
            Ok(EditExecution::NoChange) => {
                self.ui
                    .notify("Paste made no changes: the tiles already match".to_owned());
                return;
            }
            Ok(EditExecution::Committed {
                receipt,
                projection,
            }) => (receipt, projection),
            Err(EditExecutionError::Semantic(error)) => {
                self.ui.notify(format!(
                    "Paste was rejected by the semantic session: {error:?}"
                ));
                return;
            }
            Err(EditExecutionError::Projection(message)) => {
                self.ui.notify(format!("Paste was rolled back: {message}"));
                return;
            }
            Err(EditExecutionError::Rollback {
                projection,
                rollback,
            }) => {
                self.ui.notify(
                    format!(
                        "Paste projection failed and semantic rollback failed: {projection} ({rollback:?})"
                    ));
                return;
            }
            Err(EditExecutionError::Finalize(error)) => {
                self.ui.notify(format!(
                    "Paste projection succeeded but semantic finalize failed: {error:?}"
                ));
                return;
            }
        };

        // Projection planning was the fallible boundary. From here the semantic transaction is
        // finalized, so only apply the already-validated mirror values. Rebuild scope comes from
        // the authoritative semantic receipt rather than the transitional TileEdit adapter.
        let mut edits = Vec::new();
        for (key, current, values) in projection {
            self.terrain.height_edited.insert(key);
            if values != current {
                self.terrain.edited.insert(key, values);
                edits.push(self.edit_for(key, values));
            }
        }
        self.world.project_tile_edits(edits);
        self.world.invalidate_semantic_edit(&receipt);

        let count = keys.len();
        self.push_step(EditStep::semantic_path(keys, true));
        self.interaction.replace_tiles(tiles);
        self.ui.notify(format!(
            "Pasted {count} tile{}",
            if count == 1 { "" } else { "s" }
        ));
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
        let pasted = vertex((3264, 3200));
        let mut runtime = EditRuntime::new(EditorSession::new(MapDocument::from_parts(
            [],
            [(prior, 0), (pasted, 0)],
            [],
        )));
        runtime
            .commit(height_patch(prior, 0, 32))
            .unwrap_or_else(|error| unreachable!("valid setup commit: {error:?}"));
        runtime
            .undo()
            .unwrap_or_else(|error| unreachable!("valid setup undo: {error:?}"));
        assert!(runtime.can_redo());
        (runtime, pasted)
    }

    #[test]
    fn failed_paste_projection_restores_exact_pre_paste_session() {
        let (mut runtime, pasted) = runtime_with_redo();
        let before = runtime.clone();
        let key = (0, pasted.corner.x, pasted.corner.y);
        let result = runtime.execute(height_patch(pasted, 0, 64), |document| {
            assert_eq!(document.height_at(pasted), Some(64));
            plan_paste_projection(document, &[(key, None)])
        });

        assert_eq!(
            result,
            Err(EditExecutionError::Projection(
                "a pasted tile is no longer available to the projection adapter"
            ))
        );
        assert_eq!(runtime, before);
    }

    #[test]
    fn successful_paste_projection_uses_post_commit_state_and_finalizes_once() {
        let (mut runtime, pasted) = runtime_with_redo();
        let revision_before = runtime.revision();
        let key = (0, pasted.corner.x, pasted.corner.y);
        let current = TileValues::default();
        let result = runtime
            .execute(height_patch(pasted, 0, 64), |document| {
                assert_eq!(document.height_at(pasted), Some(64));
                plan_paste_projection(document, &[(key, Some(current))])
            })
            .unwrap_or_else(|error| unreachable!("Paste gateway execution succeeds: {error:?}"));

        let EditExecution::Committed { projection, .. } = result else {
            unreachable!("non-empty Paste patch commits")
        };
        assert_eq!(
            projection,
            vec![(
                key,
                current,
                TileValues {
                    height: 64,
                    ..current
                }
            )]
        );
        assert_eq!(runtime.revision(), revision_before + 1);
        assert_eq!(runtime.document().height_at(pasted), Some(64));
        assert!(runtime.can_undo());
        assert!(!runtime.can_redo());
    }
}
