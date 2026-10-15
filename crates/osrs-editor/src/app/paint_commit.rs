//! Brush-to-domain adapter. Pure semantic Paint planning lives in `osrs-edit`.

use crate::paint::{Brush, Fill, IdChannel};
pub(super) use osrs_edit::{PaintCommitError, TerrainPaintIntent, build_terrain_paint_patch};

/// The brush that removes a tile's overlay (shape and rotation go with it) and touches nothing else.
pub(super) fn overlay_clear_brush() -> Brush {
    Brush {
        overlay: IdChannel {
            armed: true,
            fill: Fill::Clear,
        },
        ..Brush::default()
    }
}
