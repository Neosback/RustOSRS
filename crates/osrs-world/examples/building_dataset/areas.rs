//! Optional, weak area labels, separate from confirmed building labels.
use serde_json::Value;
/// A named rectangle from 117HD's `areas.json`.
pub(super) struct Area {
    pub(super) name: String,
    pub(super) boxes: Vec<osrs_hd::tiles::Aabb>,
}

pub(super) fn read_areas(path: &str) -> Result<Vec<Area>, Box<dyn std::error::Error>> {
    // The file has `//` comment lines; drop them before parsing.
    let text: String = std::fs::read_to_string(path)?
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let value: Value = serde_json::from_str(&text)?;
    let mut areas = Vec::new();
    for entry in value.as_array().into_iter().flatten() {
        let Some(name) = entry["name"].as_str() else {
            continue;
        };
        let boxes = entry["aabbs"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|b| {
                let n: Vec<i32> = b
                    .as_array()?
                    .iter()
                    .filter_map(|v| v.as_i64())
                    .map(|v| v as i32)
                    .collect();
                osrs_hd::tiles::Aabb::from_numbers(&n)
            })
            .collect();
        areas.push(Area {
            name: name.to_owned(),
            boxes,
        });
    }
    Ok(areas)
}
