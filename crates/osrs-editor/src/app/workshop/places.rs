//! Named places of the world (towns, regions, mines, dungeon mouths) from a `map_labels.json`
//! file: the labels the game's world map shows. They name the areas you import, remember what is
//! near them, and let you jump straight to a place.
//!
//! The file is a list of `{ "name", "worldX", "worldY", "plane", "textScale" }`; `textScale` is
//! 0 for a small site, 1 for a town and 2 for a whole region. Only ground-plane labels are kept.

use osrs_lab::PathPlaceMark as PlaceMark;
use std::path::PathBuf;

/// One place, in world tile coordinates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Place {
    pub(super) name: String,
    pub(super) x: i32,
    pub(super) y: i32,
    /// 0 a small site, 1 a town, 2 a region.
    pub(super) size: u8,
}

pub(super) struct Places {
    pub(super) items: Vec<Place>,
    directory: Option<PathBuf>,
    loaded: bool,
    pub(super) problem: Option<String>,
}

const FILE: &str = "map_labels.json";

impl Default for Places {
    fn default() -> Self {
        Self::beside_settings()
    }
}

/// Read a labels file's text.
pub(super) fn parse(text: &str) -> Result<Vec<Place>, String> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let list = value.as_array().ok_or("this is not a list of map labels")?;
    let places: Vec<Place> = list
        .iter()
        .filter(|l| l["plane"].as_i64().unwrap_or(0) == 0)
        .filter_map(|l| {
            let name = l["name"].as_str()?.replace("<br>", " ");
            let name = name.trim();
            if name.is_empty() {
                return None;
            }
            Some(Place {
                name: name.to_owned(),
                x: i32::try_from(l["worldX"].as_i64()?).ok()?,
                y: i32::try_from(l["worldY"].as_i64()?).ok()?,
                size: u8::try_from(l["textScale"].as_i64().unwrap_or(0))
                    .unwrap_or(0)
                    .min(2),
            })
        })
        .collect();
    if places.is_empty() {
        return Err("no place with a name and a position was found in that file".into());
    }
    Ok(places)
}

impl Places {
    pub(super) fn beside_settings() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let directory = crate::settings::path()
            .and_then(|file| file.parent().map(|parent| parent.join("labels")));
        #[cfg(target_arch = "wasm32")]
        let directory = None;
        Self {
            items: Vec::new(),
            directory,
            loaded: false,
            problem: None,
        }
    }

    #[cfg(test)]
    pub(super) fn in_directory(directory: PathBuf) -> Self {
        Self {
            items: Vec::new(),
            directory: Some(directory),
            loaded: false,
            problem: None,
        }
    }

    /// Read the saved labels (once).
    pub(super) fn ensure_loaded(&mut self) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        let Some(directory) = &self.directory else {
            return;
        };
        if let Ok(text) = std::fs::read_to_string(directory.join(FILE)) {
            match parse(&text) {
                Ok(items) => self.items = items,
                Err(error) => {
                    self.problem = Some(format!(
                        "The saved place names could not be read ({error})."
                    ));
                }
            }
        }
    }

    /// Take a labels file: check it, keep a copy beside the settings, use it. Returns how many
    /// places it holds.
    pub(super) fn import(&mut self, text: &str) -> Result<usize, String> {
        let items = parse(text)?;
        if let Some(directory) = &self.directory {
            std::fs::create_dir_all(directory)
                .map_err(|e| format!("{}: {e}", directory.display()))?;
            std::fs::write(directory.join(FILE), text).map_err(|e| e.to_string())?;
        }
        self.loaded = true;
        self.problem = None;
        let count = items.len();
        self.items = items;
        Ok(count)
    }

    /// The nearest place of at least `min_size` within `reach` tiles of `(x, y)`.
    pub(super) fn nearest(&self, x: i32, y: i32, min_size: u8, reach: i32) -> Option<&Place> {
        self.items
            .iter()
            .filter(|p| p.size >= min_size)
            .map(|p| (((p.x - x).pow(2) + (p.y - y).pow(2)) as i64, p))
            .filter(|(d, _)| *d <= i64::from(reach).pow(2))
            .min_by_key(|(d, _)| *d)
            .map(|(_, p)| p)
    }

    /// Places whose name contains `query` (towns and regions first), at most `limit`.
    pub(super) fn search(&self, query: &str, limit: usize) -> Vec<&Place> {
        let query = query.trim().to_lowercase();
        let mut hits: Vec<&Place> = self
            .items
            .iter()
            .filter(|p| {
                query.is_empty() && p.size >= 1
                    || !query.is_empty() && p.name.to_lowercase().contains(&query)
            })
            .collect();
        hits.sort_by(|a, b| b.size.cmp(&a.size).then(a.name.cmp(&b.name)));
        hits.truncate(limit);
        hits
    }

    /// The places within `reach` tiles of the rectangle starting at `origin`, for a lesson to keep.
    pub(super) fn marks_near(
        &self,
        origin: [i32; 2],
        size: (i32, i32),
        reach: i32,
    ) -> Vec<PlaceMark> {
        self.items
            .iter()
            .filter(|p| {
                p.x > origin[0] - reach
                    && p.x < origin[0] + size.0 + reach
                    && p.y > origin[1] - reach
                    && p.y < origin[1] + size.1 + reach
            })
            .map(|p| PlaceMark {
                name: p.name.clone(),
                x: p.x,
                y: p.y,
                size: p.size,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"[
        {"name": "Lumbridge", "worldX": 3239, "worldY": 3234, "plane": 0, "textColor": 16777215, "textScale": 1},
        {"name": "Kingdom of<br>Misthalin", "worldX": 3217, "worldY": 3321, "plane": 0, "textColor": 16750623, "textScale": 2},
        {"name": "Lower level", "worldX": 3220, "worldY": 9620, "plane": 1, "textColor": 16777215, "textScale": 0},
        {"name": "Cow field", "worldX": 3250, "worldY": 3270, "plane": 0, "textColor": 16777215, "textScale": 0}
    ]"#;

    #[test]
    fn labels_are_read_searched_and_remembered_beside_the_settings() {
        let dir = std::env::temp_dir().join(format!("rustosrs-places-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut places = Places::in_directory(dir.clone());
        assert_eq!(
            places.import(SAMPLE),
            Ok(3),
            "the dungeon level (plane 1) is left out"
        );
        assert_eq!(
            places.items[1].name, "Kingdom of Misthalin",
            "line breaks become spaces"
        );
        assert_eq!(
            places.nearest(3228, 3219, 1, 60).map(|p| p.name.as_str()),
            Some("Lumbridge")
        );
        assert!(places.nearest(3228, 3219, 1, 5).is_none());
        assert_eq!(places.search("cow", 5).len(), 1);
        assert_eq!(
            places.search("", 5).first().map(|p| p.size),
            Some(2),
            "regions first"
        );
        assert_eq!(places.marks_near([3200, 3200], (64, 64), 100).len(), 3);
        let mut again = Places::in_directory(dir.clone());
        again.ensure_loaded();
        assert_eq!(again.items, places.items, "a copy was kept");
        assert!(places.import("{}").is_err() && places.import("[]").is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
