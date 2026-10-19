//! A trained path model: the network, what it learned in plain words, how well it did on ground
//! it had not seen, and how to draw with it. One JSON file, with a checksum.

use super::eval::{Draft, painted_radius};
use super::net::Net;
use super::route::{self, Tile};
use super::sense::{FEATURE_INFO, FEATURES, Scene};
use crate::fingerprint;
use serde::{Deserialize, Serialize};

pub const PATH_MODEL_FORMAT: &str = "rustosrs-path-model";
pub const PATH_MODEL_VERSION: u32 = 1;

/// How well a drawing matches the real paths: of what was drawn, the share that is within a
/// couple of tiles of a real path (precision), and of the real path, the share a drawing comes
/// within a couple of tiles of (recall).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Score {
    pub precision: f32,
    pub recall: f32,
    pub f1: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PathMetrics {
    pub lessons: usize,
    pub path_tiles: usize,
    /// How the ground was split into what it learned from and what it was checked on.
    pub split: String,
    pub checked_on_unseen: bool,
    /// Chance a real path tile scores above a non-path tile (0.5 = guessing, 1 = perfect).
    pub auc: f32,
    pub average_precision: f32,
    /// The share of tiles that are path: what guessing would score as precision.
    pub base_rate: f32,
    /// Drawing between the real path's ends: the model, a straight line, shortest walk.
    pub given_model: Score,
    pub given_straight: Score,
    pub given_shortest: Score,
    /// Drawing with nothing given: painting every tile the model is sure is path.
    pub alone_tiles: Score,
    /// The same by painting every open tile (what doing nothing clever scores).
    pub alone_everywhere: Score,
    /// The two above that matter most, scored with no slack (a tile must be exactly right).
    pub strict_given: Score,
    pub strict_alone: Score,
    pub seconds: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reason {
    pub feature: String,
    pub text: String,
    /// 0 to 1: how much the model's accuracy suffers without it, relative to the strongest.
    pub strength: f32,
    /// Paths are drawn more where this is high (true) or less (false).
    pub toward: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PathModel {
    pub format: String,
    pub version: u32,
    pub id: String,
    pub name: String,
    pub created: String,
    pub trained_on: Vec<String>,
    pub net: Net,
    /// Tiles at least this likely are taken to be path when it finds paths itself.
    pub threshold: f32,
    /// The usual path width in the lessons, in tiles.
    pub width: f32,
    /// The overlay the lessons' paths were made of (the most common one).
    pub overlay: u16,
    pub reasons: Vec<Reason>,
    pub metrics: PathMetrics,
    pub summary: Vec<String>,
    pub checksum: String,
}

impl PathModel {
    /// The model that ships with the editor, learned from a sample of the game's own map.
    pub fn starter() -> Result<Self, String> {
        Self::from_json(include_str!("../../data/world-paths.model.json"))
    }

    fn compute_checksum(&self) -> Result<String, String> {
        let body = (
            &self.id,
            &self.name,
            &self.trained_on,
            &self.net,
            self.threshold.to_bits(),
            self.overlay,
        );
        serde_json::to_vec(&body)
            .map(|b| fingerprint(&b))
            .map_err(|e| e.to_string())
    }

    pub fn seal(mut self) -> Result<Self, String> {
        self.checksum = self.compute_checksum()?;
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.format != PATH_MODEL_FORMAT {
            return Err(format!("not a path model (format {:?})", self.format));
        }
        if self.version != PATH_MODEL_VERSION {
            return Err(format!(
                "path model version {} is not supported",
                self.version
            ));
        }
        if self.id.is_empty() || self.id.contains(['/', '\\', '.']) {
            return Err("the model's id is not a safe file name".into());
        }
        let n = &self.net;
        if n.mean.len() != FEATURES
            || n.std.len() != FEATURES
            || n.w1.len() != n.hidden * FEATURES
            || n.b1.len() != n.hidden
            || n.w2.len() != if n.hidden == 0 { FEATURES } else { n.hidden }
        {
            return Err("the model was made for a different set of things to look at".into());
        }
        if self.checksum != self.compute_checksum()? {
            return Err("the model file was changed after it was trained".into());
        }
        Ok(())
    }

    pub fn from_json(text: &str) -> Result<Self, String> {
        let model: Self = serde_json::from_str(text).map_err(|e| e.to_string())?;
        model.validate()?;
        Ok(model)
    }

    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }

    /// The chance that each tile of a place is path.
    pub fn chances(&self, scene: &Scene) -> Vec<f32> {
        scene.features.iter().map(|f| self.net.prob(f)).collect()
    }

    /// What it costs to lay a path over each tile: cheap where paths are likely, `None` inside
    /// buildings, and dear over blocked ground.
    pub fn prices(&self, scene: &Scene, chances: &[f32]) -> Vec<Option<f32>> {
        (0..scene.tiles())
            .map(|a| {
                if scene.inside[a] {
                    None
                } else {
                    let base = 0.2 + (-chances[a].max(1e-4).ln()).min(9.0);
                    Some(if scene.walkable[a] && scene.candidate[a] {
                        base
                    } else {
                        base + 40.0
                    })
                }
            })
            .collect()
    }

    /// Draw paths joining `points` (each group is one network).
    pub fn draw_between(&self, scene: &Scene, groups: &[Vec<Tile>]) -> Draft {
        let chances = self.chances(scene);
        let prices = self.prices(scene, &chances);
        let routes: Vec<Vec<Tile>> = groups
            .iter()
            .flat_map(|g| route::network(scene.width, scene.height, &prices, g))
            .collect();
        Draft::new(scene, routes, painted_radius(self.width))
    }

    /// The tiles it is sure are path, with specks removed: paint these to draw paths with nothing
    /// given.
    pub fn sure_tiles(&self, scene: &Scene) -> Vec<bool> {
        let chances = self.chances(scene);
        let raw: Vec<bool> = chances
            .iter()
            .enumerate()
            .map(|(a, p)| *p >= self.threshold && scene.candidate[a])
            .collect();
        let mut kept = vec![false; raw.len()];
        for piece in route::pieces(&raw, scene.width, scene.height, 6) {
            for (x, y) in piece {
                kept[y * scene.width + x] = true;
            }
        }
        kept
    }

    /// The plain-language reasons, strongest first.
    pub fn reasons_text(&self) -> Vec<String> {
        self.reasons
            .iter()
            .map(|r| format!("Paths {}.", r.text))
            .collect()
    }

    /// The names the reasons use, for tests and the CLI.
    pub fn feature_keys() -> Vec<&'static str> {
        FEATURE_INFO.iter().map(|f| f.key).collect()
    }
}
