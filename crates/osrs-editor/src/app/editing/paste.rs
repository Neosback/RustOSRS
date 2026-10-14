//! Copy and paste of the selected tiles (terrain channels and heights).
//!
//! A paste plans one Paint-style patch against the authoritative session, commits it once, then
//! projects the authoritative result into the worker mirrors. It is recorded with the semantic
//! path-history route (terrain plus heights from the document), so undo and redo reuse the
//! existing projection.

use super::*;
use crate::app::paint_commit::build_terrain_paint_patch;
use crate::app::tile_clipboard::TileClipboard;

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
        match self.session.commit(patch) {
            Ok(Some(_)) => {}
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
        }
        let tiles = clipboard.tiles_at(origin);
        let keys: Vec<TileKey> = tiles.iter().map(|&(x, y)| (plane_index, x, y)).collect();
        if let Err(message) = self.project_document_tiles(&[], &keys) {
            let rolled_back = matches!(self.session.undo(), Ok(Some(_)));
            self.notice = Some((
                if rolled_back {
                    format!("Paste was rolled back: {message}")
                } else {
                    format!("Paste projection failed and rollback failed: {message}")
                },
                Instant::now(),
            ));
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
