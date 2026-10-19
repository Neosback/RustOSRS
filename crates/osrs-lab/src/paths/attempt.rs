//! Trying a model on a place: what it draws, how that compares with the real paths and with the
//! simple drawings to beat, and why it likes a tile.

use super::eval::{
    centre_lines, endpoints_of, line_score, route_tiles, score, shortest_walks, straight_lines,
};
use super::model::{PathModel, Score};
use super::route::Tile;
use super::sense::{FEATURE_INFO, FEATURES, Scene};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// The two ends of each real path are given; the model decides how to join them.
    Given,
    /// Nothing is given; the model paints the tiles it is sure of.
    Alone,
}

/// One try: the chances, what was drawn, and how it scored.
#[derive(Debug, Clone, PartialEq)]
pub struct Attempt {
    pub mode: Mode,
    pub chances: Vec<f32>,
    /// The tiles drawn (the lines, or the painted tiles).
    pub drawn: Vec<bool>,
    /// The route lines when ends were given.
    pub routes: Vec<Vec<Tile>>,
    /// The model's score, then the simple drawings it should beat, by name.
    pub score: Option<Score>,
    pub baselines: Vec<(&'static str, Score)>,
}

/// One reason a tile looks like path or not.
#[derive(Debug, Clone, PartialEq)]
pub struct Why {
    /// What the model has learned about paths, as a phrase ("lead to doors").
    pub phrase: &'static str,
    /// Whether this makes a path more likely here (true) or less likely.
    pub toward: bool,
    /// How strongly, in the model's own units.
    pub weight: f32,
}

impl PathModel {
    /// Try the model on `scene`. When the scene has real paths the result is scored against them.
    pub fn attempt(&self, scene: &Scene, mode: Mode) -> Attempt {
        let (w, h) = (scene.width, scene.height);
        let chances = self.chances(scene);
        let have_real = scene.mask.iter().any(|m| *m);
        match mode {
            Mode::Given => {
                let groups = endpoints_of(&scene.mask, w, h, 12);
                let routes = self.draw_between(scene, &groups).routes;
                let drawn = route_tiles(&routes, w, h);
                let (score_of, baselines) = if have_real {
                    let centres = centre_lines(&scene.mask, w, h, 12);
                    let straight = route_tiles(&straight_lines(&groups), w, h);
                    let shortest = route_tiles(&shortest_walks(scene, &groups), w, h);
                    (
                        Some(line_score(&scene.mask, &centres, &drawn, w, h)),
                        vec![
                            (
                                "a straight line between the ends",
                                line_score(&scene.mask, &centres, &straight, w, h),
                            ),
                            (
                                "the shortest walk between the ends",
                                line_score(&scene.mask, &centres, &shortest, w, h),
                            ),
                        ],
                    )
                } else {
                    (None, Vec::new())
                };
                Attempt {
                    mode,
                    chances,
                    drawn,
                    routes,
                    score: score_of,
                    baselines,
                }
            }
            Mode::Alone => {
                let drawn = self.sure_tiles(scene);
                let (score_of, baselines) = if have_real {
                    (
                        Some(score(&scene.mask, &drawn, w, h, 1)),
                        vec![(
                            "painting every open tile",
                            score(&scene.mask, &scene.candidate, w, h, 1),
                        )],
                    )
                } else {
                    (None, Vec::new())
                };
                Attempt {
                    mode,
                    chances,
                    drawn,
                    routes: Vec::new(),
                    score: score_of,
                    baselines,
                }
            }
        }
    }

    /// The most important reasons the model gives tile `(x, y)` its chance: how the chance
    /// changes when each thing it sees there is set to what is usual. Strongest first.
    pub fn why(&self, scene: &Scene, x: usize, y: usize, limit: usize) -> Vec<Why> {
        let features = &scene.features[y * scene.width + x];
        let base = self.net.logit(features);
        let mut out: Vec<Why> = (0..FEATURES)
            .filter_map(|k| {
                let mut usual = *features;
                usual[k] = self.net.mean[k];
                let weight = base - self.net.logit(&usual);
                (weight.abs() > 0.05).then(|| Why {
                    phrase: if weight >= 0.0 {
                        FEATURE_INFO[k].toward
                    } else {
                        FEATURE_INFO[k].away
                    },
                    toward: weight >= 0.0,
                    weight: weight.abs(),
                })
            })
            .collect();
        out.sort_by(|a, b| b.weight.total_cmp(&a.weight));
        out.truncate(limit);
        out
    }
}
