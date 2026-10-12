//! The shape atlas: every overlay shape in every orientation as a bit mask at a chosen
//! resolution, with the facts the planner needs of each (which sides it reaches, how much of the
//! tile it covers).

use super::shapes::shape_triangles;

pub const NORTH: u8 = 1;
pub const EAST: u8 = 2;
pub const SOUTH: u8 = 4;
pub const WEST: u8 = 8;

/// A square bit mask, row-major (`y * resolution + x`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitMask {
    words: Vec<u64>,
    bits: usize,
}

impl BitMask {
    pub fn new(bits: usize) -> Self {
        Self {
            words: vec![0; bits.div_ceil(64)],
            bits,
        }
    }

    pub fn set(&mut self, index: usize) {
        self.words[index >> 6] |= 1 << (index & 63);
    }

    pub fn get(&self, index: usize) -> bool {
        self.words[index >> 6] >> (index & 63) & 1 == 1
    }

    pub fn cardinality(&self) -> u32 {
        self.words.iter().map(|w| w.count_ones()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.words.iter().all(|w| *w == 0)
    }

    /// The set bits, lowest first.
    pub fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.bits).filter(|i| self.get(*i))
    }
}

#[derive(Debug, Clone)]
struct Entry {
    mask: BitMask,
    portals: u8,
    coverage: f64,
    edges: [u32; 4],
}

pub struct OverlayShapeAtlas {
    resolution: usize,
    entries: Vec<Entry>,
}

fn inside(point: [f64; 2], triangle: &[[f32; 2]; 3]) -> bool {
    let at = |i: usize| [f64::from(triangle[i][0]), f64::from(triangle[i][1])];
    let side = |p: [f64; 2], a: [f64; 2], b: [f64; 2]| {
        (p[0] - b[0]) * (a[1] - b[1]) - (a[0] - b[0]) * (p[1] - b[1])
    };
    let d1 = side(point, at(0), at(1));
    let d2 = side(point, at(1), at(2));
    let d3 = side(point, at(2), at(0));
    let negative = d1 < -1.0e-9 || d2 < -1.0e-9 || d3 < -1.0e-9;
    let positive = d1 > 1.0e-9 || d2 > 1.0e-9 || d3 > 1.0e-9;
    !(negative && positive)
}

fn area(triangle: &[[f32; 2]; 3]) -> f64 {
    let (a, b, c) = (triangle[0], triangle[1], triangle[2]);
    f64::from(((b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])).abs()) / 2.0
}

fn edge_of(mask: &BitMask, resolution: usize, direction: u8) -> u32 {
    let mut bits = 0;
    for i in 0..resolution {
        let index = match direction {
            NORTH => i,
            EAST => i * resolution + resolution - 1,
            SOUTH => (resolution - 1) * resolution + i,
            _ => i * resolution,
        };
        if mask.get(index) {
            bits |= 1 << i;
        }
    }
    bits
}

impl OverlayShapeAtlas {
    pub fn new(resolution: usize) -> Self {
        let resolution = resolution.clamp(2, 16);
        let mut entries = Vec::with_capacity(48);
        for shape in 0..12_u8 {
            for orientation in 0..4_u8 {
                let triangles = shape_triangles(shape, orientation);
                let overlay: Vec<_> = triangles.iter().filter(|t| t.1).map(|t| t.0).collect();
                let underlay: Vec<_> = triangles.iter().filter(|t| !t.1).map(|t| t.0).collect();
                let mut mask = BitMask::new(resolution * resolution);
                for y in 0..resolution {
                    for x in 0..resolution {
                        let (mut over, mut under) = (0, 0);
                        for sy in 0..4 {
                            for sx in 0..4 {
                                let p = [
                                    (x as f64 + (sx as f64 + 0.5) / 4.0) / resolution as f64,
                                    (y as f64 + (sy as f64 + 0.5) / 4.0) / resolution as f64,
                                ];
                                if overlay.iter().any(|t| inside(p, t)) {
                                    over += 1;
                                }
                                if underlay.iter().any(|t| inside(p, t)) {
                                    under += 1;
                                }
                            }
                        }
                        if over >= under {
                            mask.set(y * resolution + x);
                        }
                    }
                }
                let edges = [NORTH, EAST, SOUTH, WEST].map(|d| edge_of(&mask, resolution, d));
                let portals =
                    edges.iter().enumerate().fold(
                        0,
                        |acc, (i, bits)| if *bits != 0 { acc | 1 << i } else { acc },
                    );
                entries.push(Entry {
                    mask,
                    portals,
                    coverage: overlay.iter().map(area).sum(),
                    edges,
                });
            }
        }
        Self {
            resolution,
            entries,
        }
    }

    pub fn resolution(&self) -> usize {
        self.resolution
    }

    fn entry(&self, shape: u8, orientation: u8) -> &Entry {
        &self.entries[(usize::from(shape.min(11)) << 2) + usize::from(orientation & 3)]
    }

    pub fn mask(&self, shape: u8, orientation: u8) -> &BitMask {
        &self.entry(shape, orientation).mask
    }

    /// Which sides the shape reaches (`NORTH | EAST | SOUTH | WEST`).
    #[allow(dead_code, reason = "part of the atlas API")]
    pub fn portals(&self, shape: u8, orientation: u8) -> u8 {
        self.entry(shape, orientation).portals
    }

    /// The share of the tile the shape's overlay covers.
    #[allow(dead_code, reason = "part of the atlas API")]
    pub fn coverage(&self, shape: u8, orientation: u8) -> f64 {
        self.entry(shape, orientation).coverage
    }

    /// The bits of the shape's overlay along one side (`direction` is one of the side constants).
    pub fn edge_mask(&self, shape: u8, orientation: u8, direction: u8) -> u32 {
        let edges = &self.entry(shape, orientation).edges;
        match direction {
            NORTH => edges[0],
            EAST => edges[1],
            SOUTH => edges[2],
            _ => edges[3],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shape_zero_fills_the_tile() {
        let atlas = OverlayShapeAtlas::new(8);
        for orientation in 0..4 {
            assert_eq!(atlas.mask(0, orientation).cardinality(), 64);
            assert_eq!(atlas.portals(0, orientation), 15);
            assert!((atlas.coverage(0, orientation) - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn the_diagonal_halves_cover_about_half() {
        let atlas = OverlayShapeAtlas::new(8);
        // Shape 1 is the diagonal cut: a triangle of the tile.
        let cells = atlas.mask(1, 0).cardinality();
        assert!((20..=44).contains(&cells), "{cells}");
        assert!((atlas.coverage(1, 0) - 0.5).abs() < 0.2);
    }

    #[test]
    fn rotating_a_shape_turns_its_sides() {
        let atlas = OverlayShapeAtlas::new(8);
        // The four orientations of the quarter-tile corner reach two sides each, all different pairs.
        let mut seen = Vec::new();
        for orientation in 0..4 {
            let portals = atlas.portals(2, orientation);
            assert!(portals.count_ones() >= 2);
            seen.push(portals);
        }
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), 4);
    }
}
