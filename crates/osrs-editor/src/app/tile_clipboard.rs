//! Copy and paste of a tile selection: terrain channels and heights, as one semantic commit.
//!
//! The clipboard is plain data (tile values relative to the selection's south-west corner).
//! Pasting plans a Paint-style patch against the authoritative document with every channel set,
//! so it is one `MapPatch`, one history entry, and reuses the Paint planner and the semantic
//! path-history projection for undo and redo.

use super::paint_commit::TerrainPaintIntent;
use crate::paint::TileValues;
use osrs_core::ids::{FloorOverlayId, FloorUnderlayId};
use osrs_edit::{OverlayRotation, OverlayShape};

#[cfg(test)]
mod tests;

/// Tile values keyed by their offset from the copied area's minimum corner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TileClipboard {
    tiles: Vec<((i32, i32), TileValues)>,
}

impl TileClipboard {
    /// Build a clipboard from absolute `(tile, values)` pairs. `None` when there are none.
    pub(super) fn from_tiles(
        tiles: impl IntoIterator<Item = ((i32, i32), TileValues)>,
    ) -> Option<Self> {
        let tiles: Vec<_> = tiles.into_iter().collect();
        let min_x = tiles.iter().map(|((x, _), _)| *x).min()?;
        let min_y = tiles.iter().map(|((_, y), _)| *y).min()?;
        let mut relative: Vec<_> = tiles
            .into_iter()
            .map(|((x, y), values)| ((x - min_x, y - min_y), values))
            .collect();
        relative.sort_by_key(|(offset, _)| *offset);
        Some(Self { tiles: relative })
    }

    pub(super) fn len(&self) -> usize {
        self.tiles.len()
    }

    /// The pasted tiles' world coordinates when the area's minimum corner lands on `origin`.
    pub(super) fn tiles_at(&self, origin: (i32, i32)) -> Vec<(i32, i32)> {
        self.tiles
            .iter()
            .map(|((dx, dy), _)| (origin.0 + dx, origin.1 + dy))
            .collect()
    }

    /// One intent per tile, setting every channel. `height_for` turns the copied height into the
    /// nearest height the target tile can store.
    pub(super) fn intents_at(
        &self,
        origin: (i32, i32),
        height_for: impl Fn((i32, i32), i32) -> i32,
    ) -> Vec<TerrainPaintIntent> {
        self.tiles
            .iter()
            .map(|((dx, dy), values)| {
                let tile = (origin.0 + dx, origin.1 + dy);
                let mut intent =
                    TerrainPaintIntent::new(osrs_core::coords::MapTile::new(tile.0, tile.1));
                intent.underlay = Some(
                    (values.underlay != 0)
                        .then(|| FloorUnderlayId::new(u32::from(values.underlay))),
                );
                intent.overlay = Some(
                    (values.overlay != 0).then(|| FloorOverlayId::new(u32::from(values.overlay))),
                );
                if values.overlay != 0 {
                    intent.overlay_shape = Some((
                        OverlayShape::new(values.shape.min(OverlayShape::MAX))
                            .unwrap_or_else(|| unreachable!("shape is clamped to its maximum")),
                        OverlayRotation::new(values.rotation & OverlayRotation::MAX)
                            .unwrap_or_else(|| unreachable!("rotation is masked to its maximum")),
                    ));
                }
                intent.height = Some(height_for(tile, values.height));
                intent.flags = Some(values.flags);
                intent
            })
            .collect()
    }
}
