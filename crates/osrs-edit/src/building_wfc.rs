//! Deterministic simple-tiled Wave Function Collapse over semantic pattern sockets.
//!
//! This solver deliberately knows nothing about RuneScape object IDs, rendering, or editor UI.
//! Patterns expose only directional sockets and weights. Higher building layers decide what those
//! patterns mean and later resolve the semantic result through normal building realization.

use crate::CardinalDirection;
use std::collections::VecDeque;

pub const MAX_WFC_PATTERNS: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WfcPatternId(u16);

impl WfcPatternId {
    pub const fn new(value: u16) -> Option<Self> {
        if (value as usize) < MAX_WFC_PATTERNS {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn get(self) -> u16 {
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
pub struct PatternDomain([u64; 8]);

impl PatternDomain {
    pub const fn empty() -> Self {
        Self([0; 8])
    }

    pub const fn all(pattern_count: usize) -> Self {
        if pattern_count == 0 {
            return Self::empty();
        }
        let capped = if pattern_count > MAX_WFC_PATTERNS {
            MAX_WFC_PATTERNS
        } else {
            pattern_count
        };
        let mut words = [0_u64; 8];
        let full_words = capped / 64;
        let rem_bits = capped % 64;
        let mut i = 0;
        while i < full_words {
            words[i] = u64::MAX;
            i += 1;
        }
        if rem_bits > 0 && full_words < 8 {
            words[full_words] = (1_u64 << rem_bits) - 1;
        }
        Self(words)
    }

    pub const fn singleton(pattern: WfcPatternId) -> Self {
        let idx = (pattern.0 as usize) / 64;
        let bit = (pattern.0 as usize) % 64;
        let mut words = [0_u64; 8];
        words[idx] = 1_u64 << bit;
        Self(words)
    }

    pub fn from_patterns(patterns: impl IntoIterator<Item = WfcPatternId>) -> Self {
        let mut domain = Self::empty();
        for pattern in patterns {
            let idx = usize::from(pattern.0) / 64;
            let bit = usize::from(pattern.0) % 64;
            domain.0[idx] |= 1_u64 << bit;
        }
        domain
    }

    pub const fn contains(self, pattern: WfcPatternId) -> bool {
        let idx = (pattern.0 as usize) / 64;
        let bit = (pattern.0 as usize) % 64;
        (self.0[idx] & (1_u64 << bit)) != 0
    }

    pub const fn len(self) -> u32 {
        self.0[0].count_ones()
            + self.0[1].count_ones()
            + self.0[2].count_ones()
            + self.0[3].count_ones()
            + self.0[4].count_ones()
            + self.0[5].count_ones()
            + self.0[6].count_ones()
            + self.0[7].count_ones()
    }

    pub const fn is_empty(self) -> bool {
        (self.0[0]
            | self.0[1]
            | self.0[2]
            | self.0[3]
            | self.0[4]
            | self.0[5]
            | self.0[6]
            | self.0[7])
            == 0
    }

    pub const fn is_singleton(self) -> bool {
        self.len() == 1
    }

    pub const fn intersect(self, other: Self) -> Self {
        Self([
            self.0[0] & other.0[0],
            self.0[1] & other.0[1],
            self.0[2] & other.0[2],
            self.0[3] & other.0[3],
            self.0[4] & other.0[4],
            self.0[5] & other.0[5],
            self.0[6] & other.0[6],
            self.0[7] & other.0[7],
        ])
    }

    pub const fn union(self, other: Self) -> Self {
        Self([
            self.0[0] | other.0[0],
            self.0[1] | other.0[1],
            self.0[2] | other.0[2],
            self.0[3] | other.0[3],
            self.0[4] | other.0[4],
            self.0[5] | other.0[5],
            self.0[6] | other.0[6],
            self.0[7] | other.0[7],
        ])
    }

    pub const fn without(self, pattern: WfcPatternId) -> Self {
        let idx = (pattern.0 as usize) / 64;
        let bit = (pattern.0 as usize) % 64;
        let mut words = self.0;
        words[idx] &= !(1_u64 << bit);
        Self(words)
    }

    pub fn first_pattern(self) -> Option<WfcPatternId> {
        self.iter().next()
    }

    pub fn iter(self) -> PatternDomainIter {
        PatternDomainIter {
            words: self.0,
            word_index: 0,
        }
    }
}

pub struct PatternDomainIter {
    words: [u64; 8],
    word_index: usize,
}

impl Iterator for PatternDomainIter {
    type Item = WfcPatternId;

    fn next(&mut self) -> Option<Self::Item> {
        while self.word_index < 8 {
            let word = self.words[self.word_index];
            if word != 0 {
                let bit = word.trailing_zeros();
                self.words[self.word_index] &= word - 1;
                let id = u16::try_from(self.word_index * 64 + usize::try_from(bit).ok()?).ok()?;
                return WfcPatternId::new(id);
            }
            self.word_index += 1;
        }
        None
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
    InvalidAdjacency { from: u16, to: u16 },
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
                u16::try_from(index).unwrap_or_else(|_| unreachable!("catalog limit is 512")),
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

    /// Build an observed compatibility table, intersected with semantic sockets. Reverse edges
    /// are installed together because an observation describes one physical shared boundary.
    pub fn from_adjacencies(
        specs: impl IntoIterator<Item = WfcPatternSpec>,
        edges: &[(WfcPatternId, CardinalDirection, WfcPatternId)],
    ) -> Result<Self, WfcCatalogError> {
        let mut catalog = Self::new(specs)?;
        let semantic = catalog.compatibility.clone();
        catalog.compatibility.fill([PatternDomain::empty(); 4]);
        for &(from, direction, to) in edges {
            if usize::from(from.get()) >= catalog.pattern_count()
                || usize::from(to.get()) >= catalog.pattern_count()
                || !semantic[usize::from(from.get())][direction.index()].contains(to)
            {
                return Err(WfcCatalogError::InvalidAdjacency {
                    from: from.get(),
                    to: to.get(),
                });
            }
            let reverse = opposite(direction);
            catalog.compatibility[usize::from(from.get())][direction.index()] = catalog
                .compatibility[usize::from(from.get())][direction.index()]
            .union(PatternDomain::singleton(to));
            catalog.compatibility[usize::from(to.get())][reverse.index()] = catalog.compatibility
                [usize::from(to.get())][reverse.index()]
            .union(PatternDomain::singleton(from));
        }
        Ok(catalog)
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
        let Some(cell) = select_lowest_entropy_cell(catalog, &domains) else {
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

/// Solve WFC and, if a contradiction occurs, relax constraints within a 2-tile radius around the
/// contradiction cell and retry up to `max_repairs` times.
pub fn solve_wfc_with_repair(
    catalog: &WfcCatalog,
    width: u16,
    height: u16,
    seed: u64,
    constraints: &[WfcConstraint],
    max_backtracks: u32,
    max_repairs: u32,
) -> Result<WfcSolution, WfcSolveError> {
    let mut active = constraints.to_vec();
    let mut current_seed = seed;
    let mut repairs = 0_u32;
    loop {
        match solve_wfc(
            catalog,
            width,
            height,
            current_seed,
            &active,
            max_backtracks,
        ) {
            Ok(solution) => return Ok(solution),
            Err(WfcSolveError::Contradiction { coord, backtracks }) => {
                if repairs >= max_repairs {
                    return Err(WfcSolveError::Contradiction { coord, backtracks });
                }
                repairs += 1;
                active
                    .retain(|c| c.coord.x.abs_diff(coord.x) > 2 || c.coord.y.abs_diff(coord.y) > 2);
                current_seed = SplitMix64::new(current_seed).next_u64();
            }
            Err(other) => return Err(other),
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

fn shannon_entropy_micros(catalog: &WfcCatalog, domain: PatternDomain) -> u64 {
    let mut sum_w = 0.0_f64;
    let mut sum_w_ln_w = 0.0_f64;
    for pattern in domain.iter() {
        let w = catalog
            .pattern(pattern)
            .map_or(1.0, |p| f64::from(p.weight.max(1)));
        sum_w += w;
        sum_w_ln_w += w * w.ln();
    }
    if sum_w <= 0.0 {
        return 0;
    }
    let entropy = (sum_w.ln() - sum_w_ln_w / sum_w).max(0.0);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    {
        (entropy * 1_000_000.0).round() as u64
    }
}

fn select_lowest_entropy_cell(catalog: &WfcCatalog, domains: &[PatternDomain]) -> Option<usize> {
    domains
        .iter()
        .enumerate()
        .filter_map(|(index, domain)| {
            let count = domain.len();
            (count > 1).then(|| (shannon_entropy_micros(catalog, *domain), count, index))
        })
        .min()
        .map(|(_, _, index)| index)
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

    fn id(value: u16) -> WfcPatternId {
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

    #[test]
    fn pattern_domain_and_catalog_support_up_to_512_patterns() {
        let specs = (0..300).map(|i| pattern(1 + (i % 5), 0, 0, 0, 0));
        let catalog = WfcCatalog::new(specs)
            .unwrap_or_else(|error| unreachable!("300-pattern catalog is valid: {error:?}"));
        assert_eq!(catalog.pattern_count(), 300);

        let all = catalog.all_patterns();
        assert_eq!(all.len(), 300);
        assert!(all.contains(id(0)));
        assert!(all.contains(id(299)));
        assert!(!all.contains(id(300)));

        let solution = solve_wfc(&catalog, 4, 4, 42, &[], 16)
            .unwrap_or_else(|error| unreachable!("solvable 300-pattern WFC: {error:?}"));
        assert_eq!(solution.cells().len(), 16);
    }

    #[test]
    fn weighted_shannon_entropy_prefers_skewed_two_pattern_cell_over_uniform() {
        let catalog = WfcCatalog::new([
            pattern(100, 0, 0, 0, 0),
            pattern(1, 0, 0, 0, 0),
            pattern(10, 0, 0, 0, 0),
            pattern(10, 0, 0, 0, 0),
        ])
        .unwrap_or_else(|error| unreachable!("valid catalog: {error:?}"));
        let uniform = PatternDomain::from_patterns([id(2), id(3)]);
        let skewed = PatternDomain::from_patterns([id(0), id(1)]);
        assert!(
            shannon_entropy_micros(&catalog, skewed) < shannon_entropy_micros(&catalog, uniform)
        );
        assert_eq!(
            select_lowest_entropy_cell(&catalog, &[uniform, skewed]),
            Some(1)
        );
    }

    #[test]
    fn solve_wfc_with_repair_relaxes_conflicting_radius_and_succeeds() {
        let catalog = WfcCatalog::new([pattern(1, 0, 0, 0, 0), pattern(1, 1, 1, 1, 1)])
            .unwrap_or_else(|error| unreachable!("valid test catalog: {error:?}"));
        let constraints = [
            WfcConstraint {
                coord: WfcCoord::new(0, 0),
                allowed: PatternDomain::singleton(id(0)),
            },
            WfcConstraint {
                coord: WfcCoord::new(1, 0),
                allowed: PatternDomain::singleton(id(1)),
            },
            WfcConstraint {
                coord: WfcCoord::new(5, 0),
                allowed: PatternDomain::singleton(id(0)),
            },
        ];
        assert!(solve_wfc(&catalog, 6, 1, 5, &constraints, 8).is_err());

        let repaired = solve_wfc_with_repair(&catalog, 6, 1, 5, &constraints, 8, 2)
            .unwrap_or_else(|error| unreachable!("repair should relax conflict: {error:?}"));
        // Far constraint at x=5 (outside radius 2) remains respected.
        assert_eq!(repaired.at(WfcCoord::new(5, 0)), Some(id(0)));
    }
}
