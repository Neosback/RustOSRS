//! The workspace chrome around the viewport (the Displee-style layout):
//!
//! * a **rail** of category icons on the dock edge; each swaps the dock panel's contents,
//! * the **dock panel** (inspector, view, graphics, plugins),
//! * the **quick tools** HUD (Space) and the **tile info** HUD,
//! * tool switching: keys that tap-to-switch and hold-to-peek, and `Esc` back to Select.
//!
//! The tool drawer is in `drawer.rs`, the floating toolbar in `floating.rs`. Everything is context
//! aware: the drawer exists only for tools with a palette, the tile HUD only over terrain, and the
//! quick HUD offers what the active tool can use.

mod dock;
mod tools;

pub use tools::{select_mode_icon, tool_icon};

use super::EditorApp;
use super::widgets::tab_label;
use crate::icons;
use crate::paint::{SelectMode, TileValues, Tool};
use crate::settings::{RailTab, ViewTab};
use eframe::egui::{self, Pos2, Rect, Sense, Stroke, Vec2};
use std::time::Duration;

/// Window width (points) under which labels collapse to icons and the dock narrows.
pub const COMPACT_WIDTH: f32 = 1100.0;
const RAIL_WIDTH: f32 = 44.0;
/// Holding a tool key longer than this peeks at the tool and returns on release.
const PEEK_AFTER: Duration = Duration::from_millis(280);

impl EditorApp {
    // ---------------------------------------------------------------- palette

    // ------------------------------------------------------------------ rail

    // -------------------------------------------------------------- quick HUD

    // ---------------------------------------------------------------- tile HUD

    /// The tile under the cursor, only while the cursor is over terrain.
    pub(super) fn tile_info_lines(&self) -> Option<Vec<(String, String)>> {
        // With nothing under the cursor the last tile stays, so the HUD does not blink out.
        let tile = self
            .interaction
            .hover()
            .tile
            .map(|pick| pick.tile)
            .or(self.interaction.last_hover_tile())?;
        let values = self.tile_now(tile)?;
        let mut lines = vec![
            (
                "Tile".to_owned(),
                format!(
                    "{}, {}  \u{b7}  plane {}",
                    tile.0,
                    tile.1,
                    self.interaction.view_plane()
                ),
            ),
            (
                "Region".to_owned(),
                crate::hud::region_id(tile.0 >> 6, tile.1 >> 6).to_string(),
            ),
            (
                "Overlay".to_owned(),
                if values.overlay == 0 {
                    "none".to_owned()
                } else {
                    format!(
                        "{}  \u{b7}  shape {}  rot {}",
                        values.overlay, values.shape, values.rotation
                    )
                },
            ),
            (
                "Underlay".to_owned(),
                if values.underlay == 0 {
                    "none".to_owned()
                } else {
                    values.underlay.to_string()
                },
            ),
            (
                "Height".to_owned(),
                format!(
                    "{}  \u{b7}  flags {:#04x}",
                    crate::paint::elevation(values.height),
                    values.flags
                ),
            ),
        ];
        if let Some(object) = self.interaction.hover().object.as_ref()
            && let Some(loc) = self
                .world
                .infos()
                .get(&object.region)
                .and_then(|info| info.locs.get(object.index))
        {
            lines.push((
                "Object".to_owned(),
                format!(
                    "{} ({})",
                    loc.definition.name.as_deref().unwrap_or("?"),
                    loc.definition.identity.id.get()
                ),
            ));
        }
        Some(lines)
    }

    // ---------------------------------------------------------------- actions
}
