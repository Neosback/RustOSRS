//! A trained building model: a style kit learned from saved buildings plus a critic that likes
//! what real buildings look like, with a card saying what it was learned from and how well it does.
//!
//! A model is one JSON file (`models/<id>.model.json`). It needs no example to build with: give it
//! a brief and it builds a new building in its style, tries several seeds ("best of N") and keeps
//! the one its critic likes best.

use crate::{Brief, Critic, CriticMetrics, Generated, generate_named};
use osrs_edit::KitFile;
use osrs_scene::building_census::{ObjectInfo, ObjectTable};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MODEL_FORMAT: &str = "rustosrs-building-model";
pub const MODEL_VERSION: u32 = 1;

/// A saved building the model learned from, and a hash of what was in it then.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainedOn {
    pub name: String,
    pub hash: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelMetrics {
    pub buildings_learned_from: usize,
    pub rooms: usize,
    pub furniture_rules: usize,
    pub floor_rules: usize,
    /// Of the test briefs, the share that built a usable building.
    pub usable_rate: f32,
    /// How real the critic thinks the model's buildings look, 0 to 1, with the first seed...
    pub realism_single: f32,
    /// ...and keeping the best of 16 seeds.
    pub realism_best_of_16: f32,
    pub critic: CriticMetrics,
    pub corpus_real: usize,
    pub corpus_generated: usize,
    pub seconds: f32,
}

/// What an object the kit uses is called, so a generated building is judged the way a real one is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelName {
    pub name: String,
    pub symbol: String,
    pub actions: Vec<String>,
    pub size: [u8; 2],
}

/// How many rooms real buildings of a size have: the counts of real roofed buildings with 1, 2, 3,
/// 4 and 5 or more rooms, for floors of up to `up_to_area` tiles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructureBucket {
    pub up_to_area: u32,
    pub rooms: [u32; 5],
}

/// What a model knows about layout: how many rooms a building of a given size usually has.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Structure {
    pub buckets: Vec<StructureBucket>,
}

impl Structure {
    const EDGES: [u32; 6] = [30, 60, 100, 160, 240, u32::MAX];

    /// Count the room numbers of real buildings given as `(floor tiles, rooms)`.
    pub fn from_buildings(buildings: impl Iterator<Item = (u32, u32)>) -> Self {
        let mut buckets: Vec<StructureBucket> = Self::EDGES
            .iter()
            .map(|edge| StructureBucket {
                up_to_area: *edge,
                rooms: [0; 5],
            })
            .collect();
        for (area, rooms) in buildings {
            let at = Self::EDGES.iter().position(|e| area <= *e).unwrap_or(5);
            buckets[at].rooms[(rooms.clamp(1, 5) - 1) as usize] += 1;
        }
        Self { buckets }
    }

    /// A room count for a building of `area` floor tiles, drawn with `roll` from the counts (a
    /// bucket with no buildings in it, or none at all, gives `None`: let the planner choose).
    pub fn pick(&self, area: u32, roll: u64) -> Option<u8> {
        let bucket = self.buckets.iter().find(|b| area <= b.up_to_area)?;
        let total: u64 = bucket.rooms.iter().map(|c| u64::from(*c)).sum();
        if total == 0 {
            return None;
        }
        let mut at = roll % total;
        for (index, count) in bucket.rooms.iter().enumerate() {
            if at < u64::from(*count) {
                return Some(index as u8 + 1);
            }
            at -= u64::from(*count);
        }
        None
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Model {
    pub format: String,
    pub version: u32,
    /// File-name safe and unique.
    pub id: String,
    pub name: String,
    /// When it was trained, as the caller wrote it (the library never reads a clock).
    pub created: String,
    pub trained_on: Vec<TrainedOn>,
    pub kit: KitFile,
    pub critic: Critic,
    pub structure: Structure,
    /// Names of the objects the kit uses (by object id).
    pub names: BTreeMap<u32, ModelName>,
    pub metrics: ModelMetrics,
    /// What the training found, in plain sentences.
    pub summary: Vec<String>,
    /// Detects a damaged or hand-edited file.
    pub checksum: String,
}

/// FNV-1a over bytes: a cheap, stable fingerprint (not a security measure).
pub fn fingerprint(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

impl Model {
    /// The names as the census reads them.
    pub fn object_table(&self) -> ObjectTable {
        self.names
            .iter()
            .map(|(id, n)| {
                (
                    osrs_core::ids::ObjectId::new(*id),
                    ObjectInfo {
                        name: n.name.clone(),
                        symbol: n.symbol.clone(),
                        actions: n.actions.clone(),
                        size: (n.size[0], n.size[1]),
                    },
                )
            })
            .collect()
    }

    fn compute_checksum(&self) -> Result<String, String> {
        let body = (
            &self.id,
            &self.name,
            &self.trained_on,
            &self.kit,
            &self.critic,
        );
        serde_json::to_vec(&body)
            .map(|bytes| fingerprint(&bytes))
            .map_err(|e| e.to_string())
    }

    /// Fill in the checksum (call last, after everything else is set).
    pub fn seal(mut self) -> Result<Self, String> {
        self.checksum = self.compute_checksum()?;
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.format != MODEL_FORMAT {
            return Err(format!("not a building model (format {:?})", self.format));
        }
        if self.version != MODEL_VERSION {
            return Err(format!("model version {} is not supported", self.version));
        }
        if self.id.is_empty() || self.id.contains(['/', '\\', '.']) {
            return Err("the model's id is not a safe file name".into());
        }
        if !self.critic.is_compatible() {
            return Err(
                "the model's critic was made for a different set of building numbers".into(),
            );
        }
        self.kit.compile().map_err(|e| e.to_string())?;
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
}

/// One candidate building and how the critic scored it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    pub seed: u64,
    pub realism: f32,
}

/// What building from a brief with a model produced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Built {
    pub generated: Generated,
    pub realism: f32,
    /// Every usable candidate tried, best first.
    pub tried: Vec<Candidate>,
    /// Seeds the planner could not build at all (a layout that cannot be split) or that failed the
    /// judge.
    pub rejected: usize,
}

/// Build the brief `tries` times with consecutive seeds and keep the candidate the critic likes
/// best. No example building is involved.
pub fn build_with_model(model: &Model, brief: &Brief, tries: u32) -> Result<Built, String> {
    let tries = tries.clamp(1, 64);
    let mut best: Option<(i64, Generated)> = None;
    let mut tried = Vec::new();
    let names = model.object_table();
    let mut rejected = 0;
    let mut last_error = String::new();
    for offset in 0..u64::from(tries) {
        let seed = brief.seed.wrapping_add(offset);
        let mut attempt = Brief {
            seed,
            ..brief.clone()
        };
        if attempt.rooms.is_none() {
            // The model's idea of how many rooms a building this size has (vary it per try).
            let area = u32::from(brief.width.saturating_sub(2))
                * u32::from(brief.height.saturating_sub(2));
            attempt.rooms = model
                .structure
                .pick(area, seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) >> 20);
        }
        match generate_named(&model.kit, &attempt, &names) {
            Ok(made) if made.verdict.usable => {
                let features = features_of(&made)?;
                let score = model.critic.score(&features);
                tried.push(Candidate {
                    seed: attempt.seed,
                    realism: model.critic.realism(&features),
                });
                if best.as_ref().is_none_or(|(s, _)| score > *s) {
                    best = Some((score, made));
                }
            }
            Ok(_) => rejected += 1,
            Err(error) => {
                rejected += 1;
                last_error = error;
            }
        }
    }
    let Some((_, generated)) = best else {
        return Err(if last_error.is_empty() {
            "none of the tries gave a usable building".to_owned()
        } else {
            format!("no building could be built: {last_error}")
        });
    };
    tried.sort_by(|a, b| b.realism.total_cmp(&a.realism).then(a.seed.cmp(&b.seed)));
    let realism = tried.first().map_or(0.0, |c| c.realism);
    Ok(Built {
        generated,
        realism,
        tried,
        rejected,
    })
}

/// The critic's input for a generated building.
pub fn features_of(
    made: &Generated,
) -> Result<[f32; osrs_scene::building_census::FEATURES], String> {
    let mut out = [0.0; osrs_scene::building_census::FEATURES];
    for (slot, name) in out
        .iter_mut()
        .zip(osrs_scene::building_census::FEATURE_NAMES)
    {
        *slot = *made
            .verdict
            .features
            .get(name)
            .ok_or_else(|| format!("the building has no {name} number"))?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structure_counts_rooms_by_size_and_picks_from_the_counts() {
        // Small buildings are mostly one room, big ones have several.
        let mut real = Vec::new();
        real.extend((0..9).map(|_| (40_u32, 1_u32)));
        real.push((40, 2));
        real.extend((0..8).map(|_| (200_u32, 4_u32)));
        real.extend((0..2).map(|_| (200_u32, 7_u32)));
        let structure = Structure::from_buildings(real.into_iter());
        let small: Vec<u8> = (0..100).filter_map(|r| structure.pick(40, r)).collect();
        let ones = small.iter().filter(|r| **r == 1).count();
        assert!(ones >= 80, "mostly one room: {ones}");
        assert!(small.iter().all(|r| *r <= 2));
        let big: Vec<u8> = (0..100).filter_map(|r| structure.pick(200, r)).collect();
        assert!(big.iter().all(|r| *r == 4 || *r == 5), "{big:?}");
        // No buildings of that size: let the planner choose.
        assert_eq!(structure.pick(25, 3), None);
        assert_eq!(Structure::default().pick(40, 1), None);
        assert_eq!(
            structure.pick(40, 17),
            structure.pick(40, 17),
            "deterministic"
        );
    }
}
