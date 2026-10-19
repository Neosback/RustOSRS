//! Authentic OSRS world map labels (names, coordinates, scale, color).
//!
//! Provides geographical points of interest across the entire OSRS world map
//! (dungeons, towns, shrines, landmarks) embedded statically from `data/map_labels.json`.

use serde::Deserialize;
use std::sync::OnceLock;

static MAP_LABELS: OnceLock<Vec<MapLabel>> = OnceLock::new();

const MAP_LABELS_JSON: &str = include_str!("../../data/map_labels.json");

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct MapLabel {
    pub name: String,
    #[serde(rename = "worldX")]
    pub world_x: i32,
    #[serde(rename = "worldY")]
    pub world_y: i32,
    pub plane: u8,
    #[serde(rename = "textColor")]
    pub text_color: u32,
    #[serde(rename = "textScale")]
    pub text_scale: u8,
}

impl MapLabel {
    /// Return the display name with `<br>` replaced by spaces.
    pub fn clean_name(&self) -> String {
        self.name.replace("<br>", " ")
    }
}

pub(in crate::app) fn all_labels() -> &'static [MapLabel] {
    MAP_LABELS.get_or_init(|| serde_json::from_str(MAP_LABELS_JSON).unwrap_or_default())
}

/// Find any labels within a given world tile radius.
#[allow(dead_code)]
pub(in crate::app) fn labels_near(
    world_x: i32,
    world_y: i32,
    plane: u8,
    radius: i32,
) -> Vec<&'static MapLabel> {
    all_labels()
        .iter()
        .filter(|l| {
            l.plane == plane
                && (l.world_x - world_x).abs() <= radius
                && (l.world_y - world_y).abs() <= radius
        })
        .collect()
}

/// Find a label by name (case-insensitive substring match).
#[allow(dead_code)]
pub(in crate::app) fn find_by_name(query: &str) -> Option<&'static MapLabel> {
    let q = query.trim().to_lowercase();
    all_labels()
        .iter()
        .find(|l| l.name.to_lowercase().contains(&q) || l.clean_name().to_lowercase().contains(&q))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_embedded_map_labels() {
        let labels = all_labels();
        assert_eq!(labels.len(), 863);
        let Some(callisto) = labels.iter().find(|l| l.name == "Callisto's Den") else {
            panic!("Callisto's Den not found in map labels");
        };
        assert_eq!(callisto.world_x, 3293);
        assert_eq!(callisto.world_y, 3847);
        assert_eq!(callisto.plane, 0);
    }

    #[test]
    fn cleans_html_linebreaks() {
        let label = MapLabel {
            name: "Earthbound<br>Cavern".into(),
            world_x: 1377,
            world_y: 9700,
            plane: 0,
            text_color: 16777215,
            text_scale: 0,
        };
        assert_eq!(label.clean_name(), "Earthbound Cavern");
    }

    #[test]
    fn finds_by_name_substring() {
        let Some(found) = find_by_name("callisto") else {
            panic!("Callisto's Den not found by substring");
        };
        assert_eq!(found.name, "Callisto's Den");
    }
}
