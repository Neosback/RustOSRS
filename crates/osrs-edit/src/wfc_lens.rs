//! What the floor solver (WFC) makes of a saved building, in a form a person can look at.
//!
//! WFC sees the floor of a building as a set of tile kinds. A kind is the ground material of a
//! tile plus what stands on each of its four edges (nothing, a wall, a door); the solver also
//! learns which kinds were seen next to which. It does not see furniture, decorations, roofs, what
//! a room is for, or the difference between a window and a wall. [`look`] builds that view from a
//! prefab; [`sample`] and [`trace`] run the solver on it so the result can be inspected.

use crate::{
    CardinalDirection, LearnedCatalog, LearnedPattern, PatternDomain, Prefab, WfcConstraint,
    WfcCoord, WfcEvent, WfcPatternId, WfcSolveError, solve_wfc, solve_wfc_observed,
};
use std::collections::{BTreeMap, BTreeSet};

/// What stands on one edge of a floor tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LensEdge {
    Open,
    Wall,
    Door,
    Window,
    Unknown,
}

impl LensEdge {
    fn from_socket(socket: u16) -> Self {
        if socket == 8 {
            return Self::Unknown;
        }
        match socket & 3 {
            3 => Self::Window,
            1 => Self::Wall,
            2 => Self::Door,
            _ => Self::Open,
        }
    }
}

/// What a placed object is, as far as the lens cares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PieceClass {
    Wall,
    Door,
    Window,
    Other,
}

/// One tile kind the solver knows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LensKind {
    pub id: u16,
    /// How many floor tiles of the building are this kind.
    pub count: u32,
    pub underlay: u32,
    pub overlay: u32,
    /// North, east, south, west.
    pub edges: [LensEdge; 4],
    /// The kinds seen beside this one, for north, east, south and west.
    pub beside: [Vec<u16>; 4],
}

/// What the solver cannot see, counted.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LensIgnored {
    pub furniture: u32,
    pub decorations: u32,
    pub roof: u32,
    pub windows: u32,
    pub other: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lens {
    pub width: u16,
    pub height: u16,
    pub plane: u8,
    /// The kind of each tile (row by row from the south), or none where there is no floor.
    pub cells: Vec<Option<u16>>,
    pub kinds: Vec<LensKind>,
    pub catalog: LearnedCatalog,
    pub ignored: LensIgnored,
    pub floors: u32,
    pub doors: u32,
    pub walls: u32,
    /// The prefab is a plain rectangle of map: every tile counts as floor.
    pub whole_rectangle: bool,
}

mod look;
pub use look::look;

impl Lens {
    /// Plain-language findings about what the solver has to work with.
    pub fn findings(&self) -> Vec<String> {
        let mut out = Vec::new();
        let once = self.kinds.iter().filter(|k| k.count == 1).count();
        out.push(format!(
            "It found {} kinds of floor tile in {} tiles.",
            self.kinds.len(),
            self.floors
        ));
        if self.whole_rectangle {
            out.push("This is a plain rectangle of map, not a captured building, so every tile counts as floor and walls are only read from objects.".into());
        }
        if once * 2 > self.kinds.len() && self.kinds.len() > 2 {
            out.push(format!("{once} of them appear only once. Kinds seen once can only go where they were seen, so new layouts will look much like this one."));
        }
        if self.kinds.len() <= 2 {
            out.push("There are so few kinds that it will make very plain floors. Capture buildings with more variety, or add more buildings.".into());
        }
        if self.doors == 0 {
            out.push("It found no doors, so it cannot tell where the way in is.".into());
        }
        if self.ignored.windows > 0 {
            out.push(format!(
                "{} windows look exactly like plain walls to it.",
                self.ignored.windows
            ));
        }
        let stuck = self
            .kinds
            .iter()
            .filter(|k| k.beside.iter().any(Vec::is_empty))
            .count();
        if stuck > 0 {
            out.push(format!("{stuck} kinds were never seen with a neighbour on some side (they sit on the edge of the building), so they can only be placed at the edge of a new floor."));
        }
        out
    }

    fn windows_seen(&self) -> BTreeSet<[u16; 4]> {
        let (w, h) = (usize::from(self.width), usize::from(self.height));
        let mut seen = BTreeSet::new();
        for y in 0..h.saturating_sub(1) {
            for x in 0..w.saturating_sub(1) {
                if let (Some(a), Some(b), Some(c), Some(d)) = (
                    self.cells[y * w + x],
                    self.cells[y * w + x + 1],
                    self.cells[(y + 1) * w + x],
                    self.cells[(y + 1) * w + x + 1],
                ) {
                    seen.insert([a, b, c, d]);
                }
            }
        }
        seen
    }
}

/// A solve to try: a rectangle of the building and what the solver is told in advance.
#[derive(Debug, Clone)]
pub struct Task {
    /// The south-west tile of the rectangle in the building.
    pub x0: u16,
    pub y0: u16,
    pub width: u16,
    pub height: u16,
    pub constraints: Vec<WfcConstraint>,
    /// The kinds the building really has in the rectangle (empty for a free solve).
    pub original: Vec<u16>,
}

impl Task {
    /// A solve with nothing decided beforehand.
    pub fn free(width: u16, height: u16) -> Self {
        Self {
            x0: 0,
            y0: 0,
            width,
            height,
            constraints: Vec::new(),
            original: Vec::new(),
        }
    }
}

/// "Floor it again": the largest all-floor rectangle of the building, with every tile told which
/// walls and doors stand on its edges, so the solver only chooses what the floor is made of.
pub fn refloor_task(lens: &Lens) -> Option<Task> {
    let (w, h) = (usize::from(lens.width), usize::from(lens.height));
    let mut best: Option<(usize, usize, usize, usize)> = None; // x0, y0, width, height
    for y0 in 0..h {
        for x0 in 0..w {
            let mut max_width = w - x0;
            for y1 in y0..h {
                let run = (x0..x0 + max_width)
                    .take_while(|x| lens.cells[y1 * w + x].is_some())
                    .count();
                max_width = max_width.min(run);
                if max_width == 0 {
                    break;
                }
                let area = max_width * (y1 - y0 + 1);
                if best.is_none_or(|(_, _, bw, bh)| area > bw * bh) {
                    best = Some((x0, y0, max_width, y1 - y0 + 1));
                }
            }
        }
    }
    let (x0, y0, width, height) = best?;
    let mut constraints = Vec::new();
    let mut original = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let kind = lens.cells[(y0 + y) * w + x0 + x]?;
            original.push(kind);
            let edges = lens.kinds[usize::from(kind)].edges;
            let allowed = lens
                .kinds
                .iter()
                .filter(|k| k.edges == edges)
                .filter_map(|k| WfcPatternId::new(k.id))
                .collect::<Vec<_>>();
            constraints.push(WfcConstraint {
                coord: WfcCoord::new(x as u16, y as u16),
                allowed: PatternDomain::from_patterns(allowed),
            });
        }
    }
    Some(Task {
        x0: x0 as u16,
        y0: y0 as u16,
        width: width as u16,
        height: height as u16,
        constraints,
        original,
    })
}

/// One floor the solver produced for a seed.
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    pub seed: u64,
    /// Row by row from the south, or why it failed.
    pub result: Result<Vec<u16>, String>,
    pub backtracks: u32,
    /// Of the 2x2 blocks of tiles in the result, the share that also occur in the building.
    pub familiar: f32,
    /// How many different kinds the result uses.
    pub kinds_used: usize,
    /// How many tiles ended up a different kind from the building's own (for a re-floor).
    pub changed: usize,
}

/// Run the task once per seed.
pub fn sample(lens: &Lens, task: &Task, seeds: &[u64]) -> Vec<Sample> {
    let Ok(catalog) = lens.catalog.compile() else {
        return Vec::new();
    };
    let known = lens.windows_seen();
    seeds
        .iter()
        .map(|&seed| {
            match solve_wfc(
                &catalog,
                task.width,
                task.height,
                seed,
                &task.constraints,
                64,
            ) {
                Ok(solution) => {
                    let cells: Vec<u16> = solution.cells().iter().map(|p| p.get()).collect();
                    let (w, h) = (usize::from(task.width), usize::from(task.height));
                    let (mut blocks, mut familiar) = (0, 0);
                    for y in 0..h.saturating_sub(1) {
                        for x in 0..w.saturating_sub(1) {
                            blocks += 1;
                            let key = [
                                cells[y * w + x],
                                cells[y * w + x + 1],
                                cells[(y + 1) * w + x],
                                cells[(y + 1) * w + x + 1],
                            ];
                            familiar += usize::from(known.contains(&key));
                        }
                    }
                    let changed = if task.original.len() == cells.len() {
                        cells
                            .iter()
                            .zip(&task.original)
                            .filter(|(a, b)| a != b)
                            .count()
                    } else {
                        0
                    };
                    Sample {
                        seed,
                        backtracks: solution.backtracks(),
                        familiar: if blocks == 0 {
                            1.0
                        } else {
                            familiar as f32 / blocks as f32
                        },
                        kinds_used: cells.iter().collect::<BTreeSet<_>>().len(),
                        changed,
                        result: Ok(cells),
                    }
                }
                Err(error) => Sample {
                    seed,
                    result: Err(explain(error)),
                    backtracks: 0,
                    familiar: 0.0,
                    kinds_used: 0,
                    changed: 0,
                },
            }
        })
        .collect()
}

fn explain(error: WfcSolveError) -> String {
    match error {
        WfcSolveError::Contradiction { coord, .. } => format!(
            "it got stuck at tile ({}, {}): no kind fits all its neighbours",
            coord.x, coord.y
        ),
        WfcSolveError::ConstraintHasNoValidPatterns { coord } => format!(
            "tile ({}, {}) was told about walls no known kind has",
            coord.x, coord.y
        ),
        other => format!("{other:?}"),
    }
}

/// One step of a solve: what was fixed, and how many kinds each tile could still be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// The tile that was fixed and the kind it got, when this step fixed one.
    pub fixed: Option<(u16, u16, u16)>,
    /// How many kinds each tile could still be (1 = decided), row by row from the south.
    pub options: Vec<u16>,
    /// The tile where it got stuck, when it did.
    pub stuck: Option<(u16, u16)>,
    pub note: String,
}

/// The solve for `seed`, step by step. On a failure the steps so far come back with the reason.
pub fn trace(lens: &Lens, task: &Task, seed: u64) -> (Vec<Step>, Result<Vec<u16>, String>) {
    let Ok(catalog) = lens.catalog.compile() else {
        return (Vec::new(), Err("the catalog is not valid".into()));
    };
    let sizes = |domains: &[PatternDomain]| -> Vec<u16> {
        domains.iter().map(|d| d.len() as u16).collect()
    };
    let mut steps: Vec<Step> = Vec::new();
    let result = solve_wfc_observed(
        &catalog,
        task.width,
        task.height,
        seed,
        &task.constraints,
        64,
        &mut |event| match event {
            WfcEvent::Start { domains } => steps.push(Step {
                fixed: None,
                options: sizes(domains),
                stuck: None,
                note: if task.constraints.is_empty() {
                    "Start: every tile could be any kind.".into()
                } else {
                    "Start: each tile is limited to the kinds that have its walls and doors.".into()
                },
            }),
            WfcEvent::Collapse {
                cell,
                chosen,
                options,
                domains,
            } => steps.push(Step {
                fixed: Some((cell.x, cell.y, chosen.get())),
                options: sizes(domains),
                stuck: None,
                note: format!(
                    "Tile ({}, {}) had {options} choices left and took kind {}.",
                    cell.x,
                    cell.y,
                    chosen.get()
                ),
            }),
            WfcEvent::Contradiction { cell } => {
                let options = steps.last().map(|s| s.options.clone()).unwrap_or_default();
                steps.push(Step {
                    fixed: None,
                    options,
                    stuck: Some((cell.x, cell.y)),
                    note: format!(
                        "Stuck: tile ({}, {}) has no kind that fits all its neighbours.",
                        cell.x, cell.y
                    ),
                });
            }
            WfcEvent::Backtrack { domains } => steps.push(Step {
                fixed: None,
                options: sizes(domains),
                stuck: None,
                note: "Went back and tried a different choice.".into(),
            }),
        },
    );
    let result = result
        .map(|s| s.cells().iter().map(|p| p.get()).collect())
        .map_err(explain);
    (steps, result)
}

/// The directions of a kind's edges, with names, for display.
pub const DIRECTIONS: [(&str, CardinalDirection); 4] = [
    ("north", CardinalDirection::North),
    ("east", CardinalDirection::East),
    ("south", CardinalDirection::South),
    ("west", CardinalDirection::West),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PrefabBuilding, PrefabCell, PrefabObject, PrefabSource, PrefabTile};

    /// A 4x3 building with a floor change in the middle, walls round the outside and one door.
    fn house() -> Prefab {
        let tiles = (0..3)
            .flat_map(|y| {
                (0..4).map(move |x| PrefabTile {
                    x,
                    y,
                    plane: 0,
                    underlay: Some(5),
                    overlay: None,
                    flags: 0,
                })
            })
            .map(|mut t| {
                if t.x >= 2 {
                    t.underlay = Some(6);
                }
                t
            })
            .collect();
        let mut objects = Vec::new();
        for x in 0..4_u16 {
            objects.push(PrefabObject {
                id: 10,
                x,
                y: 0,
                plane: 0,
                kind: 0,
                orientation: 3,
            });
            objects.push(PrefabObject {
                id: 10,
                x,
                y: 2,
                plane: 0,
                kind: 0,
                orientation: 1,
            });
        }
        objects.push(PrefabObject {
            id: 20,
            x: 0,
            y: 1,
            plane: 0,
            kind: 0,
            orientation: 0,
        });
        objects.push(PrefabObject {
            id: 30,
            x: 1,
            y: 1,
            plane: 0,
            kind: 10,
            orientation: 0,
        });
        let floors = (0..3)
            .flat_map(|y| (0..4).map(move |x| PrefabCell { x, y, plane: 0 }))
            .collect();
        Prefab {
            format: crate::PREFAB_FORMAT.to_owned(),
            version: crate::PREFAB_VERSION,
            name: "house".into(),
            tags: vec![],
            notes: String::new(),
            source: PrefabSource::Authored,
            width: 4,
            height: 3,
            planes: 1,
            tiles,
            heights: vec![],
            objects,
            building: Some(PrefabBuilding {
                semantics: Default::default(),
                floors,
                support: vec![],
                source_datum: 0,
                provenance: "test".into(),
                object_sizes: Default::default(),
                relations: vec![],
                labels: Default::default(),
                reviewed: true,
            }),
        }
    }

    fn classify(id: u32, _: u8) -> PieceClass {
        match id {
            20 => PieceClass::Door,
            _ => PieceClass::Wall,
        }
    }

    #[test]
    fn the_lens_shows_kinds_doors_and_what_it_ignores_and_the_solver_runs_on_it() {
        let lens = look(&house(), 0, &classify).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(lens.floors, 12);
        assert_eq!(lens.doors, 1);
        assert_eq!(lens.ignored.furniture, 1);
        assert!(lens.kinds.iter().any(|k| k.edges.contains(&LensEdge::Door)));
        assert!(lens.kinds.len() >= 4, "{:?}", lens.kinds.len());
        let again = look(&house(), 0, &classify).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(lens, again, "the view is deterministic");
        assert!(
            lens.findings()
                .iter()
                .any(|f| f.contains("kinds of floor tile"))
        );
        let task = refloor_task(&lens).unwrap_or_else(|| panic!("a rectangle"));
        assert_eq!((task.width, task.height), (4, 3));
        let samples = sample(&lens, &task, &[1, 2, 3, 4]);
        assert!(
            samples.iter().all(|s| s.result.is_ok()),
            "told its own walls, the solver can floor it again: {samples:?}"
        );
        let (steps, result) = trace(&lens, &task, 1);
        assert!(!steps.is_empty());
        assert_eq!(steps[0].options.len(), 12);
        assert_eq!(
            trace(&lens, &task, 1).1,
            result,
            "the same seed gives the same floor"
        );
        // Without being told the walls, a floor made from one building cannot be solved freely.
        let free = sample(&lens, &Task::free(6, 6), &[1, 2]);
        assert!(free.iter().all(|s| s.result.is_err()), "{free:?}");
    }

    #[test]
    fn a_building_without_floor_is_reported() {
        let mut empty = house();
        if let Some(b) = empty.building.as_mut() {
            b.floors.clear();
        }
        assert!(look(&empty, 0, &classify).is_err());
    }
}
