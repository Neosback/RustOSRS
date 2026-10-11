//! The brush: which channels of a tile it paints, and what it does to the rest.
//!
//! A tile has an underlay, an overlay (with a shape and a rotation), four corner heights and a flag
//! byte. The brush never paints "a tile": every channel is armed or not. An unarmed channel is left
//! exactly as the tile already has it, so "keep the tile's values except the overlay I picked" is the
//! default, not a mode. This module is pure state and arithmetic (no egui, no GPU) so the same
//! rules can move into the edit crate unchanged.

use serde::{Deserialize, Serialize};

/// The editing tools, one active at a time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Tool {
    /// Click selects objects; no drawer.
    #[default]
    Select,
    /// Paint channels (overlay, underlay, shape) onto tiles.
    Paint,
    /// Sculpt tile heights.
    Height,
    /// Set tile flags (blocked, bridge, ...).
    Flags,
}

impl Tool {
    pub const ALL: [Self; 4] = [Self::Select, Self::Paint, Self::Height, Self::Flags];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Paint => "Paint",
            Self::Height => "Height",
            Self::Flags => "Flags",
        }
    }

    /// The keymap action that activates the tool.
    pub const fn action(self) -> &'static str {
        match self {
            Self::Select => "tool.select",
            Self::Paint => "tool.paint",
            Self::Height => "tool.height",
            Self::Flags => "tool.flags",
        }
    }

    /// Which drawer tab the tool starts on.
    pub const fn drawer_tab(self) -> Option<DrawerTab> {
        match self {
            Self::Select => None,
            Self::Paint => Some(DrawerTab::Overlay),
            Self::Height => Some(DrawerTab::Height),
            Self::Flags => Some(DrawerTab::Mask),
        }
    }

    /// Whether the tool has a palette, so the drawer is worth showing.
    pub const fn has_drawer(self) -> bool {
        !matches!(self, Self::Select)
    }
}

/// Channel tabs of the drawer.
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
    pub const ALL: [Self; 5] = [
        Self::Overlay,
        Self::Underlay,
        Self::Shape,
        Self::Height,
        Self::Mask,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Overlay => "Overlay",
            Self::Underlay => "Underlay",
            Self::Shape => "Shape",
            Self::Height => "Height",
            Self::Mask => "Mask",
        }
    }

    /// Tabs the tool shows: Paint has the colour channels, Height and Flags only their own.
    pub fn for_tool(tool: Tool) -> &'static [Self] {
        match tool {
            Tool::Select => &[],
            Tool::Paint => &[
                Self::Overlay,
                Self::Underlay,
                Self::Shape,
                Self::Height,
                Self::Mask,
            ],
            Tool::Height => &[Self::Height],
            Tool::Flags => &[Self::Mask],
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
    /// Height of the tile's reference corner, in client units.
    pub height: i32,
    pub flags: u8,
}

/// What an armed channel does with its value: set it, or clear the channel.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HeightMode {
    #[default]
    Set,
    Raise,
    Lower,
    Flatten,
    Smooth,
}

impl HeightMode {
    pub const ALL: [Self; 5] = [
        Self::Set,
        Self::Raise,
        Self::Lower,
        Self::Flatten,
        Self::Smooth,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Set => "Set",
            Self::Raise => "Raise",
            Self::Lower => "Lower",
            Self::Flatten => "Flatten",
            Self::Smooth => "Smooth",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeightChannel {
    pub armed: bool,
    pub mode: HeightMode,
    /// Client units (a tile is 128 wide; the client's height step is 8).
    pub amount: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FlagMode {
    #[default]
    Set,
    Add,
    Remove,
    Toggle,
}

impl FlagMode {
    pub const ALL: [Self; 4] = [Self::Set, Self::Add, Self::Remove, Self::Toggle];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Set => "Set",
            Self::Add => "Add",
            Self::Remove => "Remove",
            Self::Toggle => "Toggle",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlagChannel {
    pub armed: bool,
    pub mode: FlagMode,
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
                mode: HeightMode::Set,
                amount: 0,
            },
            flags: FlagChannel {
                armed: false,
                mode: FlagMode::Set,
                bits: 0,
            },
            radius: 0,
            footprint: BrushShape::Square,
        }
    }
}

impl Brush {
    /// The channels that will change a tile, by name (for the drawer header and the quick menu).
    pub fn armed_channels(&self) -> Vec<&'static str> {
        let mut channels = Vec::new();
        if self.overlay.armed {
            channels.push("Overlay");
        }
        if self.underlay.armed {
            channels.push("Underlay");
        }
        if self.shape.armed {
            channels.push("Shape");
        }
        if self.height.armed {
            channels.push("Height");
        }
        if self.flags.armed {
            channels.push("Mask");
        }
        channels
    }

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
        self.armed_channels().is_empty()
    }

    /// The tile after the brush: armed channels take the brush value, every other value stays.
    ///
    /// Clearing the overlay also resets its shape and rotation (the client ignores them without an
    /// overlay). Height modes other than `Set` need neighbours and are handled by the caller; here they
    /// move the reference corner only.
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
            out.height = match self.height.mode {
                HeightMode::Set | HeightMode::Flatten => self.height.amount,
                HeightMode::Raise => out.height - self.height.amount.abs(),
                HeightMode::Lower => out.height + self.height.amount.abs(),
                HeightMode::Smooth => out.height,
            };
        }
        if self.flags.armed {
            out.flags = match self.flags.mode {
                FlagMode::Set => self.flags.bits,
                FlagMode::Add => out.flags | self.flags.bits,
                FlagMode::Remove => out.flags & !self.flags.bits,
                FlagMode::Toggle => out.flags ^ self.flags.bits,
            };
        }
        out
    }

    /// Arm exactly the channels that describe `tile` and copy its values into them (the eyedropper).
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
        self.height.amount = tile.height;
        self.flags.bits = tile.flags;
        self.underlay.armed = true;
        self.overlay.armed = true;
        self.shape.armed = tile.overlay != 0;
    }

    /// Tiles covered by the brush around `center`, nearest first.
    pub fn footprint_tiles(&self, center: (i32, i32)) -> Vec<(i32, i32)> {
        let radius = i32::from(self.radius);
        let mut tiles = Vec::new();
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let inside = match self.footprint {
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
        let brush = Brush::default();
        let out = brush.apply(tile());
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
    fn flag_modes() {
        let mut brush = Brush::default();
        brush.overlay.armed = false;
        brush.flags = FlagChannel {
            armed: true,
            mode: FlagMode::Add,
            bits: 0x02,
        };
        assert_eq!(brush.apply(tile()).flags, 0x03);
        brush.flags.mode = FlagMode::Remove;
        brush.flags.bits = 0x01;
        assert_eq!(brush.apply(tile()).flags, 0);
        brush.flags.mode = FlagMode::Toggle;
        assert_eq!(brush.apply(tile()).flags, 0);
        brush.flags.mode = FlagMode::Set;
        brush.flags.bits = 0x08;
        assert_eq!(brush.apply(tile()).flags, 0x08);
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
}
