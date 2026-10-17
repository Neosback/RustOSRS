//! Deterministic simple-tiled Wave Function Collapse over semantic pattern sockets.
//!
//! This solver deliberately knows nothing about RuneScape object IDs, rendering, or editor UI.
//! Patterns expose only directional sockets and weights. Higher building layers decide what those
//! patterns mean and later resolve the semantic result through normal building realization.

use crate::CardinalDirection;
use std::collections::VecDeque;

pub const MAX_WFC_PATTERNS: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WfcPatternId(u8);

impl WfcPatternId {
    pub const fn new(value: u8) -> Option<Self> {
        if (value as usize) < MAX_WFC_PATTERNS {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WfcSocket(pub u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WfcPatternSpec {
    pub weight: u32,
    /// Socket order is north, east, south, west.
    pub sockets: [WfcSocket; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WfcPattern {
    pub id: WfcPatternId,
    pub weight: u32,
    pub sockets: [WfcSocket; 4],
}

impl WfcPattern {
    pub const fn socket(self, direction: CardinalDirection) -> WfcSocket {
        self.sockets[direction.index()]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PatternDomain(u128);

impl PatternDomain {
    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn all(pattern_count: usize) -> Self {
        if pattern_count == 0 {
            Self::empty()
        } else if pattern_count >= MAX_WFC_PATTERNS {
            Self(u128::MAX)
        } else {
            Self((1_u128 << pattern_count) - 1)
        }
    }

    pub const fn singleton(pattern: WfcPatternId) -> Self {
        Self(1_u128 << pattern.0)
    }

    pub fn from_patterns(patterns: impl IntoIterator<Item = WfcPatternId>) -> Self {
        let mut domain = Self::empty();
        for pattern in patterns {
            domain.0 |= 1_u128 << pattern.0;
        }
        domain
    }

    pub const fn contains(self, pattern: WfcPatternId) -> bool {
        self.0 & (1_u128 << pattern.0) != 0
    }

    pub const fn len(self) -> u32 {
        self.0.count_ones()
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub const fn is_singleton(self) -> bool {
        self.0.count_ones() == 1
    }

    pub const fn intersect(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn without(self, pattern: WfcPatternId) -> Self {
        Self(self.0 & !(1_u128 << pattern.0))
    }

    pub fn iter(self) -> PatternDomainIter {
        PatternDomainIter { remaining: self.0 }
    }
}

pub struct PatternDomainIter {
    remaining: u128,
}

impl Iterator for PatternDomainIter {
    type Item = WfcPatternId;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let bit = self.remaining.trailing_zeros();
        self.remaining &= self.remaining - 1;
        WfcPatternId::new(u8::try_from(bit).ok()?)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WfcCatalog {
    patterns: Vec<WfcPattern>,
    compatibility: Vec<[PatternDomain; 4]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WfcCatalogError {
    EmptyCatalog,
    TooManyPatterns { count: usize },
    ZeroWeight { pattern: usize },
}

impl WfcCatalog {
    pub fn new(specs: impl IntoIterator<Item = WfcPatternSpec>) -> Result<Self, WfcCatalogError> {
        let specs = specs.into_iter().collect::<Vec<_>>();
        if specs.is_empty() {
            return Err(WfcCatalogError::EmptyCatalog);
        }
        if specs.len() > MAX_WFC_PATTERNS {
            return Err(WfcCatalogError::TooManyPatterns { count: specs.len() });
        }

        let mut patterns = Vec::with_capacity(specs.len());
        for (index, spec) in specs.into_iter().enumerate() {
            if spec.weight == 0 {
                return Err(WfcCatalogError::ZeroWeight { pattern: index });
            }
            let id = WfcPatternId::new(
                u8::try_from(index).unwrap_or_else(|_| unreachable!("catalog limit is 128")),
            )
            .unwrap_or_else(|| unreachable!("catalog limit guarantees a valid pattern id"));
            patterns.push(WfcPattern {
                id,
                weight: spec.weight,
                sockets: spec.sockets,
            });
        }

        let mut compatibility = vec![[PatternDomain::empty(); 4]; patterns.len()];
        for pattern in &patterns {
            for direction in CardinalDirection::ALL {
                let opposite = opposite(direction);
                let mut compatible = PatternDomain::empty();
                for neighbor in &patterns {
                    if pattern.socket(direction) == neighbor.socket(opposite) {
                        compatible = compatible.union(PatternDomain::singleton(neighbor.id));
                    }
                }
                compatibility[usize::from(pattern.id.get())][direction.index()] = compatible;
            }
        }

        Ok(Self {
            patterns,
            compatibility,
        })
    }

    pub fn patterns(&self) -> &[WfcPattern] {
        &self.patterns
    }

    pub const fn pattern_count(&self) -> usize {
        self.patterns.len()
    }

    pub fn pattern(&self, id: WfcPatternId) -> Option<WfcPattern> {
        self.patterns.get(usize::from(id.get())).copied()
    }

    pub fn compatible_domain(
        &self,
        pattern: WfcPatternId,
        direction: CardinalDirection,
    ) -> PatternDomain {
        self.compatibility
            .get(usize::from(pattern.get()))
            .map(|compatibility| compatibility[direction.index()])
            .unwrap_or_default()
    }

    pub fn all_patterns(&self) -> PatternDomain {
        PatternDomain::all(self.patterns.len())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WfcCoord {
    pub x: u16,
    pub y: u16,
}

impl WfcCoord {
    pub const fn new(x: u16, y: u16) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WfcConstraint {
    pub coord: WfcCoord,
    pub allowed: PatternDomain,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WfcSolution {
    width: u16,
    height: u16,
    cells: Vec<WfcPatternId>,
    backtracks: u32,
}

impl WfcSolution {
    pub const fn width(&self) -> u16 {
        self.width
    }

    pub const fn height(&self) -> u16 {
        self.height
    }

    pub fn cells(&self) -> &[WfcPatternId] {
        &self.cells
    }

    pub const fn backtracks(&self) -> u32 {
        self.backtracks
    }

    pub fn at(&self, coord: WfcCoord) -> Option<WfcPatternId> {
        let index = grid_index(self.width, self.height, coord)?;
        self.cells.get(index).copied()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WfcSolveError {
    InvalidDimensions { width: u16, height: u16 },
    CellCountOverflow,
    ConstraintOutsideGrid { coord: WfcCoord },
    ConstraintHasNoValidPatterns { coord: WfcCoord },
    Contradiction { coord: WfcCoord, backtracks: u32 },
}

#[derive(Debug, Clone)]
struct DecisionFrame {
    domains: Vec<PatternDomain>,
    cell: usize,
    remaining: PatternDomain,
}

pub fn solve_wfc(
    catalog: &WfcCatalog,
    width: u16,
    height: u16,
    seed: u64,
    constraints: &[WfcConstraint],
    max_backtracks: u32,
) -> Result<WfcSolution, WfcSolveError> {
    if width == 0 || height == 0 {
        return Err(WfcSolveError::InvalidDimensions { width, height });
    }
    let cell_count = usize::from(width)
        .checked_mul(usize::from(height))
        .ok_or(WfcSolveError::CellCountOverflow)?;
    let all_patterns = catalog.all_patterns();
    let mut domains = vec![all_patterns; cell_count];

    for constraint in constraints {
        let Some(index) = grid_index(width, height, constraint.coord) else {
            return Err(WfcSolveError::ConstraintOutsideGrid {
                coord: constraint.coord,
            });
        };
        let narrowed = domains[index].intersect(constraint.allowed.intersect(all_patterns));
        if narrowed.is_empty() {
            return Err(WfcSolveError::ConstraintHasNoValidPatterns {
                coord: constraint.coord,
            });
        }
        domains[index] = narrowed;
    }

    let initial_queue = (0..cell_count).collect::<VecDeque<_>>();
    if let Err(cell) = propagate(catalog, width, height, &mut domains, initial_queue) {
        return Err(WfcSolveError::Contradiction {
            coord: coord_from_index(width, cell),
            backtracks: 0,
        });
    }

    let mut rng = SplitMix64::new(seed);
    let mut decisions = Vec::<DecisionFrame>::new();
    let mut backtracks = 0_u32;

    loop {
        let Some(cell) = select_lowest_entropy_cell(&domains) else {
            let cells = domains
                .iter()
                .map(|domain| {
                    domain
                        .iter()
                        .next()
                        .unwrap_or_else(|| unreachable!("solved domains are singletons"))
                })
                .collect();
            return Ok(WfcSolution {
                width,
                height,
                cells,
                backtracks,
            });
        };

        let before = domains.clone();
        let chosen = choose_weighted(catalog, domains[cell], &mut rng);
        let remaining = domains[cell].without(chosen);
        if !remaining.is_empty() {
            decisions.push(DecisionFrame {
                domains: before,
                cell,
                remaining,
            });
        }
        domains[cell] = PatternDomain::singleton(chosen);

        if let Err(contradiction) =
            propagate(catalog, width, height, &mut domains, VecDeque::from([cell]))
        {
            restore_after_contradiction(
                catalog,
                width,
                height,
                &mut domains,
                &mut decisions,
                &mut rng,
                &mut backtracks,
                max_backtracks,
                contradiction,
            )?;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn restore_after_contradiction(
    catalog: &WfcCatalog,
    width: u16,
    height: u16,
    domains: &mut Vec<PatternDomain>,
    decisions: &mut Vec<DecisionFrame>,
    rng: &mut SplitMix64,
    backtracks: &mut u32,
    max_backtracks: u32,
    mut contradiction: usize,
) -> Result<(), WfcSolveError> {
    loop {
        let Some(frame) = decisions.pop() else {
            return Err(WfcSolveError::Contradiction {
                coord: coord_from_index(width, contradiction),
                backtracks: *backtracks,
            });
        };
        if *backtracks >= max_backtracks {
            return Err(WfcSolveError::Contradiction {
                coord: coord_from_index(width, contradiction),
                backtracks: *backtracks,
            });
        }
        *backtracks += 1;
        *domains = frame.domains;

        let chosen = choose_weighted(catalog, frame.remaining, rng);
        let remaining = frame.remaining.without(chosen);
        if !remaining.is_empty() {
            decisions.push(DecisionFrame {
                domains: domains.clone(),
                cell: frame.cell,
                remaining,
            });
        }
        domains[frame.cell] = PatternDomain::singleton(chosen);

        match propagate(
            catalog,
            width,
            height,
            domains,
            VecDeque::from([frame.cell]),
        ) {
            Ok(()) => return Ok(()),
            Err(cell) => contradiction = cell,
        }
    }
}

fn propagate(
    catalog: &WfcCatalog,
    width: u16,
    height: u16,
    domains: &mut [PatternDomain],
    mut queue: VecDeque<usize>,
) -> Result<(), usize> {
    while let Some(cell) = queue.pop_front() {
        let source = domains[cell];
        if source.is_empty() {
            return Err(cell);
        }
        let coord = coord_from_index(width, cell);

        for direction in CardinalDirection::ALL {
            let Some(neighbor_coord) = grid_neighbor(width, height, coord, direction) else {
                continue;
            };
            let neighbor = grid_index(width, height, neighbor_coord)
                .unwrap_or_else(|| unreachable!("bounded neighbor must have an index"));
            let allowed = compatible_union(catalog, source, direction);
            let narrowed = domains[neighbor].intersect(allowed);
            if narrowed == domains[neighbor] {
                continue;
            }
            domains[neighbor] = narrowed;
            if narrowed.is_empty() {
                return Err(neighbor);
            }
            queue.push_back(neighbor);
        }
    }
    Ok(())
}

fn compatible_union(
    catalog: &WfcCatalog,
    domain: PatternDomain,
    direction: CardinalDirection,
) -> PatternDomain {
    domain
        .iter()
        .fold(PatternDomain::empty(), |allowed, pattern| {
            allowed.union(catalog.compatible_domain(pattern, direction))
        })
}

fn select_lowest_entropy_cell(domains: &[PatternDomain]) -> Option<usize> {
    domains
        .iter()
        .enumerate()
        .filter_map(|(index, domain)| {
            let entropy = domain.len();
            (entropy > 1).then_some((entropy, index))
        })
        .min()
        .map(|(_, index)| index)
}

fn choose_weighted(
    catalog: &WfcCatalog,
    domain: PatternDomain,
    rng: &mut SplitMix64,
) -> WfcPatternId {
    let total = domain.iter().fold(0_u64, |total, pattern| {
        let weight = catalog
            .pattern(pattern)
            .map(|pattern| u64::from(pattern.weight))
            .unwrap_or(0);
        total.saturating_add(weight)
    });
    debug_assert!(total > 0, "non-empty WFC domains must have positive weight");

    let mut roll = rng.next_u64() % total;
    for pattern in domain.iter() {
        let weight = catalog
            .pattern(pattern)
            .map(|pattern| u64::from(pattern.weight))
            .unwrap_or(0);
        if roll < weight {
            return pattern;
        }
        roll -= weight;
    }
    unreachable!("weighted selection must choose one pattern")
}

fn grid_index(width: u16, height: u16, coord: WfcCoord) -> Option<usize> {
    if coord.x >= width || coord.y >= height {
        return None;
    }
    Some(usize::from(coord.y) * usize::from(width) + usize::from(coord.x))
}

fn coord_from_index(width: u16, index: usize) -> WfcCoord {
    let width = usize::from(width);
    WfcCoord {
        x: u16::try_from(index % width)
            .unwrap_or_else(|_| unreachable!("grid x fits source width")),
        y: u16::try_from(index / width)
            .unwrap_or_else(|_| unreachable!("grid y fits source height")),
    }
}

fn grid_neighbor(
    width: u16,
    height: u16,
    coord: WfcCoord,
    direction: CardinalDirection,
) -> Option<WfcCoord> {
    match direction {
        CardinalDirection::North if coord.y + 1 < height => {
            Some(WfcCoord::new(coord.x, coord.y + 1))
        }
        CardinalDirection::East if coord.x + 1 < width => Some(WfcCoord::new(coord.x + 1, coord.y)),
        CardinalDirection::South if coord.y > 0 => Some(WfcCoord::new(coord.x, coord.y - 1)),
        CardinalDirection::West if coord.x > 0 => Some(WfcCoord::new(coord.x - 1, coord.y)),
        _ => None,
    }
}

const fn opposite(direction: CardinalDirection) -> CardinalDirection {
    match direction {
        CardinalDirection::North => CardinalDirection::South,
        CardinalDirection::East => CardinalDirection::West,
        CardinalDirection::South => CardinalDirection::North,
        CardinalDirection::West => CardinalDirection::East,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn socket(value: u16) -> WfcSocket {
        WfcSocket(value)
    }

    fn pattern(weight: u32, north: u16, east: u16, south: u16, west: u16) -> WfcPatternSpec {
        WfcPatternSpec {
            weight,
            sockets: [socket(north), socket(east), socket(south), socket(west)],
        }
    }

    fn id(value: u8) -> WfcPatternId {
        WfcPatternId::new(value).unwrap_or_else(|| unreachable!("test pattern id is valid"))
    }

    #[test]
    fn same_seed_and_constraints_produce_identical_solution() {
        let catalog = WfcCatalog::new([
            pattern(1, 0, 0, 0, 0),
            pattern(3, 0, 0, 0, 0),
            pattern(6, 0, 0, 0, 0),
        ])
        .unwrap_or_else(|error| unreachable!("valid test catalog: {error:?}"));
        let constraints = [WfcConstraint {
            coord: WfcCoord::new(1, 1),
            allowed: PatternDomain::from_patterns([id(0), id(1)]),
        }];

        let first = solve_wfc(&catalog, 4, 3, 9182, &constraints, 32)
            .unwrap_or_else(|error| unreachable!("solvable WFC: {error:?}"));
        let second = solve_wfc(&catalog, 4, 3, 9182, &constraints, 32)
            .unwrap_or_else(|error| unreachable!("solvable WFC: {error:?}"));

        assert_eq!(first, second);
    }

    #[test]
    fn solved_neighbors_have_matching_sockets() {
        let catalog = WfcCatalog::new([
            pattern(1, 0, 0, 0, 0),
            pattern(1, 1, 1, 1, 1),
            pattern(1, 0, 1, 0, 1),
        ])
        .unwrap_or_else(|error| unreachable!("valid test catalog: {error:?}"));
        let solution = solve_wfc(&catalog, 5, 4, 77, &[], 64)
            .unwrap_or_else(|error| unreachable!("solvable WFC: {error:?}"));

        for y in 0..solution.height() {
            for x in 0..solution.width() {
                let coord = WfcCoord::new(x, y);
                let pattern = catalog
                    .pattern(
                        solution
                            .at(coord)
                            .unwrap_or_else(|| unreachable!("solution cell exists")),
                    )
                    .unwrap_or_else(|| unreachable!("solution pattern exists"));
                for direction in CardinalDirection::ALL {
                    let Some(neighbor_coord) =
                        grid_neighbor(solution.width(), solution.height(), coord, direction)
                    else {
                        continue;
                    };
                    let neighbor = catalog
                        .pattern(
                            solution
                                .at(neighbor_coord)
                                .unwrap_or_else(|| unreachable!("neighbor cell exists")),
                        )
                        .unwrap_or_else(|| unreachable!("neighbor pattern exists"));
                    assert_eq!(
                        pattern.socket(direction),
                        neighbor.socket(opposite(direction))
                    );
                }
            }
        }
    }

    #[test]
    fn fixed_constraint_remains_fixed() {
        let catalog = WfcCatalog::new([pattern(1, 0, 0, 0, 0), pattern(1, 0, 0, 0, 0)])
            .unwrap_or_else(|error| unreachable!("valid test catalog: {error:?}"));
        let fixed = WfcCoord::new(2, 1);
        let solution = solve_wfc(
            &catalog,
            4,
            3,
            11,
            &[WfcConstraint {
                coord: fixed,
                allowed: PatternDomain::singleton(id(1)),
            }],
            16,
        )
        .unwrap_or_else(|error| unreachable!("solvable WFC: {error:?}"));

        assert_eq!(solution.at(fixed), Some(id(1)));
    }

    #[test]
    fn contradiction_backtracks_to_a_valid_alternative() {
        let catalog = WfcCatalog::new([
            pattern(1, 0, 0, 0, 0),
            pattern(1, 0, 1, 1, 0),
            pattern(1000, 1, 1, 0, 1),
        ])
        .unwrap_or_else(|error| unreachable!("valid test catalog: {error:?}"));
        let solution = solve_wfc(&catalog, 2, 2, 0, &[], 16)
            .unwrap_or_else(|error| unreachable!("backtracking should recover: {error:?}"));

        assert!(solution.backtracks() > 0);
        assert_ne!(solution.at(WfcCoord::new(0, 0)), Some(id(2)));
    }

    #[test]
    fn incompatible_fixed_constraints_report_contradiction() {
        let catalog = WfcCatalog::new([pattern(1, 0, 0, 0, 0), pattern(1, 1, 1, 1, 1)])
            .unwrap_or_else(|error| unreachable!("valid test catalog: {error:?}"));
        let result = solve_wfc(
            &catalog,
            2,
            1,
            5,
            &[
                WfcConstraint {
                    coord: WfcCoord::new(0, 0),
                    allowed: PatternDomain::singleton(id(0)),
                },
                WfcConstraint {
                    coord: WfcCoord::new(1, 0),
                    allowed: PatternDomain::singleton(id(1)),
                },
            ],
            8,
        );

        assert!(matches!(
            result,
            Err(WfcSolveError::Contradiction { backtracks: 0, .. })
        ));
    }

    #[test]
    fn catalog_rejects_zero_weight_patterns() {
        assert_eq!(
            WfcCatalog::new([pattern(0, 0, 0, 0, 0)]),
            Err(WfcCatalogError::ZeroWeight { pattern: 0 })
        );
    }
}
