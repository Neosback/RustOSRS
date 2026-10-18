//! Empirical catalog of OSRS overlay path shapes (`0..=11`), companion underlays (the underlay
//! stored directly beneath a shaped border tile), and bare road-shoulder underlays (cardinal
//! neighbor tiles with no overlay bordering a shaped path tile).

use crate::terrain_load::{REGION_TILES, RegionTerrain, SCENE_PLANES};
use std::collections::BTreeMap;

/// Highest overlay shape index in OSRS (`0` = flat full tile, `1..=11` = shaped border).
pub const SHAPE_COUNT: usize = 12;

/// Empirical usage profile of one floor overlay across scanned map regions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayPathProfile {
    pub overlay_id: u16,
    pub total_tiles: u64,
    pub flat_tiles: u64,
    pub shaped_tiles: u64,
    pub shape_histogram: [u64; SHAPE_COUNT],
    /// `(underlay_id -> count)` on the same tile when `shape in 1..=11`.
    pub companion_underlays: BTreeMap<u16, u64>,
    /// `(underlay_id -> count)` on cardinal neighbor tiles with no overlay bordering a shaped tile.
    pub shoulder_underlays: BTreeMap<u16, u64>,
}

impl OverlayPathProfile {
    pub fn new(overlay_id: u16) -> Self {
        Self {
            overlay_id,
            total_tiles: 0,
            flat_tiles: 0,
            shaped_tiles: 0,
            shape_histogram: [0; SHAPE_COUNT],
            companion_underlays: BTreeMap::new(),
            shoulder_underlays: BTreeMap::new(),
        }
    }

    /// Top `limit` `(underlay_id, count)` pairs directly under shaped tiles of this overlay.
    pub fn top_companions(&self, limit: usize) -> Vec<(u16, u64)> {
        top_ranked(&self.companion_underlays, limit)
    }

    /// Top `limit` `(underlay_id, count)` pairs on bare cardinal road-shoulder neighbors.
    pub fn top_shoulders(&self, limit: usize) -> Vec<(u16, u64)> {
        top_ranked(&self.shoulder_underlays, limit)
    }
}

fn top_ranked(counts: &BTreeMap<u16, u64>, limit: usize) -> Vec<(u16, u64)> {
    let mut pairs: Vec<(u16, u64)> = counts
        .iter()
        .filter(|&(&id, &count)| id != 0 && count > 0)
        .map(|(&id, &count)| (id, count))
        .collect();
    pairs.sort_by_key(|&(id, count)| (std::cmp::Reverse(count), id));
    pairs.truncate(limit);
    pairs
}

/// Aggregated path and border catalog across regions.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PathCatalog {
    pub overlays: BTreeMap<u16, OverlayPathProfile>,
}

impl PathCatalog {
    /// Observe one 2D plane slice of `width x height` tiles where `tile_at(x, y)` returns
    /// `(Option<(overlay_id, shape)>, underlay_id)`.
    pub fn observe_plane(
        &mut self,
        width: usize,
        height: usize,
        tile_at: impl Fn(usize, usize) -> (Option<(u16, u8)>, u16),
    ) {
        for x in 0..width {
            for y in 0..height {
                let (Some((raw_ov, shape)), underlay) = tile_at(x, y) else {
                    continue;
                };
                let ov_id = raw_ov & 0x7fff;
                if ov_id == 0 {
                    continue;
                }
                let profile = self
                    .overlays
                    .entry(ov_id)
                    .or_insert_with(|| OverlayPathProfile::new(ov_id));
                profile.total_tiles += 1;
                let shape_idx = usize::from(shape);
                if shape_idx < SHAPE_COUNT {
                    profile.shape_histogram[shape_idx] += 1;
                }
                if shape == 0 {
                    profile.flat_tiles += 1;
                } else if (1..=11).contains(&shape) {
                    profile.shaped_tiles += 1;
                    if underlay != 0 {
                        *profile.companion_underlays.entry(underlay).or_default() += 1;
                    }
                    for (nx, ny) in [
                        (x.wrapping_sub(1), y),
                        (x + 1, y),
                        (x, y.wrapping_sub(1)),
                        (x, y + 1),
                    ] {
                        if nx >= width || ny >= height {
                            continue;
                        }
                        let (neighbor_ov, neighbor_ul) = tile_at(nx, ny);
                        if neighbor_ov.is_none() && neighbor_ul != 0 {
                            *profile.shoulder_underlays.entry(neighbor_ul).or_default() += 1;
                        }
                    }
                }
            }
        }
    }

    /// Observe all 4 planes of a decoded [`RegionTerrain`].
    pub fn observe_region(&mut self, terrain: &RegionTerrain) {
        for plane in 0..SCENE_PLANES {
            self.observe_plane(REGION_TILES, REGION_TILES, |x, y| {
                let tile = terrain.tile(plane, x, y);
                (tile.overlay.map(|ov| (ov.raw_id, ov.shape)), tile.underlay)
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_catalog_tallies_shape_histogram_companions_and_bare_shoulders() {
        // 3x3 grid:
        // y=2: bare(ul=5)   edge(ov=12,sh=1,ul=9)  bare(ul=5)
        // y=1: bare(ul=6)   center(ov=12,sh=0,ul=9) edge(ov=12,sh=2,ul=8)
        // y=0: bare(ul=6)   bare(ul=6)             bare(ul=7)
        let mut catalog = PathCatalog::default();
        catalog.observe_plane(3, 3, |x, y| match (x, y) {
            (1, 2) => (Some((12, 1)), 9),
            (1, 1) => (Some((12, 0)), 9),
            (2, 1) => (Some((12, 2)), 8),
            (0, 2) | (2, 2) => (None, 5),
            (0, 1) | (0, 0) | (1, 0) => (None, 6),
            (2, 0) => (None, 7),
            _ => (None, 0),
        });

        let profile = catalog.overlays.get(&12).expect("overlay 12 profile");
        assert_eq!(profile.total_tiles, 3);
        assert_eq!(profile.flat_tiles, 1);
        assert_eq!(profile.shaped_tiles, 2);
        assert_eq!(profile.shape_histogram[0], 1);
        assert_eq!(profile.shape_histogram[1], 1);
        assert_eq!(profile.shape_histogram[2], 1);
        assert_eq!(profile.top_companions(3), vec![(8, 1), (9, 1)]);
        // (1,2) borders bare (0,2)=5 and (2,2)=5; (2,1) borders bare (2,2)=5 and (2,0)=7.
        assert_eq!(profile.top_shoulders(3), vec![(5, 3), (7, 1)]);
    }
}
