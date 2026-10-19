//! The small network that says how likely a tile is to be path: standardise the numbers, one
//! hidden layer (or none: plain logistic regression), then a chance between 0 and 1.
//!
//! Training is full-batch Adam on weighted rows with a fixed starting point, so the same lessons
//! and settings always give the same network. A network is a few hundred numbers: it cannot hold
//! a map, so what it keeps is a rule of thumb, not a copy.

use super::sense::FEATURES;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Net {
    /// Hidden units; 0 means plain logistic regression.
    pub hidden: usize,
    pub mean: Vec<f32>,
    pub std: Vec<f32>,
    /// `hidden * FEATURES` weights (or `FEATURES` when `hidden` is 0, taken from `w2`).
    pub w1: Vec<f32>,
    pub b1: Vec<f32>,
    /// `hidden` output weights, or `FEATURES` weights when there is no hidden layer.
    pub w2: Vec<f32>,
    pub b2: f32,
}

pub fn sigmoid(z: f32) -> f32 {
    1.0 / (1.0 + (-z).exp())
}

impl Net {
    pub fn zero(hidden: usize) -> Self {
        Self {
            hidden,
            mean: vec![0.0; FEATURES],
            std: vec![1.0; FEATURES],
            w1: vec![0.0; hidden * FEATURES],
            b1: vec![0.0; hidden],
            w2: vec![0.0; if hidden == 0 { FEATURES } else { hidden }],
            b2: 0.0,
        }
    }

    fn standard(&self, x: &[f32; FEATURES]) -> [f32; FEATURES] {
        let mut z = [0.0; FEATURES];
        for k in 0..FEATURES {
            z[k] = (x[k] - self.mean[k]) / self.std[k];
        }
        z
    }

    pub fn logit(&self, x: &[f32; FEATURES]) -> f32 {
        let z = self.standard(x);
        if self.hidden == 0 {
            return self.b2 + self.w2.iter().zip(&z).map(|(w, v)| w * v).sum::<f32>();
        }
        let mut out = self.b2;
        for j in 0..self.hidden {
            let a = self.b1[j]
                + self.w1[j * FEATURES..(j + 1) * FEATURES]
                    .iter()
                    .zip(&z)
                    .map(|(w, v)| w * v)
                    .sum::<f32>();
            out += self.w2[j] * a.tanh();
        }
        out
    }

    pub fn prob(&self, x: &[f32; FEATURES]) -> f32 {
        sigmoid(self.logit(x))
    }

    fn params(&self) -> Vec<f32> {
        let mut p = self.w1.clone();
        p.extend(&self.b1);
        p.extend(&self.w2);
        p.push(self.b2);
        p
    }

    fn set_params(&mut self, p: &[f32]) {
        let (a, b) = (self.w1.len(), self.b1.len());
        self.w1.copy_from_slice(&p[..a]);
        self.b1.copy_from_slice(&p[a..a + b]);
        let c = self.w2.len();
        self.w2.copy_from_slice(&p[a + b..a + b + c]);
        self.b2 = p[a + b + c];
    }
}

/// A training row: the numbers, the label (1 path, 0 not) and how much it counts.
pub struct Row {
    pub x: [f32; FEATURES],
    pub y: f32,
    pub w: f32,
}

/// A fixed pseudo-random stream (so training is reproducible).
pub struct Stream(pub u64);

impl Stream {
    pub fn next_u32(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) as u32
    }

    pub fn unit(&mut self) -> f32 {
        self.next_u32() as f32 / (1u64 << 31) as f32
    }

    /// Roughly normal, from the sum of three uniforms.
    pub fn normal(&mut self) -> f32 {
        (self.unit() + self.unit() + self.unit() - 1.5) * 2.0
    }
}

/// Fit a network to `rows`. `progress(epoch, epochs, loss)` is called now and then; `stop` ends
/// training early (the network so far is returned).
pub fn fit(
    rows: &[Row],
    hidden: usize,
    epochs: usize,
    l2: f32,
    seed: u64,
    mut progress: impl FnMut(usize, usize, f32),
    stop: &dyn Fn() -> bool,
) -> Net {
    let mut net = Net::zero(hidden);
    let total: f32 = rows.iter().map(|r| r.w).sum::<f32>().max(1e-6);
    for k in 0..FEATURES {
        let mean = rows.iter().map(|r| r.w * r.x[k]).sum::<f32>() / total;
        let var = rows
            .iter()
            .map(|r| r.w * (r.x[k] - mean).powi(2))
            .sum::<f32>()
            / total;
        net.mean[k] = mean;
        net.std[k] = var.sqrt().max(1e-3);
    }
    let mut rng = Stream(seed ^ 0x9e37_79b9_7f4a_7c15);
    if hidden > 0 {
        net.w1.iter_mut().for_each(|w| *w = rng.normal() * 0.3);
        net.w2.iter_mut().for_each(|w| *w = rng.normal() * 0.3);
    }
    let standard: Vec<[f32; FEATURES]> = rows.iter().map(|r| net.standard(&r.x)).collect();
    let mut p = net.params();
    let (mut m, mut v) = (vec![0.0_f32; p.len()], vec![0.0_f32; p.len()]);
    let (lr, b1c, b2c) = (0.03_f32, 0.9_f32, 0.999_f32);
    for epoch in 0..epochs {
        if stop() {
            break;
        }
        net.set_params(&p);
        let mut grad = vec![0.0_f32; p.len()];
        let mut loss = 0.0_f32;
        let (w1_len, b1_len) = (net.w1.len(), net.b1.len());
        for (row, z) in rows.iter().zip(&standard) {
            let (logit, hs) = if hidden == 0 {
                (
                    net.b2 + net.w2.iter().zip(z).map(|(w, x)| w * x).sum::<f32>(),
                    Vec::new(),
                )
            } else {
                let hs: Vec<f32> = (0..hidden)
                    .map(|j| {
                        (net.b1[j]
                            + net.w1[j * FEATURES..(j + 1) * FEATURES]
                                .iter()
                                .zip(z)
                                .map(|(w, x)| w * x)
                                .sum::<f32>())
                        .tanh()
                    })
                    .collect();
                (
                    net.b2 + net.w2.iter().zip(&hs).map(|(w, h)| w * h).sum::<f32>(),
                    hs,
                )
            };
            let pr = sigmoid(logit);
            loss -= row.w * (row.y * pr.max(1e-7).ln() + (1.0 - row.y) * (1.0 - pr).max(1e-7).ln());
            let dz = row.w * (pr - row.y);
            if hidden == 0 {
                for k in 0..FEATURES {
                    grad[w1_len + b1_len + k] += dz * z[k];
                }
            } else {
                for j in 0..hidden {
                    grad[w1_len + b1_len + j] += dz * hs[j];
                    let da = dz * net.w2[j] * (1.0 - hs[j] * hs[j]);
                    grad[w1_len + j] += da;
                    for k in 0..FEATURES {
                        grad[j * FEATURES + k] += da * z[k];
                    }
                }
            }
            let last = grad.len() - 1;
            grad[last] += dz;
        }
        for (i, g) in grad.iter_mut().enumerate() {
            *g = *g / total + if i < p.len() - 1 { l2 * p[i] } else { 0.0 };
        }
        let t = (epoch + 1) as f32;
        for i in 0..p.len() {
            m[i] = b1c * m[i] + (1.0 - b1c) * grad[i];
            v[i] = b2c * v[i] + (1.0 - b2c) * grad[i] * grad[i];
            let (mh, vh) = (m[i] / (1.0 - b1c.powf(t)), v[i] / (1.0 - b2c.powf(t)));
            p[i] -= lr * mh / (vh.sqrt() + 1e-8);
        }
        if epoch % 5 == 0 || epoch + 1 == epochs {
            progress(epoch + 1, epochs, loss / total);
        }
    }
    net.set_params(&p);
    net
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows() -> Vec<Row> {
        let mut rng = Stream(7);
        (0..600)
            .map(|i| {
                let mut x = [0.0; FEATURES];
                for v in &mut x {
                    *v = rng.unit();
                }
                // Path where feature 4 is high and feature 0 is low.
                let y = f32::from(u8::from(x[4] > 0.6 && x[0] < 0.5));
                let _ = i;
                Row { x, y, w: 1.0 }
            })
            .collect()
    }

    #[test]
    fn the_network_learns_a_rule_and_training_is_reproducible() {
        let rows = rows();
        let net = fit(&rows, 6, 120, 1e-4, 3, |_, _, _| {}, &|| false);
        let right = rows
            .iter()
            .filter(|r| (net.prob(&r.x) > 0.5) == (r.y > 0.5))
            .count();
        assert!(right as f32 / rows.len() as f32 > 0.9, "{right}");
        assert_eq!(net, fit(&rows, 6, 120, 1e-4, 3, |_, _, _| {}, &|| false));
        let linear = fit(&rows, 0, 80, 1e-4, 3, |_, _, _| {}, &|| false);
        assert!(
            linear.w2[4] > linear.w2[9],
            "feature 4 matters more than noise"
        );
        let text = serde_json::to_string(&net).unwrap_or_default();
        let back: Net = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(back.prob(&rows[0].x), net.prob(&rows[0].x));
    }
}
