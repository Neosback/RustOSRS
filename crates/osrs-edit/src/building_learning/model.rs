//! Quantized offline role suggestions. Scores are evidence margins, never calibrated probabilities.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LearnedRoleModel {
    pub format: String,
    pub version: u32,
    pub sources: Vec<String>,
    pub labels: Vec<String>,
    pub scale: i32,
    pub priors: Vec<i32>,
    pub feature_weights: BTreeMap<String, Vec<i32>>,
}
impl LearnedRoleModel {
    pub fn validate(&self) -> Result<(), super::LearningError> {
        if self.format != "rustosrs-role-model"
            || self.version != 1
            || self.labels.is_empty()
            || self.labels.len() > 128
            || self.labels.iter().collect::<BTreeSet<_>>().len() != self.labels.len()
            || self.priors.len() != self.labels.len()
            || self.scale <= 0
            || self.scale > 1_000_000
            || self.feature_weights.len() > 50_000
            || self
                .feature_weights
                .values()
                .any(|v| v.len() != self.labels.len())
            || self
                .priors
                .iter()
                .chain(self.feature_weights.values().flatten())
                .any(|v| i64::from(*v).abs() > 100_000_000)
        {
            return Err(super::LearningError::InvalidCatalog);
        }
        Ok(())
    }
    /// The adapter supplies the same named features used by the exported training manifest.
    pub fn predict(&self, features: &[String]) -> Option<(&str, i64)> {
        if self.labels.len() != self.priors.len() || self.labels.is_empty() {
            return None;
        }
        let mut scores: Vec<_> = self.priors.iter().map(|&v| i64::from(v)).collect();
        for feature in features.iter().take(1024) {
            if let Some(weights) = self.feature_weights.get(feature) {
                if weights.len() != scores.len() {
                    return None;
                }
                for (score, &w) in scores.iter_mut().zip(weights) {
                    *score += i64::from(w);
                }
            }
        }
        let mut order: Vec<_> = scores.into_iter().enumerate().collect();
        order.sort_by_key(|&(i, score)| (std::cmp::Reverse(score), i));
        let (index, best) = *order.first()?;
        Some((
            &self.labels[index],
            order.get(1).map_or(0, |(_, second)| best - second),
        ))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quantized_predictions_are_deterministic_and_invalid_dimensions_are_rejected() {
        let model = LearnedRoleModel {
            format: "rustosrs-role-model".into(),
            version: 1,
            sources: vec!["fixture".into()],
            labels: vec!["bed".into(), "barrel".into()],
            scale: 1024,
            priors: vec![0, 0],
            feature_weights: BTreeMap::from([("token:bed".into(), vec![100, -100])]),
        };
        assert!(model.validate().is_ok());
        assert_eq!(model.predict(&["token:bed".into()]), Some(("bed", 200)));
        let mut invalid = model;
        invalid.priors.clear();
        assert!(invalid.validate().is_err());
        assert!(invalid.predict(&[]).is_none());
    }
}
