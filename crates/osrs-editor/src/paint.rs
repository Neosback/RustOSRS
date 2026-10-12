//! Tools and the brush: which channels of a tile the brush paints, and what it leaves alone.
//!
//! A tile has an underlay, an overlay (with a shape and a rotation), a height and a flag byte. The
//! brush never paints "a tile": every channel is armed or not. An unarmed channel keeps exactly what the
//! tile already has, so "the tile as it is, except the overlay I picked" is the default, not a mode.
//! This module is pure state and arithmetic (no egui, no GPU) so the same rules can move into the
//! edit crate unchanged.

use serde::{Deserialize, Serialize};

/// The tools. The editor rests in `Select`; the others are picked from the floating toolbar, the
/// quick HUD, the context menu or a key, and `Esc` returns to `Select`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Tool {
    /// Click and box-select objects and tiles.
    #[default]
    Select,
    /// Paint channels (overlay, underlay, shape, height, flags) onto tiles.
    Paint,
    /// Lay a path along a line and let the path planner pick every tile's shape.
    #[serde(alias = "Flags")]
    Spline,
    /// Sculpt tile heights; paints nothing else.
    Height,
    /// Move, turn, copy and delete the selected objects.
    Objects,
}

impl Tool {
    pub const ALL: [Self; 5] = [
        Self::Select,
        Self::Paint,
        Self::Spline,
        Self::Height,
        Self::Objects,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Paint => "Paint",
            Self::Spline => "Spline",
            Self::Height => "Height",
            Self::Objects => "Objects",
        }
    }

    /// The keymap action that activates the tool.
    pub const fn action(self) -> &'static str {
        match self {
            Self::Select => "tool.select",
            Self::Paint => "tool.paint",
            Self::Spline => "tool.spline",
            Self::Height => "tool.height",
            Self::Objects => "tool.objects",
        }
    }

    /// The tabs the tool's drawer shows (none: no drawer).
    pub fn tabs(self) -> &'static [DrawerTab] {
        match self {
            Self::Select => &[],
            Self::Paint => &[
                DrawerTab::Overlay,
                DrawerTab::Underlay,
                DrawerTab::Shape,
                DrawerTab::Height,
                DrawerTab::Mask,
            ],
            Self::Spline | Self::Height | Self::Objects => &[],
        }
    }

    /// Every authoring tool has a drawer (Select has none).
    pub const fn has_drawer(self) -> bool {
        !matches!(self, Self::Select)
    }

    /// Whether the left shelf shows: the tools with options to tune (Objects only has actions, in
    /// its drawer).
    pub const fn has_shelf(self) -> bool {
        !matches!(self, Self::Select | Self::Objects)
    }

    /// Whether the tool paints with the brush footprint.
    pub const fn uses_brush(self) -> bool {
        matches!(self, Self::Paint | Self::Height)
    }
}

/// Tabs of the drawer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DrawerTab {
    #[default]
    Overlay,
    Underlay,
    Shape,
    Height,
    Mask,
}

impl DrawerTab {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Overlay => "Overlay",
            Self::Underlay => "Underlay",
            Self::Shape => "Shape",
            Self::Height => "Height",
            Self::Mask => "Mask",
        }
    }
}

/// What a click or drag in the viewport selects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SelectMode {
    /// A click takes the object under the cursor, else the tile; a box takes both.
    #[default]
    #[serde(alias = "Both")]
    Auto,
    Objects,
    Tiles,
}

impl SelectMode {
    pub const ALL: [Self; 3] = [Self::Auto, Self::Objects, Self::Tiles];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Objects => "Objects",
            Self::Tiles => "Tiles",
        }
    }

    pub const fn hint(self) -> &'static str {
        match self {
            Self::Auto => {
                "Click takes the object under the cursor, else the tile; a box takes both"
            }
            Self::Objects => "Only objects: click one, or drag a box",
            Self::Tiles => "Only tiles: click one, or drag a box",
        }
    }

    pub const fn objects(self) -> bool {
        matches!(self, Self::Auto | Self::Objects)
    }

    pub const fn tiles(self) -> bool {
        matches!(self, Self::Auto | Self::Tiles)
    }
}

/// What the path tool lays along the line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PathMode {
    /// A natural path of overlay tiles (the terrain path planner).
    #[default]
    Terrain,
    /// A castle wall: a band of tiles with wall objects on its edges.
    Walls,
}

impl PathMode {
    pub const ALL: [Self; 2] = [Self::Terrain, Self::Walls];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Terrain => "Path",
            Self::Walls => "Walls",
        }
    }
}

/// How the path tool lays a path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SplineSettings {
    pub mode: PathMode,
    /// The planner's settings (smoothness, obstacles, noise, seed ...).
    pub planner: crate::terrain_path::PathSettings,
    /// What the path is made of (width, falloff, wander ...). Its overlay is the brush's overlay.
    pub profile: crate::terrain_path::PathProfile,
    /// Control points land on tile centres (off: where you click).
    pub snap_points: bool,
    /// Walls mode: castle-wall kit object ids (0 = unset; top falls back to side).
    pub wall_side: u32,
    pub wall_top: u32,
    pub wall_door: u32,
    pub wall_diagonal: u32,
    /// Walls mode: band thickness in tiles.
    pub wall_thickness: f32,
    /// Walls mode: wall height in planes (plus a cap row on top).
    pub wall_height: f32,
    /// Walls mode: lay tower squares around each control point.
    pub wall_towers: bool,
    /// Walls mode: tower square size in tiles.
    pub tower_size: f32,
}

impl Default for SplineSettings {
    fn default() -> Self {
        Self {
            mode: PathMode::Terrain,
            planner: crate::terrain_path::PathSettings::default(),
            profile: crate::terrain_path::PathProfile::default(),
            snap_points: false,
            wall_side: 0,
            wall_top: 0,
            wall_door: 0,
            wall_diagonal: 0,
            wall_thickness: 3.0,
            wall_height: 2.0,
            wall_towers: true,
            tower_size: 2.0,
        }
    }
}

/// Which floors the palettes list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PaletteFilter {
    /// Floors that many tiles of the real map use.
    #[default]
    Common,
    /// Every floor any tile of the map uses.
    AllUsed,
    /// Overlays with a texture.
    Textured,
}

impl PaletteFilter {
    pub const ALL: [Self; 3] = [Self::Common, Self::AllUsed, Self::Textured];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Common => "Common",
            Self::AllUsed => "All used",
            Self::Textured => "Textured",
        }
    }

    /// Tiles of the whole map a floor needs to count as common.
    pub const COMMON_USES: u64 = 5_000;

    pub fn keeps(self, uses: u64, textured: bool) -> bool {
        match self {
            Self::Common => uses >= Self::COMMON_USES,
            Self::AllUsed => uses > 0,
            Self::Textured => textured,
        }
    }
}

/// Highest overlay shape id (0 = full tile).
pub const MAX_SHAPE: u8 = 11;

/// The values of one tile that the brush can touch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TileValues {
    /// Underlay id as stored in the map file (`0` = none).
    pub underlay: u16,
    /// Overlay id as stored in the map file (`0` = none).
    pub overlay: u16,
    pub shape: u8,
    pub rotation: u8,
    /// Height of the tile's reference corner in client units (up is negative, a multiple of 8).
    pub height: i32,
    pub flags: u8,
}

/// What an armed id channel does: set the id, or clear the channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Fill {
    Set(u16),
    Clear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdChannel {
    pub armed: bool,
    pub fill: Fill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShapeChannel {
    pub armed: bool,
    pub shape: u8,
    pub rotation: u8,
}

/// The height the brush gives every tile it paints (no modes: the Height tool does the sculpting).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeightChannel {
    pub armed: bool,
    /// Client units, up is negative.
    pub value: i32,
}

/// The flags the brush gives every tile it paints: a set, not an edit of the tile's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlagChannel {
    pub armed: bool,
    pub bits: u8,
}

/// The tile flag bits the client reads (`tileSettings`).
pub const FLAG_BITS: [(u8, &str, &str); 4] = [
    (0x01, "Blocked", "Tile cannot be walked on"),
    (0x02, "Bridge", "Plane 1 content is drawn one plane lower"),
    (
        0x04,
        "Hide roofs",
        "Roofs above are hidden when the player is under it",
    ),
    (0x08, "Lowest plane", "Draw this tile on plane 0"),
];

/// Highest elevation the map can store (a byte of 8-unit steps).
pub const MAX_ELEVATION: i32 = 255 * 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BrushShape {
    #[default]
    Square,
    Circle,
}

/// Everything the brush is set to, persisted with the settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Brush {
    pub overlay: IdChannel,
    pub underlay: IdChannel,
    pub shape: ShapeChannel,
    pub height: HeightChannel,
    pub flags: FlagChannel,
    /// Footprint radius in tiles (`0` = one tile).
    pub radius: u8,
    pub footprint: BrushShape,
}

impl Default for Brush {
    fn default() -> Self {
        Self {
            overlay: IdChannel {
                armed: true,
                fill: Fill::Set(1),
            },
            underlay: IdChannel {
                armed: false,
                fill: Fill::Set(1),
            },
            shape: ShapeChannel {
                armed: false,
                shape: 0,
                rotation: 0,
            },
            height: HeightChannel {
                armed: false,
                value: 0,
            },
            flags: FlagChannel {
                armed: false,
                bits: 0,
            },
            radius: 0,
            footprint: BrushShape::Square,
        }
    }
}

impl Brush {
    /// Arm or disarm one channel.
    pub fn set_armed(&mut self, tab: DrawerTab, armed: bool) {
        match tab {
            DrawerTab::Overlay => self.overlay.armed = armed,
            DrawerTab::Underlay => self.underlay.armed = armed,
            DrawerTab::Shape => self.shape.armed = armed,
            DrawerTab::Height => self.height.armed = armed,
            DrawerTab::Mask => self.flags.armed = armed,
        }
    }

    pub fn is_armed(&self, tab: DrawerTab) -> bool {
        match tab {
            DrawerTab::Overlay => self.overlay.armed,
            DrawerTab::Underlay => self.underlay.armed,
            DrawerTab::Shape => self.shape.armed,
            DrawerTab::Height => self.height.armed,
            DrawerTab::Mask => self.flags.armed,
        }
    }

    /// `true` when the brush would change nothing.
    pub fn is_empty(&self) -> bool {
        !(self.overlay.armed
            || self.underlay.armed
            || self.shape.armed
            || self.height.armed
            || self.flags.armed)
    }

    /// The tile after the brush: armed channels take the brush value, every other value stays.
    ///
    /// Clearing the overlay also resets its shape and rotation (the client ignores them without an
    /// overlay).
    pub fn apply(&self, tile: TileValues) -> TileValues {
        let mut out = tile;
        if self.underlay.armed {
            out.underlay = match self.underlay.fill {
                Fill::Set(id) => id,
                Fill::Clear => 0,
            };
        }
        if self.overlay.armed {
            match self.overlay.fill {
                Fill::Set(id) => out.overlay = id,
                Fill::Clear => {
                    out.overlay = 0;
                    out.shape = 0;
                    out.rotation = 0;
                }
            }
        }
        if self.shape.armed && out.overlay != 0 {
            out.shape = self.shape.shape.min(MAX_SHAPE);
            out.rotation = self.shape.rotation & 3;
        }
        if self.height.armed {
            out.height = self.height.value;
        }
        if self.flags.armed {
            out.flags = self.flags.bits;
        }
        out
    }

    /// Copy a tile's values into the brush and arm what the tile has (the eyedropper).
    pub fn sample(&mut self, tile: TileValues) {
        self.underlay.fill = if tile.underlay == 0 {
            Fill::Clear
        } else {
            Fill::Set(tile.underlay)
        };
        self.overlay.fill = if tile.overlay == 0 {
            Fill::Clear
        } else {
            Fill::Set(tile.overlay)
        };
        self.shape.shape = tile.shape.min(MAX_SHAPE);
        self.shape.rotation = tile.rotation & 3;
        self.height.value = tile.height;
        self.flags.bits = tile.flags;
        self.underlay.armed = true;
        self.overlay.armed = true;
        self.shape.armed = tile.overlay != 0;
    }

    /// Tiles covered by the brush around `center`, nearest first.
    pub fn footprint_tiles(&self, center: (i32, i32)) -> Vec<(i32, i32)> {
        footprint(self.radius, self.footprint, center)
    }
}

/// The tiles of a square or round footprint, nearest to `center` first.
pub fn footprint(radius: u8, shape: BrushShape, center: (i32, i32)) -> Vec<(i32, i32)> {
    let radius = i32::from(radius);
    let mut tiles = Vec::new();
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let inside = match shape {
                BrushShape::Square => true,
                BrushShape::Circle => dx * dx + dy * dy <= radius * radius + radius / 2,
            };
            if inside {
                tiles.push((center.0 + dx, center.1 + dy));
            }
        }
    }
    tiles.sort_by_key(|(x, y)| (x - center.0).pow(2) + (y - center.1).pow(2));
    tiles
}

/// Client height (up is negative) to the elevation people think in (up is positive).
pub const fn elevation(height: i32) -> i32 {
    -height
}

/// The nearest storable client height for an elevation: a multiple of 8, between sea level and the
/// highest the map can hold.
pub const fn height_for_elevation(elevation: i32) -> i32 {
    let clamped = if elevation < 0 {
        0
    } else if elevation > MAX_ELEVATION {
        MAX_ELEVATION
    } else {
        elevation
    };
    -((clamped + 4) / 8) * 8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tile() -> TileValues {
        TileValues {
            underlay: 49,
            overlay: 111,
            shape: 4,
            rotation: 2,
            height: -40,
            flags: 0x01,
        }
    }

    #[test]
    fn unarmed_channels_keep_the_tile_exactly() {
        let brush = Brush {
            overlay: IdChannel {
                armed: false,
                fill: Fill::Set(7),
            },
            ..Brush::default()
        };
        assert!(brush.is_empty());
        assert_eq!(brush.apply(tile()), tile());
    }

    #[test]
    fn an_armed_overlay_changes_only_the_overlay() {
        let out = Brush::default().apply(tile());
        assert_eq!(out.overlay, 1);
        assert_eq!((out.underlay, out.shape, out.rotation), (49, 4, 2));
        assert_eq!((out.height, out.flags), (-40, 0x01));
    }

    #[test]
    fn clearing_the_overlay_resets_shape_and_rotation() {
        let mut brush = Brush::default();
        brush.overlay.fill = Fill::Clear;
        let out = brush.apply(tile());
        assert_eq!((out.overlay, out.shape, out.rotation), (0, 0, 0));
        assert_eq!(out.underlay, 49);
    }

    #[test]
    fn shape_needs_an_overlay_to_apply_to() {
        let mut brush = Brush::default();
        brush.overlay.armed = false;
        brush.shape = ShapeChannel {
            armed: true,
            shape: 9,
            rotation: 3,
        };
        let on_overlay = brush.apply(tile());
        assert_eq!((on_overlay.shape, on_overlay.rotation), (9, 3));
        let bare = TileValues {
            overlay: 0,
            ..tile()
        };
        assert_eq!(brush.apply(bare), bare);
    }

    #[test]
    fn height_and_flags_are_plain_values_when_armed() {
        let mut brush = Brush::default();
        brush.overlay.armed = false;
        brush.height = HeightChannel {
            armed: true,
            value: -96,
        };
        brush.flags = FlagChannel {
            armed: true,
            bits: 0x0a,
        };
        let out = brush.apply(tile());
        assert_eq!((out.height, out.flags), (-96, 0x0a));
        assert_eq!(out.overlay, 111);
    }

    #[test]
    fn eyedropper_arms_and_copies_the_tile() {
        let mut brush = Brush::default();
        brush.sample(tile());
        assert_eq!(brush.overlay.fill, Fill::Set(111));
        assert_eq!(brush.underlay.fill, Fill::Set(49));
        assert!(brush.shape.armed);
        assert_eq!(brush.apply(TileValues::default()).overlay, 111);
    }

    #[test]
    fn footprint_is_centered_and_sorted() {
        let brush = Brush {
            radius: 1,
            ..Brush::default()
        };
        let tiles = brush.footprint_tiles((10, 10));
        assert_eq!(tiles.len(), 9);
        assert_eq!(tiles[0], (10, 10));
        let circle = Brush {
            radius: 3,
            footprint: BrushShape::Circle,
            ..Brush::default()
        };
        assert!(circle.footprint_tiles((0, 0)).len() < 49);
    }

    #[test]
    fn elevations_round_to_storable_heights() {
        assert_eq!(height_for_elevation(0), 0);
        assert_eq!(height_for_elevation(11), -8);
        assert_eq!(height_for_elevation(12), -16);
        assert_eq!(height_for_elevation(-50), 0);
        assert_eq!(height_for_elevation(99_999), -MAX_ELEVATION);
        assert_eq!(elevation(-96), 96);
    }

    #[test]
    fn the_retired_flags_tool_loads_as_the_spline_tool() -> Result<(), serde_json::Error> {
        let tool: Tool = serde_json::from_str("\"Flags\"")?;
        assert_eq!(tool, Tool::Spline);
        Ok(())
    }
}
