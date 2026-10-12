//! Tools the user adds in the app: saved brush presets, bookmarks, links and scripts, each a button on
//! the floating rail. They are plain data kept in the settings (nothing to compile), and everything
//! they do goes through the same commands the script language offers (`app/script.rs`).

use crate::paint::{Brush, SplineSettings, Tool};
use serde::{Deserialize, Serialize};

/// What a user tool does when its button is pressed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(
    clippy::large_enum_variant,
    reason = "Preset is cold persisted UI data; keeping brush and spline settings inline avoids needless indirection"
)]
pub enum UserToolKind {
    /// Switch to a tool with a saved brush and path settings ("dirt path, 3 wide").
    Preset {
        tool: Tool,
        brush: Brush,
        spline: SplineSettings,
    },
    /// Go to a place.
    Bookmark { x: i32, y: i32, plane: u8 },
    /// Open a link; `{x}`, `{y}`, `{plane}` and `{region}` are replaced with the camera's tile.
    Link { url: String },
    /// Run script commands, one per line.
    Script { text: String },
}

impl UserToolKind {
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Preset { .. } => "Brush preset",
            Self::Bookmark { .. } => "Bookmark",
            Self::Link { .. } => "Link",
            Self::Script { .. } => "Script",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserTool {
    pub name: String,
    /// Key into [`ICONS`].
    pub icon: String,
    pub kind: UserToolKind,
}

/// The icons a user tool can wear (key, glyph).
pub const ICONS: &[(&str, &str)] = &[
    ("star", egui_phosphor::regular::STAR),
    ("bookmark", egui_phosphor::regular::BOOKMARK_SIMPLE),
    ("pin", egui_phosphor::regular::MAP_PIN),
    ("flag", egui_phosphor::regular::FLAG),
    ("link", egui_phosphor::regular::LINK),
    ("bolt", egui_phosphor::regular::LIGHTNING),
    ("wrench", egui_phosphor::regular::WRENCH),
    ("code", egui_phosphor::regular::CODE),
    ("brush", egui_phosphor::regular::PAINT_BRUSH_BROAD),
    ("path", egui_phosphor::regular::BEZIER_CURVE),
    ("mountain", egui_phosphor::regular::MOUNTAINS),
    ("tree", egui_phosphor::regular::TREE),
    ("house", egui_phosphor::regular::HOUSE),
    ("castle", egui_phosphor::regular::CASTLE_TURRET),
    ("water", egui_phosphor::regular::WAVES),
    ("fire", egui_phosphor::regular::FIRE),
    ("skull", egui_phosphor::regular::SKULL),
    ("gem", egui_phosphor::regular::DIAMOND),
    ("cube", egui_phosphor::regular::CUBE),
    ("grid", egui_phosphor::regular::GRID_FOUR),
    ("eye", egui_phosphor::regular::EYE),
    ("sun", egui_phosphor::regular::SUN),
    ("moon", egui_phosphor::regular::MOON),
    ("heart", egui_phosphor::regular::HEART),
];

/// The glyph for an icon key (a star for an unknown one).
pub fn glyph(key: &str) -> &'static str {
    ICONS
        .iter()
        .find(|(name, _)| *name == key)
        .map_or(egui_phosphor::regular::STAR, |(_, glyph)| *glyph)
}

/// A tool being made or edited in the Add tool window.
#[derive(Debug, Clone)]
pub struct ToolDraft {
    /// The tool's position when editing one, `None` for a new tool.
    pub index: Option<usize>,
    pub tool: UserTool,
}

impl ToolDraft {
    pub fn new_script() -> Self {
        Self {
            index: None,
            tool: UserTool {
                name: "My tool".to_owned(),
                icon: "star".to_owned(),
                kind: UserToolKind::Script {
                    text: "tool paint\nradius 2".to_owned(),
                },
            },
        }
    }
}

/// Replace `{x}`, `{y}`, `{plane}` and `{region}` in a link template.
pub fn fill_link(url: &str, tile: (i32, i32), plane: u8) -> String {
    url.replace("{x}", &tile.0.to_string())
        .replace("{y}", &tile.1.to_string())
        .replace("{plane}", &plane.to_string())
        .replace(
            "{region}",
            &crate::hud::region_id(tile.0 >> 6, tile.1 >> 6).to_string(),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_templates_fill_in_the_camera_tile() {
        assert_eq!(
            fill_link(
                "https://x/?a={x}&b={y}&p={plane}&r={region}",
                (3222, 3218),
                1
            ),
            "https://x/?a=3222&b=3218&p=1&r=12850"
        );
    }

    #[test]
    fn unknown_icon_keys_fall_back_to_a_star() {
        assert_eq!(glyph("nope"), glyph("star"));
        assert_ne!(glyph("tree"), glyph("star"));
    }
}
