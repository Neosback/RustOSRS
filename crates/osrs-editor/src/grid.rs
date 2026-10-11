//! Tile / zone / region grid overlay.
//!
//! Lines follow the terrain: every tile edge is one segment between its two corner heights, so the
//! grid drapes exactly over the ground. Tile lines fade out within a short radius, zone (8 tiles)
//! lines reach farther and region (64 tiles) borders farthest. The renderer draws the lines
//! depth-tested over the scene (see `SceneRenderer::set_overlay_lines`).

use crate::pick::Infos;
use osrs_render::gpu::LineVertex;

const TILE_RADIUS: i32 = 24;
const ZONE_RADIUS: i32 = 96;
const REGION_RADIUS: i32 = 384;
/// Lines sit this far above the terrain (render units; y is negative up).
const LIFT: f32 = 2.0;

/// Which grid levels are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GridSettings {
    pub tiles: bool,
    pub zones: bool,
    pub regions: bool,
}

impl Default for GridSettings {
    fn default() -> Self {
        Self {
            tiles: false,
            zones: false,
            regions: false,
        }
    }
}

impl GridSettings {
    pub fn any(self) -> bool {
        self.tiles || self.zones || self.regions
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Level {
    Tile,
    Zone,
    Region,
}

impl Level {
    fn radius(self) -> i32 {
        match self {
            Self::Tile => TILE_RADIUS,
            Self::Zone => ZONE_RADIUS,
            Self::Region => REGION_RADIUS,
        }
    }

    fn color(self) -> [f32; 3] {
        match self {
            Self::Tile => [1.0, 1.0, 1.0],
            Self::Zone => [0.3, 0.9, 1.0],
            Self::Region => [1.0, 0.6, 0.1],
        }
    }

    fn base_alpha(self) -> f32 {
        match self {
            Self::Tile => 0.3,
            Self::Zone => 0.6,
            Self::Region => 0.95,
        }
    }

    /// Level of a grid line at tile coordinate `coordinate` (a line along the other axis).
    fn of_line(coordinate: i32) -> Self {
        if coordinate.rem_euclid(64) == 0 {
            Self::Region
        } else if coordinate.rem_euclid(8) == 0 {
            Self::Zone
        } else {
            Self::Tile
        }
    }

    fn enabled(self, settings: GridSettings) -> bool {
        match self {
            Self::Tile => settings.tiles,
            Self::Zone => settings.zones,
            Self::Region => settings.regions,
        }
    }
}

/// Terrain height at tile corner `(x, y)` on `plane`.
fn corner_height(infos: &Infos, x: i32, y: i32, plane: usize) -> Option<f32> {
    let region = (x.div_euclid(64), y.div_euclid(64));
    infos.get(&region).map(|info| {
        info.corner_height(plane, x.rem_euclid(64) as usize, y.rem_euclid(64) as usize) as f32
    })
}

struct Builder<'a> {
    infos: &'a Infos,
    plane: usize,
    center: (i32, i32),
    origin: (i32, i32),
    lines: Vec<LineVertex>,
}

impl Builder<'_> {
    fn vertex(&self, x: i32, y: i32, height: f32, color: [f32; 4]) -> LineVertex {
        LineVertex {
            position: [
                (x * 128 - self.origin.0) as f32,
                height - LIFT,
                (y * 128 - self.origin.1) as f32,
            ],
            color,
        }
    }

    /// Segment between adjacent corners `a` and `b`.
    fn segment(&mut self, a: (i32, i32), b: (i32, i32), level: Level) {
        let mid = ((a.0 + b.0) as f32 * 0.5, (a.1 + b.1) as f32 * 0.5);
        let distance = (mid.0 - self.center.0 as f32)
            .abs()
            .max((mid.1 - self.center.1 as f32).abs());
        let radius = level.radius() as f32;
        if distance > radius {
            return;
        }
        let (Some(ha), Some(hb)) = (
            corner_height(self.infos, a.0, a.1, self.plane),
            corner_height(self.infos, b.0, b.1, self.plane),
        ) else {
            return;
        };
        let fade = (1.0 - distance / radius).clamp(0.0, 1.0).powf(0.7);
        let [r, g, bl] = level.color();
        let color = [r, g, bl, level.base_alpha() * fade];
        let va = self.vertex(a.0, a.1, ha, color);
        let vb = self.vertex(b.0, b.1, hb, color);
        self.lines.push(va);
        self.lines.push(vb);
    }
}

/// Build the line list around `center` (world tile) on `plane`. Positions are relative to the
/// renderer's `origin` (world units).
pub fn build(
    settings: GridSettings,
    infos: &Infos,
    center: (i32, i32),
    plane: usize,
    origin: (i32, i32),
) -> Vec<LineVertex> {
    let mut builder = Builder {
        infos,
        plane,
        center,
        origin,
        lines: Vec::new(),
    };
    if settings.tiles || settings.zones {
        let radius = if settings.zones {
            ZONE_RADIUS
        } else {
            TILE_RADIUS
        };
        for y in center.1 - radius..=center.1 + radius {
            for x in center.0 - radius..=center.0 + radius {
                // Segment along x on the line at row `y`, along y on the line at column `x`.
                let row = Level::of_line(y);
                if row != Level::Region && row.enabled(settings) {
                    builder.segment((x, y), (x + 1, y), row);
                }
                let column = Level::of_line(x);
                if column != Level::Region && column.enabled(settings) {
                    builder.segment((x, y), (x, y + 1), column);
                }
            }
        }
    }
    if settings.regions {
        let first = |c: i32| (c - REGION_RADIUS).div_euclid(64) * 64;
        for line_y in (first(center.1)..=center.1 + REGION_RADIUS).step_by(64) {
            for x in center.0 - REGION_RADIUS..=center.0 + REGION_RADIUS {
                builder.segment((x, line_y), (x + 1, line_y), Level::Region);
            }
        }
        for line_x in (first(center.0)..=center.0 + REGION_RADIUS).step_by(64) {
            for y in center.1 - REGION_RADIUS..=center.1 + REGION_RADIUS {
                builder.segment((line_x, y), (line_x, y + 1), Level::Region);
            }
        }
    }
    builder.lines
}
