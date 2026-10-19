//! Teaching the path model: from lessons (real places with their paths) to a network that says
//! how likely each tile is to be path, then checks on places it did not learn from.

use super::eval::{
    centre_lines, endpoints_of, line_score, route_tiles, score, shortest_walks, straight_lines,
};
use super::lesson::PathLesson;
use super::model::{PATH_MODEL_FORMAT, PATH_MODEL_VERSION, PathMetrics, PathModel, Reason, Score};
use super::net::{Net, Row, fit, sigmoid};
use super::route;
use super::sense::{FEATURE_INFO, FEATURES, Scene};
use std::collections::BTreeSet;

pub struct PathTrainRequest<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub created: &'a str,
    pub lessons: &'a [PathLesson],
    /// Hidden units (0 = plain logistic regression).
    pub hidden: usize,
    pub epochs: usize,
    pub seed: u64,
    pub cancelled: &'a dyn Fn() -> bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PathProgress {
    Step { percent: u8, message: String },
}

fn report(on: &mut dyn FnMut(PathProgress), percent: u8, message: impl Into<String>) {
    on(PathProgress::Step {
        percent,
        message: message.into(),
    });
}

fn hash(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// Whether tile `(x, y)` of lesson `index` is kept out of training to be checked on.
#[derive(Clone, Copy)]
enum Split {
    /// Whole areas are held out: those of these lessons.
    Areas,
    /// Every area keeps its eastern third for checking.
    EastThird,
}

fn is_held(split: Split, lesson: &PathLesson, x: usize) -> bool {
    match split {
        Split::Areas => hash(&lesson.group).is_multiple_of(5),
        Split::EastThird => x * 3 >= usize::from(lesson.width) * 2,
    }
}

fn near_mask(mask: &[bool], width: usize, height: usize, tolerance: u8) -> Vec<bool> {
    let d = super::grid::distance_to(mask, width, height, 250);
    d.iter().map(|d| *d <= tolerance).collect()
}

fn auc_and_ap(scored: &mut [(f32, bool)]) -> (f32, f32) {
    let positives = scored.iter().filter(|s| s.1).count();
    let negatives = scored.len() - positives;
    if positives == 0 || negatives == 0 {
        return (0.5, 0.0);
    }
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    // AUC from ranks (ties share the average rank).
    let mut rank_sum = 0.0_f64;
    let mut i = scored.len();
    let mut rank = 1.0_f64;
    while i > 0 {
        let mut j = i;
        while j > 0 && scored[j - 1].0 == scored[i - 1].0 {
            j -= 1;
        }
        let tied = i - j;
        let average = rank + (tied as f64 - 1.0) / 2.0;
        rank_sum += average * scored[j..i].iter().filter(|s| s.1).count() as f64;
        rank += tied as f64;
        i = j;
    }
    let auc = (rank_sum - positives as f64 * (positives as f64 + 1.0) / 2.0)
        / (positives as f64 * negatives as f64);
    let (mut hits, mut ap) = (0.0_f64, 0.0_f64);
    for (n, s) in scored.iter().enumerate() {
        if s.1 {
            hits += 1.0;
            ap += hits / (n as f64 + 1.0);
        }
    }
    (auc as f32, (ap / positives as f64) as f32)
}

pub fn train_paths(
    req: &PathTrainRequest<'_>,
    clock: &dyn Fn() -> f32,
    on: &mut dyn FnMut(PathProgress),
) -> Result<PathModel, String> {
    if req.lessons.is_empty() {
        return Err("there are no lessons to learn from".into());
    }
    report(on, 2, format!("Reading {} place(s)", req.lessons.len()));
    let scenes: Vec<Scene> = req.lessons.iter().map(Scene::of).collect();
    let path_tiles: usize = scenes
        .iter()
        .map(|s| s.mask.iter().filter(|m| **m).count())
        .sum();
    if path_tiles < 12 {
        return Err(
            "those places have almost no path in them, so there is nothing to learn".into(),
        );
    }
    if (req.cancelled)() {
        return Err("stopped".into());
    }
    let groups: BTreeSet<&str> = req.lessons.iter().map(|l| l.group.as_str()).collect();
    let split = if groups.len() >= 3
        && req.lessons.iter().any(|l| is_held(Split::Areas, l, 0))
        && req.lessons.iter().any(|l| !is_held(Split::Areas, l, 0))
    {
        Split::Areas
    } else {
        Split::EastThird
    };
    report(on, 8, "Choosing what to learn from");
    let mut rows: Vec<Row> = Vec::new();
    let mut rng = super::net::Stream(req.seed ^ 0xabcd);
    for (lesson, scene) in req.lessons.iter().zip(&scenes) {
        let near = near_mask(&scene.mask, scene.width, scene.height, 2);
        for y in 0..scene.height {
            for x in 0..scene.width {
                let a = y * scene.width + x;
                if is_held(split, lesson, x) || !scene.candidate[a] {
                    continue;
                }
                if scene.mask[a] || near[a] {
                    rows.push(Row {
                        x: scene.features[a],
                        y: f32::from(u8::from(scene.mask[a])),
                        w: 1.0,
                    });
                } else if rng.unit() < 0.15 {
                    rows.push(Row {
                        x: scene.features[a],
                        y: 0.0,
                        w: 1.0 / 0.15,
                    });
                }
            }
        }
    }
    if rows.iter().filter(|r| r.y > 0.5).count() < 8 {
        return Err(
            "too little path is left to learn from after setting some ground aside to check on"
                .into(),
        );
    }
    report(on, 12, format!("Learning from {} spots", rows.len()));
    let epochs = req.epochs.max(1);
    let net = fit(
        &rows,
        req.hidden,
        epochs,
        1e-3,
        req.seed,
        |epoch, total, loss| {
            let percent = 12 + (epoch * 60 / total) as u8;
            report(
                on,
                percent,
                format!("Learning: round {epoch} of {total} (mistakes {loss:.3})"),
            );
        },
        req.cancelled,
    );
    if (req.cancelled)() {
        return Err("stopped".into());
    }
    // Check on the ground it did not learn from.
    report(on, 74, "Checking on ground it has not seen");
    let mut scored: Vec<(f32, bool)> = Vec::new();
    let mut held_features: Vec<([f32; FEATURES], bool)> = Vec::new();
    for (lesson, scene) in req.lessons.iter().zip(&scenes) {
        for y in 0..scene.height {
            for x in 0..scene.width {
                let a = y * scene.width + x;
                if is_held(split, lesson, x) && scene.candidate[a] {
                    scored.push((net.prob(&scene.features[a]), scene.mask[a]));
                    held_features.push((scene.features[a], scene.mask[a]));
                }
            }
        }
    }
    let base_rate = scored.iter().filter(|s| s.1).count() as f32 / scored.len().max(1) as f32;
    let (auc, ap) = auc_and_ap(&mut scored.clone());
    report(on, 80, "Working out its reasons");
    let reasons = reasons_of(&net, &held_features, auc);
    report(on, 86, "Testing drawing paths");
    let (threshold, width, overlay) = settings_of(&net, req.lessons, &scenes);
    let mut model = PathModel {
        format: PATH_MODEL_FORMAT.to_owned(),
        version: PATH_MODEL_VERSION,
        id: req.id.to_owned(),
        name: req.name.to_owned(),
        created: req.created.to_owned(),
        trained_on: req.lessons.iter().map(|l| l.name.clone()).collect(),
        net,
        threshold,
        width,
        overlay,
        reasons,
        metrics: PathMetrics {
            lessons: req.lessons.len(),
            path_tiles,
            split: match split {
                Split::Areas => "whole areas".to_owned(),
                Split::EastThird => {
                    "the eastern third of each area was set aside to check on".to_owned()
                }
            },
            checked_on_unseen: matches!(split, Split::Areas),
            auc,
            average_precision: ap,
            base_rate,
            given_model: Score::default(),
            given_straight: Score::default(),
            given_shortest: Score::default(),
            alone_tiles: Score::default(),
            alone_everywhere: Score::default(),
            strict_given: Score::default(),
            strict_alone: Score::default(),
            seconds: 0.0,
        },
        summary: Vec::new(),
        checksum: String::new(),
    };
    // Drawing checks: on the held-out areas when whole areas were set aside, otherwise on all.
    let checked: Vec<usize> = (0..req.lessons.len())
        .filter(|i| matches!(split, Split::EastThird) || is_held(Split::Areas, &req.lessons[*i], 0))
        .collect();
    let mut sums = [Score::default(); 7];
    let mut n = 0.0_f32;
    for &i in &checked {
        if (req.cancelled)() {
            return Err("stopped".into());
        }
        let scene = &scenes[i];
        let groups = endpoints_of(&scene.mask, scene.width, scene.height, 12);
        if groups.is_empty() {
            continue;
        }
        let (w, h) = (scene.width, scene.height);
        let centres = centre_lines(&scene.mask, w, h, 12);
        let given = route_tiles(&model.draw_between(scene, &groups).routes, w, h);
        let straight = route_tiles(&straight_lines(&groups), w, h);
        let shortest = route_tiles(&shortest_walks(scene, &groups), w, h);
        let sure = model.sure_tiles(scene);
        let results = [
            line_score(&scene.mask, &centres, &given, w, h),
            line_score(&scene.mask, &centres, &straight, w, h),
            line_score(&scene.mask, &centres, &shortest, w, h),
            score(&scene.mask, &sure, w, h, 1),
            score(&scene.mask, &scene.candidate, w, h, 1),
            score(
                &scene.mask,
                &route::band(&model.draw_between(scene, &groups).routes, 1, w, h),
                w,
                h,
                0,
            ),
            score(&scene.mask, &sure, w, h, 0),
        ];
        for (sum, s) in sums.iter_mut().zip(results) {
            sum.precision += s.precision;
            sum.recall += s.recall;
            sum.f1 += s.f1;
        }
        n += 1.0;
    }
    let mean = |s: Score| {
        if n == 0.0 {
            Score::default()
        } else {
            Score {
                precision: s.precision / n,
                recall: s.recall / n,
                f1: s.f1 / n,
            }
        }
    };
    model.metrics.given_model = mean(sums[0]);
    model.metrics.given_straight = mean(sums[1]);
    model.metrics.given_shortest = mean(sums[2]);
    model.metrics.alone_tiles = mean(sums[3]);
    model.metrics.alone_everywhere = mean(sums[4]);
    model.metrics.strict_given = mean(sums[5]);
    model.metrics.strict_alone = mean(sums[6]);
    model.metrics.seconds = clock();
    model.summary = summary_of(&model);
    report(on, 100, "Done");
    model.seal()
}

/// Tune when "likely enough to be path" is (by tile score), the usual width, and the overlay.
fn settings_of(net: &Net, lessons: &[PathLesson], scenes: &[Scene]) -> (f32, f32, u16) {
    let mut best = (0.5_f32, -1.0_f32);
    for step in 1..=18 {
        let t = step as f32 * 0.05;
        let (mut hit, mut made, mut real) = (0_usize, 0_usize, 0_usize);
        for scene in scenes {
            for a in 0..scene.tiles() {
                let said = net.prob(&scene.features[a]) >= t && scene.candidate[a];
                made += usize::from(said);
                real += usize::from(scene.mask[a]);
                hit += usize::from(said && scene.mask[a]);
            }
        }
        let f1 = if made + real == 0 {
            0.0
        } else {
            2.0 * hit as f32 / (made + real) as f32
        };
        if f1 > best.1 {
            best = (t, f1);
        }
    }
    let mut overlay_counts: std::collections::BTreeMap<u16, u32> = Default::default();
    let (mut width_sum, mut width_tiles) = (0.0_f32, 0_u32);
    for (lesson, scene) in lessons.iter().zip(scenes) {
        let inside: BTreeSet<usize> = (0..scene.tiles()).filter(|a| scene.inside[*a]).collect();
        let set = lesson.path_overlay_set(&inside);
        for a in 0..scene.tiles() {
            if scene.mask[a] {
                *overlay_counts.entry(lesson.overlay[a]).or_default() += 1;
            }
        }
        for g in lesson
            .guess_paths(&inside)
            .into_iter()
            .filter(|g| set.contains(&g.overlay))
        {
            width_sum += g.width * g.tiles as f32;
            width_tiles += g.tiles;
        }
    }
    let width = if width_tiles == 0 {
        3.0
    } else {
        ((width_sum / width_tiles as f32) * 2.0).round() / 2.0
    };
    let overlay = overlay_counts
        .into_iter()
        .max_by_key(|(_, n)| *n)
        .map_or(0, |(o, _)| o);
    (best.0, width.clamp(1.0, 7.0), overlay)
}

/// Why it draws where it does: how much the checking score drops without each thing, and which
/// way each thing pushes.
fn reasons_of(net: &Net, held: &[([f32; FEATURES], bool)], auc: f32) -> Vec<Reason> {
    if held.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (k, info) in FEATURE_INFO.iter().enumerate() {
        let mut shuffled: Vec<(f32, bool)> = held
            .iter()
            .map(|(x, y)| {
                let mut x = *x;
                x[k] = net.mean[k];
                (net.prob(&x), *y)
            })
            .collect();
        let (without, _) = auc_and_ap(&mut shuffled);
        let drop = (auc - without).max(0.0);
        let positives: Vec<&[f32; FEATURES]> = held.iter().filter(|h| h.1).map(|h| &h.0).collect();
        let sample: Vec<&[f32; FEATURES]> = if positives.is_empty() {
            held.iter().take(200).map(|h| &h.0).collect()
        } else {
            positives
        };
        let slope = sample
            .iter()
            .take(400)
            .map(|x| {
                let mut up = **x;
                up[k] += net.std[k] * 0.5;
                net.logit(&up) - net.logit(x)
            })
            .sum::<f32>();
        out.push((drop, slope >= 0.0, info));
    }
    let top = out.iter().map(|o| o.0).fold(0.0_f32, f32::max).max(1e-6);
    let mut reasons: Vec<Reason> = out
        .into_iter()
        .map(|(drop, toward, info)| Reason {
            feature: info.key.to_owned(),
            text: (if toward { info.toward } else { info.away }).to_owned(),
            strength: drop / top,
            toward,
        })
        .collect();
    reasons.sort_by(|a, b| {
        b.strength
            .total_cmp(&a.strength)
            .then(a.feature.cmp(&b.feature))
    });
    reasons
}

fn summary_of(model: &PathModel) -> Vec<String> {
    let m = &model.metrics;
    let mut lines = vec![
        format!(
            "Learned from {} place(s) with {} path tiles.",
            m.lessons, m.path_tiles
        ),
        format!(
            "It set aside {} to check on, and did not learn from it.",
            m.split
        ),
        format!(
            "Picking out path tiles on ground it had not seen: right {:.0}% of the time when comparing a path tile with a bare one (50% would be guessing). Only {:.1}% of tiles are path.",
            m.auc * 100.0,
            m.base_rate * 100.0
        ),
    ];
    if m.given_model.f1 > 0.0 || m.given_straight.f1 > 0.0 {
        lines.push(format!(
            "Given the two ends of a real path, its drawing matches {:.0}% (a straight line matches {:.0}%, the shortest walk {:.0}%).",
            m.given_model.f1 * 100.0,
            m.given_straight.f1 * 100.0,
            m.given_shortest.f1 * 100.0
        ));
        lines.push(format!(
            "With nothing given, painting what it is sure of matches {:.0}% (painting every open tile matches {:.0}%).",
            m.alone_tiles.f1 * 100.0,
            m.alone_everywhere.f1 * 100.0
        ));
        lines.push(
            "\"Matches\" counts a tile as right when a real path is within 1 tile of it (for the drawn lines: on a real path, and covering the real centre line to within 2 tiles)."
                .to_owned(),
        );
    }
    let top: Vec<String> = model
        .reasons
        .iter()
        .filter(|r| r.strength > 0.2)
        .take(3)
        .map(|r| format!("paths {}", r.text))
        .collect();
    if !top.is_empty() {
        lines.push(format!("Its main reasons: {}.", top.join("; ")));
    }
    lines
}

/// Silence an unused warning for `sigmoid` and `route` imports in some configurations.
#[allow(dead_code)]
fn _uses() -> f32 {
    let _ = route::straight((0, 0), (1, 1));
    sigmoid(0.0)
}
