//! A small judge of "does this look like a real building": a logistic model over the numbers the
//! building judge computes ([`osrs_scene::building_census::FEATURE_NAMES`]), trained on real
//! buildings against generated ones.
//!
//! Training uses floating point, but what is saved is integers and the score is integer
//! arithmetic, so the same building always gets the same score on any machine. Build several
//! candidates and keep the one the critic likes best ("best of N").

use osrs_scene::building_census::{FEATURE_NAMES, FEATURES};
use serde::{Deserialize, Serialize};

/// Fixed-point scales: a feature is multiplied by `FEATURE_SCALE`, a weight by `WEIGHT_SCALE`.
const FEATURE_SCALE: f64 = 256.0;
const WEIGHT_SCALE: f64 = 1024.0;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Critic {
    /// The feature names the weights are for, in order (checked when a model is loaded).
    pub features: Vec<String>,
    pub weights: Vec<i64>,
    pub bias: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CriticMetrics {
    /// Share of held-out buildings the critic puts on the right side (real or generated).
    pub held_out_accuracy: f32,
    /// Chance a held-out real building scores above a held-out generated one (0.5 = no better
    /// than a coin).
    pub held_out_auc: f32,
    pub trained_on: usize,
    #[serde(default)]
    pub held_out_examples: usize,
}

impl Critic {
    /// The expected feature names, for checking a loaded critic.
    pub fn expected_features() -> Vec<String> {
        FEATURE_NAMES.iter().map(|n| (*n).to_owned()).collect()
    }

    pub fn is_compatible(&self) -> bool {
        self.features == Self::expected_features() && self.weights.len() == FEATURES
    }

    /// The critic's score: higher looks more like a real building. Integer arithmetic only.
    pub fn score(&self, features: &[f32; FEATURES]) -> i64 {
        let mut total = self.bias;
        for (weight, value) in self.weights.iter().zip(features) {
            total += weight * (f64::from(*value) * FEATURE_SCALE).round() as i64;
        }
        total
    }

    /// The score as a chance between 0 and 1, for showing a person.
    pub fn realism(&self, features: &[f32; FEATURES]) -> f32 {
        let logit = self.score(features) as f64 / (WEIGHT_SCALE * FEATURE_SCALE);
        (1.0 / (1.0 + (-logit).exp())) as f32
    }
}

fn mean_std(rows: &[&[f32; FEATURES]]) -> ([f64; FEATURES], [f64; FEATURES]) {
    let n = rows.len().max(1) as f64;
    let mut mean = [0.0; FEATURES];
    for row in rows {
        for (m, v) in mean.iter_mut().zip(row.iter()) {
            *m += f64::from(*v) / n;
        }
    }
    let mut std = [0.0; FEATURES];
    for row in rows {
        for ((s, v), m) in std.iter_mut().zip(row.iter()).zip(&mean) {
            *s += (f64::from(*v) - m).powi(2) / n;
        }
    }
    for s in &mut std {
        *s = s.sqrt().max(1e-3);
    }
    (mean, std)
}

fn auc(real: &[f64], fake: &[f64]) -> f64 {
    if real.is_empty() || fake.is_empty() {
        return 0.5;
    }
    let mut wins = 0.0;
    for r in real {
        for f in fake {
            wins += if r > f {
                1.0
            } else if (r - f).abs() < f64::EPSILON {
                0.5
            } else {
                0.0
            };
        }
    }
    wins / (real.len() * fake.len()) as f64
}

type Rows<'a> = Vec<&'a [f32; FEATURES]>;

/// Every fifth row is held out.
fn split(rows: &[[f32; FEATURES]]) -> (Rows<'_>, Rows<'_>) {
    let train = rows
        .iter()
        .enumerate()
        .filter(|(i, _)| i % 5 != 0)
        .map(|(_, r)| r)
        .collect();
    let held = rows
        .iter()
        .enumerate()
        .filter(|(i, _)| i % 5 == 0)
        .map(|(_, r)| r)
        .collect();
    (train, held)
}

/// Train on `real` (label 1) against `fake` (label 0); every fifth building of each is held out
/// to measure how well the critic does on buildings it did not train on. `progress` is called
/// with a fraction between 0 and 1 and the current training loss.
pub fn train_critic(
    real: &[[f32; FEATURES]],
    fake: &[[f32; FEATURES]],
    epochs: usize,
    progress: impl FnMut(f32, f32),
) -> Result<(Critic, CriticMetrics), String> {
    let (real_train, real_held) = split(real);
    let (fake_train, fake_held) = split(fake);
    train_critic_partitioned(
        &real_train.into_iter().copied().collect::<Vec<_>>(),
        &fake_train.into_iter().copied().collect::<Vec<_>>(),
        &real_held.into_iter().copied().collect::<Vec<_>>(),
        &fake_held.into_iter().copied().collect::<Vec<_>>(),
        epochs,
        progress,
    )
}

/// Fit only the supplied training partitions. Validation never affects normalization or weights.
pub fn train_critic_partitioned(
    real: &[[f32; FEATURES]],
    fake: &[[f32; FEATURES]],
    real_validation: &[[f32; FEATURES]],
    fake_validation: &[[f32; FEATURES]],
    epochs: usize,
    mut progress: impl FnMut(f32, f32),
) -> Result<(Critic, CriticMetrics), String> {
    if real.len() < 4 || fake.len() < 4 || epochs == 0 {
        return Err(
            "the critic needs at least 4 training examples of each class and nonzero epochs".into(),
        );
    }
    let real_train: Vec<_> = real.iter().collect();
    let fake_train: Vec<_> = fake.iter().collect();
    let real_held: Vec<_> = real_validation.iter().collect();
    let fake_held: Vec<_> = fake_validation.iter().collect();
    let all: Vec<&[f32; FEATURES]> = real_train.iter().chain(&fake_train).copied().collect();
    let (mean, std) = mean_std(&all);
    let standard = |row: &[f32; FEATURES]| -> [f64; FEATURES] {
        let mut z = [0.0; FEATURES];
        for i in 0..FEATURES {
            z[i] = (f64::from(row[i]) - mean[i]) / std[i];
        }
        z
    };
    let rows: Vec<([f64; FEATURES], f64)> = real_train
        .iter()
        .map(|r| (standard(r), 1.0))
        .chain(fake_train.iter().map(|r| (standard(r), 0.0)))
        .collect();
    // Balance the two classes whatever their sizes.
    let weight_real = 0.5 / real_train.len() as f64;
    let weight_fake = 0.5 / fake_train.len() as f64;
    let mut weights = [0.0_f64; FEATURES];
    let mut bias = 0.0_f64;
    let (rate, decay) = (0.8, 1e-3);
    for epoch in 0..epochs {
        let mut grad = [0.0_f64; FEATURES];
        let (mut grad_b, mut loss) = (0.0, 0.0);
        for (z, label) in &rows {
            let logit: f64 = bias + weights.iter().zip(z).map(|(w, x)| w * x).sum::<f64>();
            let p = 1.0 / (1.0 + (-logit).exp());
            let sample_weight = if *label > 0.5 {
                weight_real
            } else {
                weight_fake
            };
            let error = (p - label) * sample_weight;
            for (g, x) in grad.iter_mut().zip(z) {
                *g += error * x;
            }
            grad_b += error;
            loss -= sample_weight
                * (label * p.max(1e-9).ln() + (1.0 - label) * (1.0 - p).max(1e-9).ln());
        }
        for (w, g) in weights.iter_mut().zip(&grad) {
            *w -= rate * (g + decay * *w);
        }
        bias -= rate * grad_b;
        if epoch % 20 == 0 || epoch + 1 == epochs {
            progress((epoch + 1) as f32 / epochs as f32, loss as f32);
        }
    }

    // Fold the standardising into the weights, then make everything integers.
    let mut int_weights = Vec::with_capacity(FEATURES);
    let mut shift = bias;
    for i in 0..FEATURES {
        let effective = weights[i] / std[i];
        shift -= weights[i] * mean[i] / std[i];
        int_weights.push((effective * WEIGHT_SCALE).round() as i64);
    }
    let critic = Critic {
        features: Critic::expected_features(),
        weights: int_weights,
        bias: (shift * WEIGHT_SCALE * FEATURE_SCALE).round() as i64,
    };

    let score = |rows: &[&[f32; FEATURES]]| -> Vec<f64> {
        rows.iter().map(|r| critic.score(r) as f64).collect()
    };
    let (real_scores, fake_scores) = (score(&real_held), score(&fake_held));
    let correct = real_scores.iter().filter(|s| **s > 0.0).count()
        + fake_scores.iter().filter(|s| **s <= 0.0).count();
    let held = real_scores.len() + fake_scores.len();
    let metrics = CriticMetrics {
        held_out_accuracy: correct as f32 / held.max(1) as f32,
        held_out_auc: auc(&real_scores, &fake_scores) as f32,
        trained_on: real_train.len() + fake_train.len(),
        held_out_examples: held,
    };
    Ok((critic, metrics))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real buildings have more furniture and windows than the plain generated ones.
    fn rows(n: usize, furniture: f32, windows: f32) -> Vec<[f32; FEATURES]> {
        (0..n)
            .map(|i| {
                let mut row = [0.0; FEATURES];
                row[0] = 60.0 + (i % 7) as f32;
                row[1] = 2.0 + (i % 3) as f32;
                row[4] = windows + (i % 5) as f32 * 0.01;
                row[8] = furniture + (i % 4) as f32 * 0.01;
                row
            })
            .collect()
    }

    #[test]
    fn the_critic_learns_what_separates_real_from_generated_and_scores_deterministically() {
        let (real, fake) = (rows(80, 0.30, 0.20), rows(80, 0.05, 0.02));
        let mut calls = 0;
        let (critic, metrics) = train_critic(&real, &fake, 200, |fraction, _| {
            assert!((0.0..=1.0).contains(&fraction));
            calls += 1;
        })
        .unwrap_or_else(|e| panic!("{e}"));
        assert!(calls >= 2, "progress is reported");
        assert!(metrics.held_out_accuracy > 0.95, "{metrics:?}");
        assert!(metrics.held_out_auc > 0.95, "{metrics:?}");
        assert!(critic.is_compatible());
        assert!(critic.score(&real[3]) > critic.score(&fake[3]));
        assert!(critic.realism(&real[3]) > 0.5 && critic.realism(&fake[3]) < 0.5);
        let (again, _) =
            train_critic(&real, &fake, 200, |_, _| {}).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(critic, again, "training is reproducible");
        // The saved form round trips and gives the same score.
        let text = serde_json::to_string(&critic).unwrap_or_default();
        let back: Critic = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(back.score(&real[7]), critic.score(&real[7]));
    }

    #[test]
    fn too_little_data_is_an_explicit_error() {
        assert!(train_critic(&rows(3, 0.3, 0.2), &rows(80, 0.05, 0.02), 10, |_, _| {}).is_err());
    }
}
