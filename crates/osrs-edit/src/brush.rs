//! Shared brush footprint arithmetic; no UI or renderer state.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BrushShape {
    #[default]
    Square,
    Circle,
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
