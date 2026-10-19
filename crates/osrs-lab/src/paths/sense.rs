//! What the learner sees of a place: for every ground tile, a handful of numbers about its
//! surroundings (is it blocked, how close are doors and buildings, how steep, what stands
//! nearby). The paths themselves are never part of it: the canvas is blank.
//!
//! Everything here is computed from the ground, heights and objects alone, so the same numbers
//! come out of a real map region, an editor selection and a hand-made test grid.

use super::grid::{count_within, distance_to};
use super::lesson::{ObjectEntry, PathLesson};
use osrs_core::ids::ObjectId;
use osrs_scene::building_census::{
    CensusArea, EdgeFeature, Edges, Ground, ObjectInfo, ObjectTable, analyze,
};
use osrs_scene::object_semantics::Placed;
use std::collections::{BTreeSet, HashMap};

pub const FEATURES: usize = 26;

/// A thing the learner can see, and what it means for a path that it matters.
pub struct FeatureInfo {
    pub key: &'static str,
    /// Said when paths are drawn more where this is high.
    pub toward: &'static str,
    /// Said when paths are drawn less where this is high.
    pub away: &'static str,
}

pub const FEATURE_INFO: [FeatureInfo; FEATURES] = [
    FeatureInfo {
        key: "blocked",
        toward: "go over blocked ground",
        away: "stay off blocked ground (water, trees, rocks, solid objects)",
    },
    FeatureInfo {
        key: "next_to_blocked",
        toward: "hug blocked things",
        away: "keep a tile away from blocked things",
    },
    FeatureInfo {
        key: "inside_building",
        toward: "run through buildings",
        away: "never run through buildings",
    },
    FeatureInfo {
        key: "near_building",
        toward: "stay close to buildings",
        away: "keep away from buildings",
    },
    FeatureInfo {
        key: "near_door",
        toward: "lead to doors",
        away: "stay away from doors",
    },
    FeatureInfo {
        key: "at_door",
        toward: "start right outside a door",
        away: "leave the spot outside a door bare",
    },
    FeatureInfo {
        key: "free_wall_near",
        toward: "run along fences and free-standing walls",
        away: "keep away from fences and walls",
    },
    FeatureInfo {
        key: "steep",
        toward: "climb steep ground",
        away: "avoid steep ground",
    },
    FeatureInfo {
        key: "high_spot",
        toward: "run along the high ground",
        away: "avoid bumps and ridges",
    },
    FeatureInfo {
        key: "map_edge",
        toward: "come in from the edge of the area",
        away: "stay away from the edge of the area",
    },
    FeatureInfo {
        key: "trees_near",
        toward: "wind through trees",
        away: "keep clear of trees",
    },
    FeatureInfo {
        key: "rocks_near",
        toward: "pass close to rocks",
        away: "keep clear of rocks",
    },
    FeatureInfo {
        key: "lamps_near",
        toward: "follow lamps, signs and posts",
        away: "keep away from lamps and signs",
    },
    FeatureInfo {
        key: "clutter_near",
        toward: "go through market clutter (crates, stalls, carts)",
        away: "avoid crates, stalls and carts",
    },
    FeatureInfo {
        key: "narrow",
        toward: "follow narrow gaps between things",
        away: "prefer open ground to narrow gaps",
    },
    FeatureInfo {
        key: "other_floor_near",
        toward: "run beside other floor types (plazas, bridges, shores)",
        away: "keep away from other floor types",
    },
    FeatureInfo {
        key: "plants_near",
        toward: "run beside bushes and flowers",
        away: "keep clear of bushes and flowers",
    },
    FeatureInfo {
        key: "flat_open",
        toward: "cross open flat ground",
        away: "avoid open flat ground",
    },
    FeatureInfo {
        key: "wide_open",
        toward: "cross wide open areas",
        away: "avoid wide open areas",
    },
    FeatureInfo {
        key: "street",
        toward: "run along streets between buildings",
        away: "avoid the gaps between buildings",
    },
    FeatureInfo {
        key: "building_zone",
        toward: "stay within a short walk of buildings",
        away: "keep well away from buildings",
    },
    FeatureInfo {
        key: "floor_edge_far",
        toward: "follow the edges of plazas, shores and other floors",
        away: "keep away from plazas and shores",
    },
    FeatureInfo {
        key: "rough",
        toward: "cross rough ground",
        away: "keep to smooth ground",
    },
    FeatureInfo {
        key: "hillside",
        toward: "climb hills",
        away: "stay off hillsides",
    },
    FeatureInfo {
        key: "crowded",
        toward: "go where it is crowded with obstacles",
        away: "avoid crowded ground",
    },
    FeatureInfo {
        key: "edge_far",
        toward: "come in from the edge of the area",
        away: "stay away from the edge of the area",
    },
];

const TREE: &[&str] = &["tree", "stump", "log"];
const ROCK: &[&str] = &["rock", "boulder", "stone", "pillar"];
const LAMP: &[&str] = &[
    "lamp", "lantern", "sign", "post", "torch", "street", "light", "banner",
];
const CLUTTER: &[&str] = &[
    "crate", "barrel", "stall", "cart", "sack", "bench", "table", "chest", "bucket", "wagon",
];
const PLANT: &[&str] = &[
    "bush", "flower", "plant", "fern", "shrub", "grass", "weed", "hedge", "vine",
];

fn says(entry: Option<&ObjectEntry>, words: &[&str]) -> bool {
    entry.is_some_and(|e| {
        words
            .iter()
            .any(|w| e.name.contains(w) || e.symbol.contains(w))
    })
}

fn object_table(lesson: &PathLesson) -> ObjectTable {
    lesson
        .names
        .iter()
        .map(|(id, e)| {
            (
                ObjectId::new(*id),
                ObjectInfo {
                    name: e.name.clone(),
                    symbol: e.symbol.clone(),
                    actions: e.actions.clone(),
                    size: (e.size[0], e.size[1]),
                },
            )
        })
        .collect()
}

/// A lesson read as a place: the numbers per tile, which tiles can be walked, where the doors
/// are, and (for checking, never for learning) which tiles are path.
pub struct Scene {
    pub width: usize,
    pub height: usize,
    pub features: Vec<[f32; FEATURES]>,
    /// Open to walk on: not blocked, not inside a building.
    pub walkable: Vec<bool>,
    pub inside: Vec<bool>,
    /// Tiles where a path could be drawn: not in a building and not already another kind of
    /// floor. These are the only tiles the learner decides about and is checked on.
    pub candidate: Vec<bool>,
    /// The tile just outside each exterior door.
    pub door_fronts: Vec<(usize, usize)>,
    /// The real paths. Only the checks and the trainer's labels read this.
    pub mask: Vec<bool>,
}

impl Scene {
    pub fn of(lesson: &PathLesson) -> Self {
        let (w, h) = (usize::from(lesson.width), usize::from(lesson.height));
        let objects = object_table(lesson);
        let placed: Vec<Placed> = lesson
            .objects
            .iter()
            .map(|o| Placed {
                id: ObjectId::new(o[0]),
                kind: o[1] as u8,
                orientation: o[2] as u8,
                tile: (o[3] as i32, o[4] as i32),
                plane: 0,
            })
            .collect();
        // Buildings: the census over the ground plane.
        let mut area = CensusArea::new((0, 0), w, h);
        for y in 0..h {
            for x in 0..w {
                let at = lesson.index(x, y);
                area.set(
                    0,
                    (x as i32, y as i32),
                    Ground {
                        overlay: 0,
                        shape: 0,
                        settings: lesson.flags[at],
                        height: lesson.heights[at],
                        height_known: true,
                        underlay: 0,
                    },
                );
            }
        }
        let buildings = analyze(&area, &placed, &objects);
        let mut inside = vec![false; w * h];
        let mut door_fronts = Vec::new();
        for building in &buildings {
            for (x, y) in building.tiles() {
                inside[y as usize * w + x as usize] = true;
            }
        }
        for building in &buildings {
            for wall in building.walls.iter().filter(|wall| wall.exterior) {
                if !matches!(wall.piece.feature, EdgeFeature::Door | EdgeFeature::Opening) {
                    continue;
                }
                let near = wall.dir.from(wall.tile);
                let outside = [wall.tile, near].into_iter().find(|t| {
                    t.0 >= 0
                        && t.1 >= 0
                        && (t.0 as usize) < w
                        && (t.1 as usize) < h
                        && !inside[t.1 as usize * w + t.0 as usize]
                });
                if let Some(t) = outside {
                    door_fronts.push((t.0 as usize, t.1 as usize));
                }
            }
        }
        door_fronts.sort_unstable();
        door_fronts.dedup();
        let inside_set: BTreeSet<usize> = (0..w * h).filter(|a| inside[*a]).collect();
        let path_set = lesson.path_overlay_set(&inside_set);
        let mask = lesson.path_mask(&inside_set);
        let mut solid: Vec<bool> = lesson.flags.iter().map(|f| f & 1 != 0).collect();
        let mut tree = vec![false; w * h];
        let mut rock = vec![false; w * h];
        let mut lamp = vec![false; w * h];
        let mut clutter = vec![false; w * h];
        let mut plant = vec![false; w * h];
        for o in &lesson.objects {
            if o[3] as usize >= w || o[4] as usize >= h {
                continue;
            }
            let entry = lesson.names.get(&o[0]);
            let kind = o[1];
            let (sx, sy) = entry.map_or((1, 1), |e| {
                if o[2] & 1 == 1 {
                    (e.size[1], e.size[0])
                } else {
                    (e.size[0], e.size[1])
                }
            });
            let blocks = entry.and_then(|e| e.blocks).unwrap_or_else(|| {
                (kind == 10 || kind == 11)
                    && entry.is_some_and(|e| !e.name.is_empty())
                    && !says(entry, PLANT)
            });
            let named = |marks: &mut Vec<bool>| {
                for dy in 0..usize::from(sy.max(1)) {
                    for dx in 0..usize::from(sx.max(1)) {
                        let (x, y) = (o[3] as usize + dx, o[4] as usize + dy);
                        if x < w && y < h {
                            marks[y * w + x] = true;
                        }
                    }
                }
            };
            if kind == 10 || kind == 11 {
                if blocks {
                    named(&mut solid);
                }
                if says(entry, TREE) {
                    named(&mut tree);
                } else if says(entry, ROCK) {
                    named(&mut rock);
                } else if says(entry, CLUTTER) {
                    named(&mut clutter);
                }
            }
            if says(entry, LAMP) {
                named(&mut lamp);
            }
            if says(entry, PLANT) {
                named(&mut plant);
            }
        }
        // Fences and walls that belong to no building.
        let edges = Edges::of_plane(&placed, 0, &objects);
        let mut free_wall = vec![false; w * h];
        for (tile, dir, piece) in edges.pieces() {
            if piece.feature == EdgeFeature::Opening {
                continue;
            }
            let other = dir.from(tile);
            let here =
                |t: (i32, i32)| t.0 >= 0 && t.1 >= 0 && (t.0 as usize) < w && (t.1 as usize) < h;
            let in_building = |t: (i32, i32)| here(t) && inside[t.1 as usize * w + t.0 as usize];
            if !in_building(tile) && !in_building(other) {
                for t in [tile, other] {
                    if here(t) {
                        free_wall[t.1 as usize * w + t.0 as usize] = true;
                    }
                }
            }
        }
        let other_floor: Vec<bool> = (0..w * h)
            .map(|a| lesson.overlay[a] != 0 && !path_set.contains(&lesson.overlay[a]) && !inside[a])
            .collect();
        let blocked_for_walk: Vec<bool> = (0..w * h).map(|a| solid[a] || inside[a]).collect();
        let d_solid = distance_to(&solid, w, h, 6);
        let d_building = distance_to(&inside, w, h, 8);
        let fronts: Vec<bool> = {
            let mut marks = vec![false; w * h];
            for (x, y) in &door_fronts {
                marks[y * w + x] = true;
            }
            marks
        };
        let d_door = distance_to(&fronts, w, h, 14);
        let d_other = distance_to(&other_floor, w, h, 12);
        let d_building_far = distance_to(&inside, w, h, 16);
        let n_blocked_9 = count_within(&blocked_for_walk, w, h, 4);
        let street_at = |x: usize, y: usize| -> bool {
            let along = |dx: i32, dy: i32| {
                (1..=7).any(|k| {
                    let (xx, yy) = (x as i32 + dx * k, y as i32 + dy * k);
                    xx >= 0
                        && yy >= 0
                        && (xx as usize) < w
                        && (yy as usize) < h
                        && inside[yy as usize * w + xx as usize]
                })
            };
            (along(-1, 0) && along(1, 0)) || (along(0, -1) && along(0, 1))
        };
        let n_tree = count_within(&tree, w, h, 2);
        let n_rock = count_within(&rock, w, h, 2);
        let n_lamp = count_within(&lamp, w, h, 3);
        let n_clutter = count_within(&clutter, w, h, 2);
        let n_plant = count_within(&plant, w, h, 2);
        let n_wall = count_within(&free_wall, w, h, 1);
        let n_blocked_5 = count_within(&blocked_for_walk, w, h, 2);
        let run = |horizontal: bool| -> Vec<u8> {
            let mut out = vec![0_u8; w * h];
            let (outer, inner) = if horizontal { (h, w) } else { (w, h) };
            for a in 0..outer {
                let mut b = 0;
                while b < inner {
                    let at = |b: usize| if horizontal { a * w + b } else { b * w + a };
                    if blocked_for_walk[at(b)] {
                        b += 1;
                        continue;
                    }
                    let start = b;
                    while b < inner && !blocked_for_walk[at(b)] {
                        b += 1;
                    }
                    let length = (b - start).min(10) as u8;
                    for k in start..b {
                        out[at(k)] = length;
                    }
                }
            }
            out
        };
        let (run_x, run_y) = (run(true), run(false));
        let height_at = |x: usize, y: usize| lesson.heights[y * w + x];
        let near = |d: u8, cap: u8| {
            if d >= cap {
                0.0
            } else {
                1.0 / (1.0 + f32::from(d))
            }
        };
        let mut features = vec![[0.0_f32; FEATURES]; w * h];
        for y in 0..h {
            for x in 0..w {
                let a = y * w + x;
                let here = height_at(x, y);
                let mut step = 0;
                let mut sum = 0_i64;
                let mut count = 0_i64;
                for (nx, ny) in [
                    (x.wrapping_sub(1), y),
                    (x + 1, y),
                    (x, y.wrapping_sub(1)),
                    (x, y + 1),
                ] {
                    if nx < w && ny < h {
                        step = step.max((height_at(nx, ny) - here).abs());
                    }
                }
                for yy in y.saturating_sub(3)..(y + 4).min(h) {
                    for xx in x.saturating_sub(3)..(x + 4).min(w) {
                        sum += i64::from(height_at(xx, yy));
                        count += 1;
                    }
                }
                let mean = (sum / count.max(1)) as i32;
                let (mut low, mut high) = (i32::MAX, i32::MIN);
                for yy in y.saturating_sub(2)..(y + 3).min(h) {
                    for xx in x.saturating_sub(2)..(x + 3).min(w) {
                        low = low.min(height_at(xx, yy));
                        high = high.max(height_at(xx, yy));
                    }
                }
                let range5 = high - low;
                let spread = {
                    let mut acc = 0_i64;
                    for yy in y.saturating_sub(3)..(y + 4).min(h) {
                        for xx in x.saturating_sub(3)..(x + 4).min(w) {
                            acc += i64::from((height_at(xx, yy) - mean).abs());
                        }
                    }
                    (acc / count.max(1)) as f32
                };
                let edge = x.min(y).min(w - 1 - x).min(h - 1 - y).min(250) as u8;
                let open_run = f32::from(run_x[a].min(run_y[a])) / 10.0;
                features[a] = [
                    f32::from(u8::from(solid[a])),
                    near(d_solid[a], 4),
                    f32::from(u8::from(inside[a])),
                    near(d_building[a], 6),
                    near(d_door[a], 12),
                    f32::from(u8::from(d_door[a] == 0)),
                    (f32::from(n_wall[a]) / 4.0).min(1.0),
                    (step as f32 / 256.0).min(1.0),
                    ((here - mean) as f32 / 256.0).clamp(-1.0, 1.0),
                    near(edge, 7),
                    (f32::from(n_tree[a]) / 4.0).min(1.0),
                    (f32::from(n_rock[a]) / 4.0).min(1.0),
                    (f32::from(n_lamp[a]) / 3.0).min(1.0),
                    (f32::from(n_clutter[a]) / 4.0).min(1.0),
                    if blocked_for_walk[a] {
                        0.0
                    } else {
                        1.0 - open_run
                    },
                    near(d_other[a].min(4), 4),
                    (f32::from(n_plant[a]) / 4.0).min(1.0),
                    if n_blocked_5[a] == 0 && step < 64 {
                        1.0
                    } else {
                        0.0
                    },
                    1.0 - f32::from(n_blocked_9[a]) / 81.0,
                    f32::from(u8::from(street_at(x, y))),
                    near(d_building_far[a], 16),
                    near(d_other[a], 12),
                    (spread / 128.0).min(1.0),
                    (range5 as f32 / 512.0).min(1.0),
                    (f32::from(n_blocked_9[a]) / 40.0).min(1.0),
                    near(edge.min(16), 17),
                ];
            }
        }
        let walkable = blocked_for_walk.iter().map(|b| !*b).collect();
        let candidate = (0..w * h).map(|a| !inside[a] && !other_floor[a]).collect();
        Self {
            candidate,
            width: w,
            height: h,
            features,
            walkable,
            inside,
            door_fronts,
            mask,
        }
    }

    pub fn tiles(&self) -> usize {
        self.width * self.height
    }

    /// Every tile's chance, in a hash map keyed by index, for tests.
    #[cfg(test)]
    pub fn count(&self, feature: usize) -> usize {
        self.features.iter().filter(|f| f[feature] > 0.0).count()
    }
}

/// Entries for the objects of a lesson built from the editor: name, size and what the editor's
/// catalog knows.
#[allow(dead_code)]
pub fn entries_from(
    ids: impl Iterator<Item = u32>,
    lookup: impl Fn(u32) -> Option<ObjectEntry>,
) -> std::collections::BTreeMap<u32, ObjectEntry> {
    let mut seen: HashMap<u32, ()> = HashMap::new();
    ids.filter(|id| seen.insert(*id, ()).is_none())
        .filter_map(|id| lookup(id).map(|e| (id, e)))
        .collect()
}
