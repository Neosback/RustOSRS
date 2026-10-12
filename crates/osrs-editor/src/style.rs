//! One source of truth for the colours the editor draws with outside the UI theme: the viewport's
//! indicators (hover tile, brush footprint, selections, the path preview), the grid, the HUD and the
//! "transparent" checkerboard.
//!
//! Each colour is a [`ColorKey`] with a default, an optional user override (with alpha) kept in the
//! settings, and a label for the Colors settings page. Drawing code asks [`Colors`] and never spells a
//! colour itself, so changing one entry (or the page's reset) changes it everywhere. The UI theme
//! (panels, buttons, text) is separate: see `theme.rs`.

use eframe::egui::Color32;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A named colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorKey {
    // Viewport indicators.
    HoverTileOutline,
    HoverTileFill,
    BrushOutline,
    BrushFill,
    BrushEmptyOutline,
    BrushEmptyFill,
    ObjectHoverOutline,
    ObjectHoverFill,
    SelectionOutline,
    SelectionFill,
    TileSelectionOutline,
    TileSelectionFill,
    // The path tool.
    PathLine,
    PathGhost,
    PathFill,
    PathEdge,
    // The grid.
    GridTile,
    GridZone,
    GridRegion,
    // HUD.
    HudBackground,
    HudText,
    HudLabel,
    // Palettes.
    CheckerLight,
    CheckerDark,
}

/// How a colour is grouped on the Colors page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorGroup {
    Indicators,
    Path,
    Grid,
    Hud,
    Palettes,
}

impl ColorGroup {
    pub const ALL: [Self; 5] = [
        Self::Indicators,
        Self::Path,
        Self::Grid,
        Self::Hud,
        Self::Palettes,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Indicators => "Indicators in the viewport",
            Self::Path => "Path tool",
            Self::Grid => "Grid",
            Self::Hud => "HUD",
            Self::Palettes => "Palettes",
        }
    }
}

impl ColorKey {
    pub const ALL: [Self; 24] = [
        Self::HoverTileOutline,
        Self::HoverTileFill,
        Self::BrushOutline,
        Self::BrushFill,
        Self::BrushEmptyOutline,
        Self::BrushEmptyFill,
        Self::ObjectHoverOutline,
        Self::ObjectHoverFill,
        Self::SelectionOutline,
        Self::SelectionFill,
        Self::TileSelectionOutline,
        Self::TileSelectionFill,
        Self::PathLine,
        Self::PathGhost,
        Self::PathFill,
        Self::PathEdge,
        Self::GridTile,
        Self::GridZone,
        Self::GridRegion,
        Self::HudBackground,
        Self::HudText,
        Self::HudLabel,
        Self::CheckerLight,
        Self::CheckerDark,
    ];

    /// The settings key.
    pub const fn id(self) -> &'static str {
        match self {
            Self::HoverTileOutline => "hover_tile_outline",
            Self::HoverTileFill => "hover_tile_fill",
            Self::BrushOutline => "brush_outline",
            Self::BrushFill => "brush_fill",
            Self::BrushEmptyOutline => "brush_empty_outline",
            Self::BrushEmptyFill => "brush_empty_fill",
            Self::ObjectHoverOutline => "object_hover_outline",
            Self::ObjectHoverFill => "object_hover_fill",
            Self::SelectionOutline => "selection_outline",
            Self::SelectionFill => "selection_fill",
            Self::TileSelectionOutline => "tile_selection_outline",
            Self::TileSelectionFill => "tile_selection_fill",
            Self::PathLine => "path_line",
            Self::PathGhost => "path_ghost",
            Self::PathFill => "path_fill",
            Self::PathEdge => "path_edge",
            Self::GridTile => "grid_tile",
            Self::GridZone => "grid_zone",
            Self::GridRegion => "grid_region",
            Self::HudBackground => "hud_background",
            Self::HudText => "hud_text",
            Self::HudLabel => "hud_label",
            Self::CheckerLight => "checker_light",
            Self::CheckerDark => "checker_dark",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::HoverTileOutline => "Hovered tile: outline",
            Self::HoverTileFill => "Hovered tile: fill",
            Self::BrushOutline => "Brush footprint: outline",
            Self::BrushFill => "Brush footprint: fill",
            Self::BrushEmptyOutline => "Empty brush: outline",
            Self::BrushEmptyFill => "Empty brush: fill",
            Self::ObjectHoverOutline => "Hovered object: outline",
            Self::ObjectHoverFill => "Hovered object: fill",
            Self::SelectionOutline => "Selected object: outline",
            Self::SelectionFill => "Selected object: fill",
            Self::TileSelectionOutline => "Selected tiles: outline",
            Self::TileSelectionFill => "Selected tiles: fill",
            Self::PathLine => "Path line",
            Self::PathGhost => "Path: next segment",
            Self::PathFill => "Path preview: fill",
            Self::PathEdge => "Path preview: edges",
            Self::GridTile => "Tile lines",
            Self::GridZone => "Zone lines (8 tiles)",
            Self::GridRegion => "Region lines (64 tiles)",
            Self::HudBackground => "HUD background",
            Self::HudText => "HUD text",
            Self::HudLabel => "HUD labels",
            Self::CheckerLight => "Transparent: light squares",
            Self::CheckerDark => "Transparent: dark squares",
        }
    }

    pub const fn group(self) -> ColorGroup {
        match self {
            Self::HoverTileOutline
            | Self::HoverTileFill
            | Self::BrushOutline
            | Self::BrushFill
            | Self::BrushEmptyOutline
            | Self::BrushEmptyFill
            | Self::ObjectHoverOutline
            | Self::ObjectHoverFill
            | Self::SelectionOutline
            | Self::SelectionFill
            | Self::TileSelectionOutline
            | Self::TileSelectionFill => ColorGroup::Indicators,
            Self::PathLine | Self::PathGhost | Self::PathFill | Self::PathEdge => ColorGroup::Path,
            Self::GridTile | Self::GridZone | Self::GridRegion => ColorGroup::Grid,
            Self::HudBackground | Self::HudText | Self::HudLabel => ColorGroup::Hud,
            Self::CheckerLight | Self::CheckerDark => ColorGroup::Palettes,
        }
    }

    /// Unmultiplied RGBA.
    pub const fn default_rgba(self) -> [u8; 4] {
        match self {
            Self::HoverTileOutline => [0, 255, 255, 255],
            Self::HoverTileFill => [0, 255, 255, 28],
            Self::BrushOutline => [255, 170, 40, 255],
            Self::BrushFill => [255, 170, 40, 40],
            Self::BrushEmptyOutline => [190, 190, 190, 255],
            Self::BrushEmptyFill => [200, 200, 200, 22],
            Self::ObjectHoverOutline => [255, 255, 255, 90],
            Self::ObjectHoverFill => [255, 255, 255, 40],
            Self::SelectionOutline => [255, 200, 0, 255],
            Self::SelectionFill => [255, 190, 0, 60],
            Self::TileSelectionOutline => [120, 200, 255, 200],
            Self::TileSelectionFill => [80, 170, 255, 70],
            Self::PathLine => [255, 200, 80, 230],
            Self::PathGhost => [120, 200, 255, 200],
            Self::PathFill => [255, 176, 56, 120],
            Self::PathEdge => [255, 236, 190, 150],
            Self::GridTile => [255, 255, 255, 77],
            Self::GridZone => [77, 230, 255, 153],
            Self::GridRegion => [255, 153, 26, 242],
            Self::HudBackground => [0, 0, 0, 165],
            Self::HudText => [255, 255, 255, 255],
            Self::HudLabel => [255, 255, 255, 150],
            Self::CheckerLight => [104, 104, 112, 255],
            Self::CheckerDark => [62, 62, 70, 255],
        }
    }
}

/// The user's colour overrides (only the ones that differ from the defaults are stored).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Colors {
    overrides: BTreeMap<String, [u8; 4]>,
}

impl Colors {
    pub fn rgba(&self, key: ColorKey) -> [u8; 4] {
        self.overrides
            .get(key.id())
            .copied()
            .unwrap_or_else(|| key.default_rgba())
    }

    pub fn get(&self, key: ColorKey) -> Color32 {
        let [r, g, b, a] = self.rgba(key);
        Color32::from_rgba_unmultiplied(r, g, b, a)
    }

    /// The colour as straight `[r, g, b, a]` floats (for the renderer's line vertices).
    pub fn linear(&self, key: ColorKey) -> [f32; 4] {
        self.rgba(key).map(|v| f32::from(v) / 255.0)
    }

    pub fn set(&mut self, key: ColorKey, rgba: [u8; 4]) {
        if rgba == key.default_rgba() {
            self.overrides.remove(key.id());
        } else {
            self.overrides.insert(key.id().to_owned(), rgba);
        }
    }

    pub fn reset(&mut self, key: ColorKey) {
        self.overrides.remove(key.id());
    }

    pub fn reset_all(&mut self) {
        self.overrides.clear();
    }

    pub fn is_default(&self, key: ColorKey) -> bool {
        !self.overrides.contains_key(key.id())
    }

    /// Changes whenever any colour does (plugins rebuild their lines when it moves).
    pub fn stamp(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for (key, rgba) in &self.overrides {
            for byte in key.bytes().chain(rgba.iter().copied()) {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x0100_0000_01b3);
            }
        }
        hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_has_a_unique_settings_id_and_is_listed_once() {
        let mut ids: Vec<&str> = ColorKey::ALL.iter().map(|k| k.id()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), ColorKey::ALL.len());
    }

    #[test]
    fn overrides_replace_defaults_and_setting_the_default_clears_them() {
        let mut colors = Colors::default();
        let before = colors.stamp();
        assert_eq!(
            colors.rgba(ColorKey::GridTile),
            ColorKey::GridTile.default_rgba()
        );
        colors.set(ColorKey::GridTile, [1, 2, 3, 4]);
        assert_eq!(colors.rgba(ColorKey::GridTile), [1, 2, 3, 4]);
        assert_ne!(colors.stamp(), before);
        colors.set(ColorKey::GridTile, ColorKey::GridTile.default_rgba());
        assert!(colors.is_default(ColorKey::GridTile));
        assert_eq!(colors.stamp(), before);
    }
}
