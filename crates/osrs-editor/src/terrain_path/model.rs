//! The path data model: points, a network of nodes and edges, the paints a plan produces, the
//! environment a plan reads (bounds, obstacles, tiles that already carry the overlay) and warnings.

use super::settings::PathProfile;
use std::sync::Arc;

/// A point in tile coordinates (`x` east, `y` north; a tile `(i, j)` spans `i..i + 1`, `j..j + 1`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathPoint {
    pub x: f64,
    pub y: f64,
}

impl PathPoint {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn distance_to(self, other: Self) -> f64 {
        (self.x - other.x).hypot(self.y - other.y)
    }
}

/// An end or junction of a path.
#[derive(Debug, Clone, PartialEq)]
pub struct PathNode {
    pub id: u64,
    pub x: f64,
    pub y: f64,
    pub plane: u8,
    pub width_scale: f64,
}

impl PathNode {
    pub fn new(id: u64, x: f64, y: f64, plane: u8) -> Self {
        Self {
            id,
            x,
            y,
            plane: plane.min(3),
            width_scale: 1.0,
        }
    }

    pub fn point(&self) -> PathPoint {
        PathPoint::new(self.x, self.y)
    }
}

/// A path between two nodes, through control points.
#[derive(Debug, Clone, PartialEq)]
pub struct PathEdge {
    pub id: u64,
    pub start: u64,
    pub end: u64,
    pub control_points: Vec<PathPoint>,
    pub profile_id: String,
    pub width_scale: f64,
    /// Whether the planner may route it around obstacles.
    pub auto_route: bool,
}

impl PathEdge {
    pub fn new(id: u64, start: u64, end: u64, control_points: Vec<PathPoint>, profile_id: &str) -> Self {
        Self {
            id,
            start,
            end,
            control_points,
            profile_id: profile_id.to_owned(),
            width_scale: 1.0,
            auto_route: true,
        }
    }
}

/// Nodes, the edges between them, and the profiles the edges use.
#[derive(Debug, Clone)]
pub struct PathNetwork {
    pub id: u64,
    pub name: String,
    pub seed: i64,
    pub nodes: Vec<PathNode>,
    pub edges: Vec<PathEdge>,
    pub profiles: Vec<PathProfile>,
}

/// One tile a plan paints: an overlay in a shape and orientation, and how much of the tile the
/// path covered.
#[derive(Debug, Clone, PartialEq)]
pub struct TilePaint {
    pub tile_x: i32,
    pub tile_y: i32,
    pub plane: u8,
    pub overlay_id: u16,
    pub shape: u8,
    pub orientation: u8,
    pub coverage: f64,
    pub profile_id: String,
}

impl TilePaint {
    pub fn new(tile_x: i32, tile_y: i32, plane: u8, overlay_id: u16, shape: u8, orientation: u8) -> Self {
        Self {
            tile_x,
            tile_y,
            plane,
            overlay_id,
            shape,
            orientation,
            coverage: 1.0,
            profile_id: "default".to_owned(),
        }
    }
}

/// A rectangle of tiles, `min` inclusive and `max` exclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileBounds {
    pub min_x: i32,
    pub min_y: i32,
    pub max_x: i32,
    pub max_y: i32,
}

impl TileBounds {
    pub const fn empty() -> Self {
        Self {
            min_x: 0,
            min_y: 0,
            max_x: 0,
            max_y: 0,
        }
    }

    pub fn new(min_x: i32, min_y: i32, max_x: i32, max_y: i32) -> Self {
        Self {
            min_x,
            min_y,
            max_x: max_x.max(min_x),
            max_y: max_y.max(min_y),
        }
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.min_x && x < self.max_x && y >= self.min_y && y < self.max_y
    }

    pub fn is_empty(&self) -> bool {
        self.min_x == self.max_x || self.min_y == self.max_y
    }

    pub fn intersect(&self, other: &Self) -> Self {
        let min_x = self.min_x.max(other.min_x);
        let min_y = self.min_y.max(other.min_y);
        let max_x = self.max_x.min(other.max_x);
        let max_y = self.max_y.min(other.max_y);
        if max_x > min_x && max_y > min_y {
            Self {
                min_x,
                min_y,
                max_x,
                max_y,
            }
        } else {
            Self::empty()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

/// Something the planner wants the author to know.
#[derive(Debug, Clone, PartialEq)]
pub struct PathWarning {
    pub severity: Severity,
    pub code: &'static str,
    pub message: String,
    pub tile: Option<(i32, i32, u8)>,
}

impl PathWarning {
    pub fn general(severity: Severity, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            severity,
            code,
            message: message.into(),
            tile: None,
        }
    }

    pub fn at(
        severity: Severity,
        code: &'static str,
        message: impl Into<String>,
        tile: (i32, i32, u8),
    ) -> Self {
        Self {
            severity,
            code,
            message: message.into(),
            tile: Some(tile),
        }
    }
}

/// What a plan makes: the paints, in plane, x, y order, and what went wrong or was worked around.
#[derive(Debug, Clone)]
pub struct PathPlan {
    pub paints: Vec<TilePaint>,
    pub warnings: Vec<PathWarning>,
    pub affected: TileBounds,
    /// The centre line of each edge as it was stamped (after routing and wander), for drawing.
    pub lines: Vec<Vec<PathPoint>>,
}

impl PathPlan {
    pub fn empty() -> Self {
        Self {
            paints: Vec::new(),
            warnings: Vec::new(),
            affected: TileBounds::empty(),
            lines: Vec::new(),
        }
    }

    #[allow(dead_code, reason = "part of the plan API")]
    pub fn has_errors(&self) -> bool {
        self.warnings.iter().any(|w| w.severity == Severity::Error)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Occupancy {
    Clear,
    Decoration,
    Solid,
}

/// What the planner knows of the ground: what stands on a tile, how steep it is, and which steps
/// between tiles a wall stops.
pub trait CostGrid: Send + Sync {
    fn occupancy(&self, _x: i32, _y: i32, _plane: u8) -> Occupancy {
        Occupancy::Clear
    }

    fn traversal_cost(&self, _x: i32, _y: i32, _plane: u8) -> f64 {
        1.0
    }

    fn slope(&self, _x: i32, _y: i32, _plane: u8) -> f64 {
        0.0
    }

    fn edge_blocked(&self, _from_x: i32, _from_y: i32, _to_x: i32, _to_y: i32, _plane: u8) -> bool {
        false
    }
}

/// A grid with nothing on it.
pub struct OpenGround;

impl CostGrid for OpenGround {}

/// The ground a plan is made on.
#[derive(Clone)]
pub struct PathEnvironment {
    pub bounds: TileBounds,
    pub cost_grid: Arc<dyn CostGrid>,
    /// Tiles that already carry an overlay (for keeping what is there and joining onto it).
    pub existing_paints: Vec<TilePaint>,
}

impl PathEnvironment {
    pub fn empty(bounds: TileBounds) -> Self {
        Self {
            bounds,
            cost_grid: Arc::new(OpenGround),
            existing_paints: Vec::new(),
        }
    }
}
