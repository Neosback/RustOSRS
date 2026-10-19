//! One source-group split shared with the Python learning pipeline.
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LearningSplit {
    Train,
    Validation,
    Test,
}
impl LearningSplit {
    pub fn for_group(group: &str) -> Self {
        let parts: Vec<_> = group.split(':').collect();
        let slot = if let ["geography", x, y] = parts.as_slice()
            && let (Ok(x), Ok(y)) = (x.parse::<i64>(), y.parse::<i64>())
        {
            (x.rem_euclid(5) * 31 + y.rem_euclid(5)).rem_euclid(5) as u64
        } else {
            let hash = group.bytes().fold(0xcbf29ce484222325_u64, |h, b| {
                (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
            });
            hash % 5
        };
        match slot {
            0 => Self::Test,
            1 => Self::Validation,
            _ => Self::Train,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn geography_matches_the_exporter_and_negative_coordinates_are_stable() {
        assert_eq!(
            LearningSplit::for_group("geography:0:0"),
            LearningSplit::Test
        );
        assert_eq!(
            LearningSplit::for_group("geography:1:0"),
            LearningSplit::Validation
        );
        assert_eq!(
            LearningSplit::for_group("geography:-1:0"),
            LearningSplit::Train
        );
    }
}
