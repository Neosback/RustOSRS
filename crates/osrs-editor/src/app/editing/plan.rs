//! Pure Paint/Height planning and semantic-to-preview projection helpers.
//!
//! These functions read `MapQuery` and never touch UI, renderer, or worker state.

use super::TileKey;
use crate::app::paint_commit::{PaintCommitError, TerrainPaintIntent, build_terrain_paint_patch};
use crate::paint::{Brush, Fill, TileValues};
use osrs_core::coords::{MapTile, SourcePlane};
use osrs_core::ids::{FloorOverlayId, FloorUnderlayId};
use osrs_edit::{
    HeightVertexCoord, MapPatch, MapPosition, MapQuery, OverlayRotation, OverlayShape,
};
use std::collections::BTreeMap;

pub(super) fn paint_intent_from_brush(
    brush: &Brush,
    tile: (i32, i32),
    after: TileValues,
) -> TerrainPaintIntent {
    let mut intent = TerrainPaintIntent::new(tile);
    if brush.underlay.armed {
        intent.underlay = Some(match brush.underlay.fill {
            Fill::Clear | Fill::Set(0) => None,
            Fill::Set(id) => Some(FloorUnderlayId::new(u32::from(id))),
        });
    }
    if brush.overlay.armed {
        intent.overlay = Some(match brush.overlay.fill {
            Fill::Clear | Fill::Set(0) => None,
            Fill::Set(id) => Some(FloorOverlayId::new(u32::from(id))),
        });
    }
    if brush.shape.armed && after.overlay != 0 {
        intent.overlay_shape = Some((
            OverlayShape::new(after.shape)
                .unwrap_or_else(|| unreachable!("Brush::apply clamps overlay shape")),
            OverlayRotation::new(after.rotation)
                .unwrap_or_else(|| unreachable!("Brush::apply clamps overlay rotation")),
        ));
    }
    intent.height = brush.height.armed.then_some(after.height);
    intent.flags = brush.flags.armed.then_some(after.flags);
    intent
}

pub(super) fn build_paint_stroke_patch(
    query: &impl MapQuery,
    brush: &Brush,
    changes: &[(TileKey, TileValues, TileValues)],
) -> Result<MapPatch, PaintCommitError> {
    let mut by_plane = BTreeMap::<SourcePlane, Vec<TerrainPaintIntent>>::new();
    for (key, _, after) in changes {
        let plane = SourcePlane::new(key.0)
            .unwrap_or_else(|| unreachable!("editor stroke planes are always validated"));
        by_plane
            .entry(plane)
            .or_default()
            .push(paint_intent_from_brush(brush, (key.1, key.2), *after));
    }

    let mut ops = Vec::new();
    for (plane, intents) in by_plane {
        let patch = build_terrain_paint_patch(query, plane, &intents)?;
        ops.extend_from_slice(patch.ops());
    }
    Ok(MapPatch::new(ops))
}

pub(super) fn project_paint_values(
    query: &impl MapQuery,
    brush: &Brush,
    key: TileKey,
    mut current: TileValues,
) -> Option<TileValues> {
    let plane = SourcePlane::new(key.0)?;
    let tile = MapTile::new(key.1, key.2);
    let terrain = query.terrain_at(MapPosition::new(tile, plane));

    if brush.underlay.armed {
        current.underlay = match terrain.underlay {
            Some(id) => u16::try_from(id.get()).ok()?,
            None => 0,
        };
    }
    if brush.overlay.armed {
        match terrain.overlay {
            Some(overlay) => {
                current.overlay = u16::try_from(overlay.id.get()).ok()?;
                current.shape = overlay.shape.get();
                current.rotation = overlay.rotation.get();
            }
            None => {
                current.overlay = 0;
                current.shape = 0;
                current.rotation = 0;
            }
        }
    } else if brush.shape.armed
        && let Some(overlay) = terrain.overlay
    {
        current.shape = overlay.shape.get();
        current.rotation = overlay.rotation.get();
    }
    if brush.height.armed {
        current.height = query.height_at(HeightVertexCoord::new(tile, plane))?;
    }
    if brush.flags.armed {
        current.flags = terrain.flags;
    }
    Some(current)
}

pub(super) fn project_height_values(
    query: &impl MapQuery,
    key: TileKey,
    mut current: TileValues,
) -> Option<TileValues> {
    let plane = SourcePlane::new(key.0)?;
    let tile = MapTile::new(key.1, key.2);
    current.height = query.height_at(HeightVertexCoord::new(tile, plane))?;
    Some(current)
}
