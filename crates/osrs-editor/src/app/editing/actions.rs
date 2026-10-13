//! One-shot terrain actions: selection fills, overlay clear, and click-to-apply brushes.

use super::*;

impl EditorApp {
    /// Apply the Height tool's op to every selected tile (one undo step).
    pub(in crate::app) fn height_on_selection(&mut self) {
        let mut tiles: Vec<(i32, i32)> = self.interaction.selection_tiles.iter().copied().collect();
        tiles.sort_unstable();
        if tiles.is_empty() {
            return;
        }
        self.begin_stroke();
        let Some(mut stroke) = self.stroke.take() else {
            return;
        };
        let plane = self.interaction.view_plane;
        let source_plane = SourcePlane::new(plane)
            .unwrap_or_else(|| unreachable!("editor view planes are always validated"));
        let mut tool = self.prefs.height_tool.clone();
        tool.falloff = crate::heights::Falloff::Flat;
        // Flatten levels the whole selection to one height: the first selected tile's.
        let anchor = tiles
            .first()
            .and_then(|t| self.current_tile((plane, t.0, t.1)))
            .map_or(0, |v| v.height);
        let spec = HeightStrokeSpec::new(
            source_plane,
            tool,
            0,
            crate::paint::BrushShape::Square,
            anchor,
            stroke.seed,
        );
        stroke.height_spec = Some(spec.clone());
        let mut edits = Vec::new();
        for tile in tiles {
            let center = MapTile::new(tile.0, tile.1);
            stroke.height_centers.push(center);
            let lookup = |t: (i32, i32)| self.current_tile((plane, t.0, t.1)).map(|v| v.height);
            let changes = spec.apply_sample(center, &lookup);
            for (t, height) in changes {
                let key = (plane, t.0, t.1);
                if let Some(mut after) = self.current_tile(key) {
                    after.height = height;
                    self.record(&mut stroke, key, after, &mut edits);
                }
            }
        }
        self.send_edits(edits);
        self.stroke = Some(stroke);
        self.end_stroke();
    }

    /// Apply the brush to every selected tile (one undo step).
    pub(in crate::app) fn fill_selection(&mut self) {
        let tiles: Vec<(i32, i32)> = self.interaction.selection_tiles.iter().copied().collect();
        if tiles.is_empty() || self.prefs.brush.is_empty() {
            return;
        }
        self.begin_stroke();
        let Some(mut stroke) = self.stroke.take() else {
            return;
        };
        let brush = stroke
            .paint_brush
            .get_or_insert_with(|| self.prefs.brush.clone())
            .clone();
        let plane = self.interaction.view_plane;
        let mut edits = Vec::new();
        for (x, y) in tiles {
            let key = (plane, x, y);
            if let Some(before) = self.current_tile(key) {
                let after = brush.apply(before);
                self.record(&mut stroke, key, after, &mut edits);
            }
        }
        self.send_edits(edits);
        self.stroke = Some(stroke);
        self.end_stroke();
    }

    /// Remove a tile's overlay (its shape and rotation with it): one semantic commit and one undo
    /// step, planned through the same Paint planner as every other terrain stroke.
    pub(in crate::app) fn clear_overlay_at(&mut self, tile: (i32, i32)) {
        let key = (self.interaction.view_plane, tile.0, tile.1);
        let Some(before) = self.current_tile(key) else {
            return;
        };
        self.begin_stroke();
        let Some(mut stroke) = self.stroke.take() else {
            return;
        };
        let brush = stroke
            .paint_brush
            .get_or_insert_with(overlay_clear_brush)
            .clone();
        let mut edits = Vec::new();
        self.record(&mut stroke, key, brush.apply(before), &mut edits);
        self.send_edits(edits);
        self.stroke = Some(stroke);
        self.end_stroke();
    }

    /// One click without a drag (the context menu's "paint here" and "sculpt here").
    pub(in crate::app) fn apply_brush_at(&mut self, tile: (i32, i32)) {
        match self.prefs.tool {
            Tool::Paint => {
                if self.prefs.brush.is_empty() {
                    self.notice = Some((
                        "Nothing armed: tick a channel in the drawer".to_owned(),
                        Instant::now(),
                    ));
                    return;
                }
                self.begin_stroke();
                self.paint_at(tile);
                self.end_stroke();
            }
            Tool::Height => {
                self.begin_stroke();
                self.sculpt_at(tile);
                self.end_stroke();
            }
            Tool::Spline | Tool::Select | Tool::Objects => {}
        }
    }
}
