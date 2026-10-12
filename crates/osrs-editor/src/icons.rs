//! The editor's icon vocabulary: one place that maps each semantic action to an icon glyph
//! (Phosphor Regular, an MIT icon font). The same action uses the same icon everywhere; every
//! icon button must also carry a tooltip.

#![allow(
    dead_code,
    reason = "vocabulary: icons are added before the tools that use them"
)]

use egui_phosphor::regular as ph;

pub const FLY: &str = ph::AIRPLANE_TILT;
pub const ORBIT: &str = ph::ARROWS_CLOCKWISE;
pub const GRID: &str = ph::GRID_FOUR;
pub const TELEPORT: &str = ph::MAP_PIN;
pub const SIDE_PANEL: &str = ph::SIDEBAR_SIMPLE;
pub const STATS: &str = ph::GAUGE;
pub const WORLD_MAP: &str = ph::GLOBE_HEMISPHERE_WEST;
pub const COMPASS: &str = ph::COMPASS;
pub const WIKI: &str = ph::BOOK_OPEN;
pub const SELECT: &str = ph::CURSOR;
pub const PAINT: &str = ph::PAINT_BRUSH;
pub const LAYERS: &str = ph::STACK;
pub const OBJECT: &str = ph::CUBE;
pub const SEARCH: &str = ph::MAGNIFYING_GLASS;
pub const KEYBOARD: &str = ph::KEYBOARD;
pub const SETTINGS: &str = ph::GEAR;
pub const THEME: &str = ph::PALETTE;
pub const MOVE: &str = ph::ARROWS_OUT_CARDINAL;
pub const MEASURE: &str = ph::RULER;
pub const VISIBLE: &str = ph::EYE;
pub const STAIRS: &str = ph::STAIRS;
pub const MARQUEE: &str = ph::SELECTION;
pub const ZOOM_IN: &str = ph::MAGNIFYING_GLASS_PLUS;
pub const ZOOM_OUT: &str = ph::MAGNIFYING_GLASS_MINUS;
pub const CROSSHAIR: &str = ph::CROSSHAIR_SIMPLE;
pub const INSPECT: &str = ph::INFO;
pub const VIEW: &str = ph::EYE;
pub const GRAPHICS: &str = ph::SUN;
pub const PLUGINS: &str = ph::PUZZLE_PIECE;
pub const TOOL_SELECT: &str = ph::CURSOR;
pub const TOOL_PAINT: &str = ph::PAINT_BRUSH;
pub const TOOL_HEIGHT: &str = ph::MOUNTAINS;
pub const TOOL_SPLINE: &str = ph::BEZIER_CURVE;
pub const SELECT_OBJECTS: &str = ph::CUBE;
pub const SELECT_TILES: &str = ph::SQUARES_FOUR;
pub const SELECT_AUTO: &str = ph::CURSOR;
pub const EYEDROPPER: &str = ph::EYEDROPPER;
pub const DRAWER: &str = ph::CARET_DOWN;
pub const BRUSH_SQUARE: &str = ph::SQUARE;
pub const BRUSH_CIRCLE: &str = ph::CIRCLE;

// Painter channels (the tabs of the tile painter's drawer).
pub const CH_OVERLAY: &str = ph::SQUARE_HALF;
pub const CH_UNDERLAY: &str = ph::STACK;
pub const CH_SHAPE: &str = ph::SHAPES;
pub const CH_HEIGHT: &str = ph::MOUNTAINS;
pub const CH_MASK: &str = ph::FLAG;
pub const HIDE: &str = ph::EYE_SLASH;
pub const OVERLAYS: &str = ph::STACK_SIMPLE;
pub const TOOL_OBJECTS: &str = ph::CUBE_FOCUS;
pub const ROTATE: &str = ph::ARROW_CLOCKWISE;
pub const DELETE: &str = ph::TRASH;
pub const DUPLICATE: &str = ph::COPY;
