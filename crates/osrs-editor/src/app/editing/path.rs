//! Runtime terrain-path commit and worker projection.
//!
//! Path fitting stays in `terrain_path` / `spline_tool`, and pure semantic patch planning stays in
//! `app/path_commit.rs`. This module owns only the application boundary: plan against the persistent
//! editor session, preflight the transitional worker representation, commit once, then project.

use super::{EditStep, EditorApp, PathTile, TileKey};
use crate::app::path_commit::{TerrainPathIntent, build_terrain_path_patch};
use crate::heights::{decode_height, encode_height};
use crate::paint::TileValues;
use osrs_core::coords::{MapTile, SourcePlane};
use osrs_edit::{HeightVertexCoord, MapPosition, MapQuery, TerrainTile};
use std::collections::HashSet;
use web_time::Instant;

fn project_semantic_terrain(values: &mut TileValues, terrain: TerrainTile) -> Option<()> {
    values.underlay = match terrain.underlay {
        Some(id) => u16::try_from(id.get()).ok()?,
        None => 0,
    };
    match terrain.overlay {
        Some(overlay) => {
            values.overlay = u16::try_from(overlay.id.get()).ok()?;
            values.shape = overlay.shape.get();
            values.rotation = overlay.rotation.get();
        }
        None => {
            values.overlay = 0;
            values.shape = 0;
            values.rotation = 0;
        }
    }
    values.flags = terrain.flags;
    Some(())
}

fn quantize_path_height(
    query: &impl MapQuery,
    plane: SourcePlane,
    tile: MapTile,
    desired: i32,
) -> Result<i32, &'static str> {
    let plane_index = plane.index().get();
    let base = if plane_index == 0 {
        0
    } else {
        let below = SourcePlane::new(plane_index - 1)
            .unwrap_or_else(|| unreachable!("a non-zero source plane always has a lower plane"));
        query
            .height_at(HeightVertexCoord::new(tile, below))
            .ok_or("the semantic lower-plane height is missing")?
    };
    Ok(decode_height(base, encode_height(base, desired)))
}

impl EditorApp {
    /// Lay one fitted terrain path as one persistent semantic transaction.
    ///
    /// The scratch document below is projection preflight only: it is cloned from authoritative
    /// session state and receives the exact patch that will later be committed. It never becomes
    /// editor authority and is discarded before worker mutation.
    pub(super) fn apply_terrain_path(&mut self, tiles: &[PathTile]) -> usize {
        if tiles.is_empty() {
            return 0;
        }
        let Some(plane) = SourcePlane::new(self.view_plane) else {
            self.notice = Some((
                "The path uses an invalid source plane".to_owned(),
                Instant::now(),
            ));
            return 0;
        };

        let brush = self.prefs.brush.clone();
        let mut intents = Vec::new();
        for path_tile in tiles {
            let key = (self.view_plane, path_tile.tile.0, path_tile.tile.1);
            if self.current_tile(key).is_none() {
                continue;
            }
            let map_tile = MapTile::new(path_tile.tile.0, path_tile.tile.1);
            let height = if brush.height.armed {
                match quantize_path_height(
                    self.session.document(),
                    plane,
                    map_tile,
                    brush.height.value,
                ) {
                    Ok(height) => Some(height),
                    Err(message) => {
                        self.notice = Some((
                            format!("The path was not laid at {:?}: {message}", path_tile.tile),
                            Instant::now(),
                        ));
                        return 0;
                    }
                }
            } else {
                None
            };
            let flags = brush.flags.armed.then_some(brush.flags.bits);
            intents.push(TerrainPathIntent::from_path_tile(*path_tile, height, flags));
        }

        if intents.is_empty() {
            return 0;
        }

        let patch = match build_terrain_path_patch(self.session.document(), plane, &intents) {
            Ok(patch) => patch,
            Err(error) => {
                self.notice = Some((
                    format!("The path was not laid: invalid semantic commit data ({error:?})"),
                    Instant::now(),
                ));
                return 0;
            }
        };
        if patch.is_empty() {
            self.notice = Some((
                "The path made no semantic changes".to_owned(),
                Instant::now(),
            ));
            return 0;
        }

        // Prove every touched semantic result can be represented by the transitional TileEdit
        // adapter before the authoritative session is mutated.
        let mut projected_document = self.session.document().clone();
        if let Err(error) = projected_document.apply_patch(&patch) {
            self.notice = Some((
                format!("The path was not laid: semantic preflight failed ({error:?})"),
                Instant::now(),
            ));
            return 0;
        }

        let mut projected = HashSet::new();
        let mut changes: Vec<(TileKey, TileValues, TileValues)> = Vec::new();
        for intent in &intents {
            if !projected.insert(intent.tile) {
                continue;
            }
            let key = (self.view_plane, intent.tile.0, intent.tile.1);
            let Some(before) = self.current_tile(key) else {
                self.notice = Some((
                    format!(
                        "The path was not laid: tile {:?} disappeared before projection",
                        intent.tile
                    ),
                    Instant::now(),
                ));
                return 0;
            };
            let mut after = before;
            let map_tile = MapTile::new(intent.tile.0, intent.tile.1);
            let position = MapPosition::new(map_tile, plane);
            if project_semantic_terrain(&mut after, projected_document.terrain_at(position))
                .is_none()
            {
                self.notice = Some((
                    "The path was not laid: semantic terrain IDs exceed the legacy adapter range"
                        .to_owned(),
                    Instant::now(),
                ));
                return 0;
            }
            if brush.height.armed {
                let vertex = HeightVertexCoord::new(map_tile, plane);
                let Some(height) = projected_document.height_at(vertex) else {
                    self.notice = Some((
                        format!(
                            "The path was not laid: semantic height is missing at {:?}",
                            intent.tile
                        ),
                        Instant::now(),
                    ));
                    return 0;
                };
                after.height = height;
            }
            if after != before {
                changes.push((key, before, after));
            }
        }

        if changes.is_empty() {
            self.notice = Some((
                "The path semantic patch cannot be represented as a worker change".to_owned(),
                Instant::now(),
            ));
            return 0;
        }

        match self.session.commit(patch) {
            Ok(Some(_)) => {}
            Ok(None) => {
                self.notice = Some((
                    "The path made no semantic changes".to_owned(),
                    Instant::now(),
                ));
                return 0;
            }
            Err(error) => {
                self.notice = Some((
                    format!("The path was rejected by the semantic session: {error:?}"),
                    Instant::now(),
                ));
                return 0;
            }
        }

        let mut edits = Vec::with_capacity(changes.len());
        for (key, before, after) in &changes {
            if after.height != before.height {
                self.height_edited.insert(*key);
            }
            self.edited.insert(*key, *after);
            edits.push(self.edit_for(*key, *after));
        }
        let count = changes.len();
        self.send_edits(edits);
        self.push_step(EditStep {
            changes,
            ..EditStep::default()
        });
        self.notice = Some((
            format!(
                "Changed {count} path tile{}",
                if count == 1 { "" } else { "s" }
            ),
            Instant::now(),
        ));
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::EditorSession;
    use osrs_core::ids::FloorOverlayId;
    use osrs_edit::{MapDocument, MapQuery, OverlayRotation, OverlayShape, TerrainOverlay};

    fn plane(value: u8) -> SourcePlane {
        SourcePlane::new(value).unwrap_or_else(|| unreachable!("test plane is valid"))
    }

    #[test]
    fn upper_plane_path_height_requires_the_semantic_lower_plane_base() {
        let tile = MapTile::new(3200, 3200);
        let document =
            MapDocument::from_parts([], [(HeightVertexCoord::new(tile, plane(1)), 100)], []);

        assert_eq!(
            quantize_path_height(&document, plane(1), tile, 64),
            Err("the semantic lower-plane height is missing")
        );
    }

    #[test]
    fn one_path_patch_is_one_persistent_session_revision() {
        let tile = MapTile::new(3200, 3200);
        let coord = MapPosition::new(tile, plane(0));
        let overlay = TerrainOverlay {
            id: FloorOverlayId::new(7),
            shape: OverlayShape::new(0).unwrap_or_else(|| unreachable!("shape zero is valid")),
            rotation: OverlayRotation::new(0)
                .unwrap_or_else(|| unreachable!("rotation zero is valid")),
        };
        let document =
            MapDocument::from_parts([], [(HeightVertexCoord::new(tile, plane(0)), 0)], []);
        let mut session = EditorSession::new(document);
        let intent = TerrainPathIntent {
            tile: (3200, 3200),
            overlay: Some(7),
            shape: 0,
            rotation: 0,
            height: None,
            flags: None,
        };
        let patch = build_terrain_path_patch(session.document(), plane(0), &[intent])
            .unwrap_or_else(|error| unreachable!("valid path patch: {error:?}"));

        let commit = session
            .commit(patch)
            .unwrap_or_else(|error| unreachable!("path commit should succeed: {error:?}"))
            .unwrap_or_else(|| unreachable!("path changes semantic terrain"));

        assert_eq!(commit.revision, 1);
        assert_eq!(session.revision(), 1);
        assert!(session.can_undo());
        assert_eq!(session.document().terrain_at(coord).overlay, Some(overlay));
    }
}
