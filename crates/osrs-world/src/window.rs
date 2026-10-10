//! The fixed 104x104 scene window the reference client builds around the camera.

use osrs_core::coords::RegionCoord;
use osrs_scene::terrain_load::REFERENCE_SCENE_TILES;

/// A 104x104-tile scene whose tile `(0, 0)` is world tile `(base_x, base_y)`.
///
/// The client builds scenes on 8-tile chunk boundaries (13x13 chunks). The builder only emits
/// terrain for the interior `1..103` tiles on each axis, so editors that tile a larger world must
/// overlap windows by at least that border.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SceneWindow {
    pub base_x: i32,
    pub base_y: i32,
}

impl SceneWindow {
    /// Window whose origin is the given world tile; must be a multiple of 8 on both axes.
    pub const fn new(base_x: i32, base_y: i32) -> Option<Self> {
        if base_x.rem_euclid(8) == 0 && base_y.rem_euclid(8) == 0 {
            Some(Self { base_x, base_y })
        } else {
            None
        }
    }

    /// Window centered on a chunk, as the client does (`chunk * 8 - 48`).
    pub const fn around_chunk(chunk_x: i32, chunk_y: i32) -> Self {
        Self {
            base_x: chunk_x * 8 - 48,
            base_y: chunk_y * 8 - 48,
        }
    }

    /// Every map region the window overlaps, in client iteration order (x-major).
    pub fn regions(self) -> Vec<RegionCoord> {
        let last = REFERENCE_SCENE_TILES as i32 - 1;
        let min_x = self.base_x.div_euclid(64);
        let max_x = (self.base_x + last).div_euclid(64);
        let min_y = self.base_y.div_euclid(64);
        let max_y = (self.base_y + last).div_euclid(64);
        let mut regions = Vec::new();
        for x in min_x..=max_x {
            for y in min_y..=max_y {
                regions.push(RegionCoord::new(x, y));
            }
        }
        regions
    }

    /// Scene tile of a region's local `(0, 0)`.
    pub const fn region_scene_origin(self, region: RegionCoord) -> (i32, i32) {
        (region.x * 64 - self.base_x, region.y * 64 - self.base_y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lumbridge_window_overlaps_nine_regions() -> Result<(), &'static str> {
        let window = SceneWindow::new(3176, 3176).ok_or("aligned")?;
        let regions = window.regions();
        assert_eq!(regions.len(), 9);
        assert_eq!(
            window.region_scene_origin(RegionCoord::new(50, 50)),
            (24, 24)
        );
        assert_eq!(
            window.region_scene_origin(RegionCoord::new(49, 49)),
            (-40, -40)
        );
    }

    #[test]
    fn chunk_window_matches_client_formula() {
        assert_eq!(
            SceneWindow::around_chunk(400, 400),
            SceneWindow {
                base_x: 3152,
                base_y: 3152
            }
        );
        assert!(SceneWindow::new(3177, 3176).is_none());
    }
}
