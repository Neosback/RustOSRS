//! Reproducible simple-tiled catalogs learned from reviewed symbolic grids.
mod capture;
pub use capture::learn_prefab_catalog;
mod model;
mod placement;
use crate::{
    CardinalDirection, MAX_WFC_PATTERNS, WfcCatalog, WfcPatternId, WfcPatternSpec, WfcSocket,
};
pub use model::LearnedRoleModel;
pub use placement::plan_wfc_floors;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LearnedPattern {
    pub floor_class: u32,
    /// Explicit north/east/south/west order. 0..3: open/wall/door/window; +4: exterior; 8: unknown.
    pub sockets: [u16; 4],
    #[serde(default)]
    pub shape: u8,
    #[serde(default)]
    pub rotation: u8,
}

impl LearnedPattern {
    pub const fn rotate_cw(&self, turns: u8) -> Self {
        let mut s = self.sockets;
        let mut i = 0;
        let count = turns & 3;
        while i < count {
            s = [s[3], s[0], s[1], s[2]];
            i += 1;
        }
        Self {
            shape: self.shape,
            rotation: if self.floor_class & 0xffff == 0 {
                0
            } else {
                (self.rotation + turns) & 3
            },
            floor_class: self.floor_class,
            sockets: s,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LearningGrid {
    pub width: u16,
    pub height: u16,
    pub cells: Vec<LearnedPattern>,
    pub reviewed: bool,
    #[serde(default)]
    pub active: Vec<bool>,
}

impl LearningGrid {
    /// Rotate the grid and its cell sockets 90 degrees clockwise.
    pub fn rotate_cw(&self) -> Self {
        let w = usize::from(self.width);
        let h = usize::from(self.height);
        if w == 0 || h == 0 || self.cells.len() != w * h {
            return self.clone();
        }
        let new_w = h;
        let new_h = w;
        let mut cells = vec![
            LearnedPattern {
                shape: 0,
                rotation: 0,
                floor_class: 0,
                sockets: [0; 4],
            };
            new_w * new_h
        ];
        for y in 0..h {
            for x in 0..w {
                let new_x = y;
                let new_y = w - 1 - x;
                cells[new_y * new_w + new_x] = self.cells[y * w + x].rotate_cw(1);
            }
        }
        Self {
            width: self.height,
            height: self.width,
            cells,
            reviewed: self.reviewed,
            active: if self.active.is_empty() {
                vec![]
            } else {
                let mut active = vec![false; self.active.len()];
                for y in 0..h {
                    for x in 0..w {
                        active[(w - 1 - x) * h + y] = self.active[y * w + x];
                    }
                }
                active
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LearnedCatalog {
    pub format: String,
    pub version: u32,
    pub style: String,
    pub role: String,
    pub source: String,
    pub patterns: Vec<LearnedPattern>,
    pub weights: Vec<u32>,
    /// Pattern, direction (N=0,E=1,S=2,W=3), neighboring pattern.
    pub adjacency: Vec<(u16, u8, u16)>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LearningError {
    Unreviewed,
    InvalidGrid,
    TooManyPatterns(usize),
    CountOverflow,
    InvalidCatalog,
}
impl std::fmt::Display for LearningError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "learning catalog: {self:?}")
    }
}
impl std::error::Error for LearningError {}

impl LearnedCatalog {
    pub fn compile(&self) -> Result<WfcCatalog, LearningError> {
        if self.format != "rustosrs-learned-catalog"
            || !(1..=2).contains(&self.version)
            || self.patterns.len() != self.weights.len()
            || self.patterns.is_empty()
            || self.patterns.iter().any(|p| p.shape > 11 || p.rotation > 3)
        {
            return Err(LearningError::InvalidCatalog);
        }
        let specs = self
            .patterns
            .iter()
            .zip(&self.weights)
            .map(|(p, &weight)| WfcPatternSpec {
                weight,
                sockets: p.sockets.map(WfcSocket),
            });
        let mut edges = Vec::new();
        for &(a, d, b) in &self.adjacency {
            let direction = *CardinalDirection::ALL
                .get(usize::from(d))
                .ok_or(LearningError::InvalidCatalog)?;
            edges.push((
                WfcPatternId::new(a).ok_or(LearningError::InvalidCatalog)?,
                direction,
                WfcPatternId::new(b).ok_or(LearningError::InvalidCatalog)?,
            ));
        }
        WfcCatalog::from_adjacencies(specs, &edges).map_err(|_| LearningError::InvalidCatalog)
    }
}

pub fn learn_catalog(
    grids: &[LearningGrid],
    style: String,
    role: String,
    source: String,
) -> Result<LearnedCatalog, LearningError> {
    let mut counts = BTreeMap::<LearnedPattern, u32>::new();
    for grid in grids {
        if !grid.reviewed {
            return Err(LearningError::Unreviewed);
        }
        if grid.width == 0
            || grid.height == 0
            || grid.cells.len() != usize::from(grid.width) * usize::from(grid.height)
            || (!grid.active.is_empty() && grid.active.len() != grid.cells.len())
        {
            return Err(LearningError::InvalidGrid);
        }
        for (index, p) in grid.cells.iter().enumerate() {
            if !grid.active.is_empty() && !grid.active[index] {
                continue;
            }
            let count = counts.entry(p.clone()).or_default();
            *count = count.checked_add(1).ok_or(LearningError::CountOverflow)?;
        }
    }
    if counts.len() > MAX_WFC_PATTERNS {
        return Err(LearningError::TooManyPatterns(counts.len()));
    }
    if counts.is_empty() {
        return Err(LearningError::InvalidGrid);
    }
    let patterns: Vec<_> = counts.keys().cloned().collect();
    let weights = counts.values().copied().collect();
    let ids: BTreeMap<_, _> = patterns
        .iter()
        .enumerate()
        .map(|(i, p)| {
            (
                p.clone(),
                u16::try_from(i).unwrap_or_else(|_| unreachable!("capped by MAX_WFC_PATTERNS")),
            )
        })
        .collect();
    let mut adjacency = BTreeSet::new();
    for grid in grids {
        let w = usize::from(grid.width);
        let h = usize::from(grid.height);
        for y in 0..h {
            for x in 0..w {
                if !grid.active.is_empty() && !grid.active[y * w + x] {
                    continue;
                }
                let a = ids[&grid.cells[y * w + x]];
                for (d, dx, dy) in [(0, 0, 1), (1, 1, 0), (2, 0, -1), (3, -1, 0)] {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    if nx >= 0
                        && ny >= 0
                        && (nx as usize) < w
                        && (ny as usize) < h
                        && (grid.active.is_empty() || grid.active[ny as usize * w + nx as usize])
                    {
                        adjacency.insert((a, d, ids[&grid.cells[ny as usize * w + nx as usize]]));
                    }
                }
            }
        }
    }
    let catalog = LearnedCatalog {
        format: "rustosrs-learned-catalog".into(),
        version: 2,
        style,
        role,
        source,
        patterns,
        weights,
        adjacency: adjacency.into_iter().collect(),
    };
    catalog.compile()?;
    Ok(catalog)
}

/// Learn a catalog across all four 90-degree clockwise rotations of `grids`.
pub fn learn_catalog_with_rotations(
    grids: &[LearningGrid],
    style: String,
    role: String,
    source: String,
) -> Result<LearnedCatalog, LearningError> {
    let mut expanded = Vec::with_capacity(grids.len() * 4);
    for grid in grids {
        let r0 = grid.clone();
        let r1 = r0.rotate_cw();
        let r2 = r1.rotate_cw();
        let r3 = r2.rotate_cw();
        expanded.extend([r0, r1, r2, r3]);
    }
    learn_catalog(&expanded, style, role, source)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pattern(class: u32) -> LearnedPattern {
        LearnedPattern {
            shape: 0,
            rotation: 0,
            floor_class: class,
            sockets: [0; 4],
        }
    }
    #[test]
    fn observations_do_not_allow_unseen_pairs_with_identical_sockets() {
        let grids = [LearningGrid {
            active: vec![],
            width: 2,
            height: 1,
            cells: vec![pattern(1), pattern(2)],
            reviewed: true,
        }];
        let learned = learn_catalog(&grids, "town".into(), "house".into(), "fixture".into())
            .unwrap_or_else(|e| panic!("{e}"));
        let catalog = learned.compile().unwrap_or_else(|e| panic!("{e}"));
        let a = WfcPatternId::new(0).unwrap_or_else(|| unreachable!());
        let b = WfcPatternId::new(1).unwrap_or_else(|| unreachable!());
        assert!(
            catalog
                .compatible_domain(a, CardinalDirection::East)
                .contains(b)
        );
        assert!(
            !catalog
                .compatible_domain(a, CardinalDirection::East)
                .contains(a)
        );
        assert!(
            catalog
                .compatible_domain(b, CardinalDirection::West)
                .contains(a)
        );
        assert_eq!(learned.weights, [1, 1]);
    }
    #[test]
    fn catalogs_are_sorted_and_reject_unreviewed_or_oversized_input() {
        let a = LearningGrid {
            active: vec![],
            width: 1,
            height: 1,
            cells: vec![pattern(2)],
            reviewed: true,
        };
        let b = LearningGrid {
            active: vec![],
            width: 1,
            height: 1,
            cells: vec![pattern(1)],
            reviewed: true,
        };
        assert_eq!(
            learn_catalog(&[a.clone(), b.clone()], "s".into(), "r".into(), "x".into()),
            learn_catalog(&[b, a], "s".into(), "r".into(), "x".into())
        );
        let grid = LearningGrid {
            active: vec![],
            width: 513,
            height: 1,
            cells: (0..513).map(pattern).collect(),
            reviewed: true,
        };
        assert_eq!(
            learn_catalog(&[grid], "".into(), "".into(), "".into()),
            Err(LearningError::TooManyPatterns(513))
        );
        assert_eq!(
            learn_catalog(
                &[LearningGrid {
                    active: vec![],
                    width: 1,
                    height: 1,
                    cells: vec![pattern(0)],
                    reviewed: false
                }],
                "".into(),
                "".into(),
                "".into()
            ),
            Err(LearningError::Unreviewed)
        );
    }

    #[test]
    fn rotation_synthesis_rotates_sockets_and_adjacencies() {
        let grids = [LearningGrid {
            active: vec![],
            width: 2,
            height: 1,
            cells: vec![
                LearnedPattern {
                    shape: 0,
                    rotation: 0,
                    floor_class: 1,
                    sockets: [10, 20, 30, 40],
                },
                LearnedPattern {
                    shape: 0,
                    rotation: 0,
                    floor_class: 2,
                    sockets: [50, 60, 70, 20],
                },
            ],
            reviewed: true,
        }];
        let learned =
            learn_catalog_with_rotations(&grids, "town".into(), "house".into(), "fixture".into())
                .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(learned.patterns.len(), 8);
        assert!(learned.compile().is_ok());
    }
}
