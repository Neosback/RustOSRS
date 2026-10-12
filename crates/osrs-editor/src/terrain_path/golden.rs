//! The planner against the reference: `golden.txt` is what Terraini's own planner (its jar, run on
//! these exact inputs) makes, tile by tile, and the port must make the same.

use super::model::*;
use super::planner::{PathPlanner, stroke_network};
use super::settings::{MaterialVariant, PathProfile, PathSettings};
use std::collections::BTreeMap;
use std::sync::Arc;

const GOLDEN: &str = include_str!("golden.txt");

type Expected = (Vec<String>, Vec<String>);

fn parse() -> BTreeMap<String, Expected> {
    let mut cases = BTreeMap::new();
    let mut name = String::new();
    let mut tiles = Vec::new();
    let mut warnings = Vec::new();
    for line in GOLDEN.lines() {
        if let Some(n) = line.strip_prefix("CASE ") {
            name = n.to_owned();
            tiles = Vec::new();
            warnings = Vec::new();
        } else if line == "END" {
            cases.insert(name.clone(), (tiles.clone(), warnings.clone()));
        } else if let Some(t) = line.strip_prefix("T ") {
            tiles.push(t.to_owned());
        } else if let Some(w) = line.strip_prefix("W ") {
            warnings.push(w.to_owned());
        }
    }
    cases
}

fn render(plan: &PathPlan) -> Expected {
    let tiles = plan
        .paints
        .iter()
        .map(|p| {
            format!(
                "{} {} {} {} {} {} {:.6} {}",
                p.tile_x,
                p.tile_y,
                p.plane,
                p.overlay_id,
                p.shape,
                p.orientation,
                p.coverage,
                p.profile_id
            )
        })
        .collect();
    let warnings = plan
        .warnings
        .iter()
        .map(|w| {
            let severity = match w.severity {
                Severity::Info => "INFO",
                Severity::Warning => "WARNING",
                Severity::Error => "ERROR",
            };
            let at = w.tile.map_or("null null null".to_owned(), |(x, y, z)| {
                format!("{x} {y} {z}")
            });
            format!("{severity} {} {at}", w.code)
        })
        .collect();
    (tiles, warnings)
}

fn pts(values: &[f64]) -> Vec<PathPoint> {
    values
        .chunks(2)
        .map(|c| PathPoint::new(c[0], c[1]))
        .collect()
}

struct Blocks {
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
}

impl CostGrid for Blocks {
    fn occupancy(&self, x: i32, y: i32, _plane: u8) -> Occupancy {
        if x >= self.x0 && x <= self.x1 && y >= self.y0 && y <= self.y1 {
            Occupancy::Solid
        } else {
            Occupancy::Clear
        }
    }
}

fn run(
    points: &[f64],
    profile: &PathProfile,
    settings: &PathSettings,
    env: &PathEnvironment,
    edge_id: u64,
) -> PathPlan {
    let points = pts(points);
    let network = stroke_network(
        &points,
        0,
        profile,
        settings.seed,
        [3000 + edge_id, 1000 + edge_id, 2000 + edge_id, edge_id],
    );
    PathPlanner.plan(&network, settings, env)
}

fn check(name: &str, plan: &PathPlan, golden: &BTreeMap<String, Expected>) {
    let (tiles, warnings) = render(plan);
    let (want_tiles, want_warnings) = &golden[name];
    assert_eq!(tiles.len(), want_tiles.len(), "{name}: tile count");
    for (i, (got, want)) in tiles.iter().zip(want_tiles).enumerate() {
        assert_eq!(got, want, "{name}: tile {i}");
    }
    assert_eq!(&warnings, want_warnings, "{name}: warnings");
}

#[test]
fn the_port_plans_what_the_reference_plans() {
    let golden = parse();
    let bounds = TileBounds::new(0, 0, 200, 200);
    let open = PathEnvironment::empty(bounds);
    let profile = PathProfile::new("default", "Default path", 5).sanitized();
    let default = PathSettings::default();
    let calm = PathSettings {
        noise_strength: 0.0,
        ..default
    };
    let seeded = PathSettings {
        seed: 12345,
        ..default
    };
    let curve = [20.0, 20.0, 40.0, 25.0, 55.0, 45.0, 70.0, 48.0, 90.0, 30.0];

    check(
        "straight_calm",
        &run(&[20.5, 20.5, 60.5, 20.5], &profile, &calm, &open, 1),
        &golden,
    );
    check(
        "diagonal_calm",
        &run(&[20.5, 20.5, 50.5, 44.5], &profile, &calm, &open, 2),
        &golden,
    );
    check(
        "curve_calm",
        &run(&curve, &profile, &calm, &open, 3),
        &golden,
    );
    check(
        "curve_default",
        &run(&curve, &profile, &default, &open, 4),
        &golden,
    );
    check(
        "curve_seeded",
        &run(&curve, &profile, &seeded, &open, 5),
        &golden,
    );

    let wide = PathProfile {
        id: "wide".to_owned(),
        name: "Wide".to_owned(),
        primary_overlay_id: 7,
        base_width: 5.0,
        edge_noise: 0.4,
        width_variation: 0.3,
        ..PathProfile::default()
    }
    .sanitized();
    check(
        "wide_noisy",
        &run(
            &[30.0, 30.0, 50.0, 60.0, 80.0, 70.0],
            &wide,
            &seeded,
            &open,
            6,
        ),
        &golden,
    );

    let mixed = PathProfile {
        id: "mixed".to_owned(),
        name: "Mixed".to_owned(),
        primary_overlay_id: 3,
        material_variants: vec![
            MaterialVariant {
                overlay_id: 3,
                weight: 2.0,
            },
            MaterialVariant {
                overlay_id: 4,
                weight: 1.0,
            },
            MaterialVariant {
                overlay_id: 9,
                weight: 1.0,
            },
        ],
        material_patch_length: 5.0,
        ..PathProfile::default()
    }
    .sanitized();
    check(
        "mixed_materials",
        &run(&[20.0, 100.0, 120.0, 100.0], &mixed, &seeded, &open, 7),
        &golden,
    );

    let walled = PathEnvironment {
        bounds,
        cost_grid: Arc::new(Blocks {
            x0: 50,
            y0: 15,
            x1: 52,
            y1: 30,
        }),
        existing_paints: Vec::new(),
    };
    check(
        "reroute",
        &run(&[20.5, 22.5, 80.5, 22.5], &profile, &default, &walled, 8),
        &golden,
    );

    let existing: Vec<TilePaint> = (40..60)
        .map(|x| TilePaint {
            tile_x: x,
            tile_y: 80,
            plane: 0,
            overlay_id: 5,
            shape: 0,
            orientation: 0,
            coverage: 1.0,
            profile_id: "existing".to_owned(),
        })
        .collect();
    let joined = PathEnvironment {
        bounds,
        cost_grid: Arc::new(OpenGround),
        existing_paints: existing,
    };
    check(
        "autoconnect",
        &run(&[20.5, 70.5, 38.5, 78.5], &profile, &default, &joined, 9),
        &golden,
    );
    check(
        "preserve",
        &run(&[30.5, 80.5, 70.5, 80.5], &profile, &calm, &joined, 10),
        &golden,
    );

    // Three edges meeting at one node.
    let nodes = vec![
        PathNode::new(1, 100.0, 100.0, 0),
        PathNode::new(2, 70.0, 100.0, 0),
        PathNode::new(3, 130.0, 100.0, 0),
        PathNode::new(4, 100.0, 130.0, 0),
    ];
    let edges = vec![
        PathEdge::new(3, 1, 2, Vec::new(), "default"),
        PathEdge::new(0, 1, 3, Vec::new(), "default"),
        PathEdge::new(1, 1, 4, Vec::new(), "default"),
    ];
    let network = PathNetwork {
        id: 99,
        name: "Junction".to_owned(),
        seed: 99,
        nodes,
        edges,
        profiles: vec![profile.clone()],
    };
    check(
        "junction",
        &PathPlanner.plan(&network, &default, &open),
        &golden,
    );
}

const FUZZ: &str = include_str!("fuzz.txt");

struct Slopes {
    rect: Option<(i32, i32, i32, i32, bool)>,
}

impl CostGrid for Slopes {
    fn occupancy(&self, x: i32, y: i32, _plane: u8) -> Occupancy {
        match self.rect {
            Some((x0, y0, x1, y1, all)) if x >= x0 && x <= x1 && y >= y0 && y <= y1 => {
                if all {
                    Occupancy::Decoration
                } else {
                    Occupancy::Solid
                }
            }
            _ => Occupancy::Clear,
        }
    }

    fn slope(&self, x: i32, y: i32, _plane: u8) -> f64 {
        if (x + y) % 17 == 0 { 0.5 } else { 0.0 }
    }
}

fn numbers(line: &str) -> Vec<f64> {
    line.split_whitespace()
        .filter_map(|t| t.parse().ok())
        .collect()
}

fn fnv_lines(plan: &PathPlan) -> u64 {
    let mut hash = 0xCBF2_9CE4_8422_2325_u64;
    for p in &plan.paints {
        let line = format!(
            "{} {} {} {} {} {} {:.6} {}\n",
            p.tile_x,
            p.tile_y,
            p.plane,
            p.overlay_id,
            p.shape,
            p.orientation,
            p.coverage,
            p.profile_id
        );
        for byte in line.bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0100_0000_01B3);
        }
    }
    hash
}

/// Forty random paths, profiles, settings and grounds: the port agrees with the reference on all
/// of them (38 thousand tiles between them).
#[test]
fn the_port_agrees_with_the_reference_on_random_paths() {
    use super::settings::ObstaclePolicy;
    let mut cases = 0;
    let mut lines = FUZZ.lines().peekable();
    while let Some(line) = lines.next() {
        if !line.starts_with("CASE ") {
            continue;
        }
        let case: u64 = line[5..].parse().unwrap_or(0);
        let mut profile = PathProfile::default();
        let mut settings = PathSettings::default();
        let mut points = Vec::new();
        let mut env_rect = None;
        let mut existing = Vec::new();
        let (mut count, mut hash) = (0_usize, 0_u64);
        let mut warnings: Vec<String> = Vec::new();
        for line in lines.by_ref() {
            if line == "END" {
                break;
            }
            let (tag, rest) = line.split_at(1);
            let rest = rest.trim();
            match tag {
                "P" => {
                    let fields: Vec<&str> = rest.split_whitespace().collect();
                    let n = |i: usize| fields[i].parse::<f64>().unwrap_or(0.0);
                    profile = PathProfile {
                        id: "p".to_owned(),
                        name: "P".to_owned(),
                        primary_overlay_id: n(0) as u16,
                        base_width: n(1),
                        edge_falloff: n(2),
                        centerline_wander: n(3),
                        wander_scale: n(4),
                        width_variation: n(5),
                        width_noise_scale: n(6),
                        edge_noise: n(7),
                        edge_noise_scale: n(8),
                        noise_octaves: n(9) as u32,
                        obstacle_clearance: n(10),
                        reroute: fields[11] == "true",
                        auto_connect: fields[12] == "true",
                        ..PathProfile::default()
                    };
                }
                "M" => {
                    profile.material_variants = rest
                        .split_whitespace()
                        .filter_map(|t| {
                            let (id, weight) = t.split_once(',')?;
                            Some(MaterialVariant {
                                overlay_id: id.parse().ok()?,
                                weight: weight.parse().ok()?,
                            })
                        })
                        .collect();
                }
                "S" => {
                    let f: Vec<&str> = rest.split_whitespace().collect();
                    let n = |i: usize| f[i].parse::<f64>().unwrap_or(0.0);
                    settings = PathSettings {
                        seed: f[0].parse().unwrap_or(0),
                        smoothness: n(1),
                        noise_strength: n(2),
                        noise_scale: n(3),
                        clearance: n(4),
                        supersample: f[5].parse().unwrap_or(8),
                        obstacle_policy: match f[6] {
                            "AVOID_SOLIDS" => ObstaclePolicy::AvoidSolids,
                            "AVOID_ALL" => ObstaclePolicy::AvoidAll,
                            "MASK" => ObstaclePolicy::Mask,
                            "WARN_ONLY" => ObstaclePolicy::WarnOnly,
                            _ => ObstaclePolicy::Ignore,
                        },
                        maximum_slope: if f[7] == "inf" { f64::MAX } else { 0.3 },
                        slope_penalty: n(8),
                        route_corridor: n(9),
                        ..PathSettings::default()
                    };
                }
                "L" => {
                    points = rest
                        .split_whitespace()
                        .filter_map(|t| {
                            let (x, y) = t.split_once(',')?;
                            Some(PathPoint::new(x.parse().ok()?, y.parse().ok()?))
                        })
                        .collect();
                }
                "E" => {
                    let f: Vec<&str> = rest.split_whitespace().collect();
                    if f[0] == "true" {
                        let n = |i: usize| f[i].parse::<i32>().unwrap_or(0);
                        env_rect = Some((n(1), n(2), n(1) + n(3), n(2) + n(4), f[5] == "true"));
                    }
                }
                "X" => {
                    for token in rest.split_whitespace() {
                        let f: Vec<i32> = token.split(',').filter_map(|v| v.parse().ok()).collect();
                        existing.push(TilePaint {
                            tile_x: f[0],
                            tile_y: f[1],
                            plane: 0,
                            overlay_id: profile.primary_overlay_id,
                            shape: f[2] as u8,
                            orientation: f[3] as u8,
                            coverage: 1.0,
                            profile_id: "e".to_owned(),
                        });
                    }
                }
                "N" => {
                    let f: Vec<&str> = rest.split_whitespace().collect();
                    count = f[0].parse().unwrap_or(0);
                    hash = f[1].parse().unwrap_or(0);
                }
                "W" => warnings.push(rest.to_owned()),
                _ => {}
            }
        }
        let profile = profile.sanitized();
        let env = PathEnvironment {
            bounds: TileBounds::new(0, 0, 160, 160),
            // The reference harness only gives the ground slopes when it has blocks on it.
            cost_grid: if env_rect.is_some() {
                Arc::new(Slopes { rect: env_rect })
            } else {
                Arc::new(OpenGround)
            },
            existing_paints: existing,
        };
        let network = stroke_network(&points, 0, &profile, settings.seed, [9, 1, 2, 100 + case]);
        let plan = PathPlanner.plan(&network, &settings, &env);
        if std::env::var("DUMP_CASE")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            == Some(case)
        {
            for p in &plan.paints {
                eprintln!(
                    "TT {} {} {} {} {}",
                    p.tile_x, p.tile_y, p.overlay_id, p.shape, p.orientation
                );
            }
        }
        assert_eq!(plan.paints.len(), count, "case {case}: tile count");
        assert_eq!(fnv_lines(&plan), hash, "case {case}: tiles differ");
        assert_eq!(render(&plan).1, warnings, "case {case}: warnings");
        cases += 1;
    }
    assert_eq!(cases, 40);
}
