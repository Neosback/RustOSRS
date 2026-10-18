//! A searchable list of the objects an editor can place, read from the cache's object configs.
//!
//! Every definition with at least one model is listed, including unnamed (`"null"`) architectural
//! walls, roofs, and trims so the editor and procedural tools can inspect and place them by ID,
//! gameval symbol, slot, category, or mapScene.

use crate::WorldError;
use osrs_cache::{
    decode::DecoderContext, decode::decode_object_definition, transport::CacheRepository,
};
use osrs_core::{definitions::ObjectModels, ids::ObjectId};

const CONFIG_INDEX: u8 = 2;
const OBJECT_GROUP: u32 = 6;
/// The loc type of ordinary scenery (trees, furniture, statues).
pub const SCENERY_LOC_TYPE: u8 = 10;

/// One placeable object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectEntry {
    pub id: u32,
    pub name: String,
    /// Readable gameval symbol (`castle door l`) when populated from an installation's `NameBook`.
    pub symbol: Option<String>,
    pub size_x: u16,
    pub size_y: u16,
    /// Loc types the definition has models for, ascending (untyped models count as scenery).
    pub loc_types: Vec<u8>,
    pub category: Option<u32>,
    pub map_scene: Option<u32>,
    pub map_icon: Option<u32>,
    pub support_items: Option<u8>,
    pub blocking_mask: u8,
}

impl ObjectEntry {
    /// The loc type to place by default: scenery when available, else the lowest.
    pub fn default_loc_type(&self) -> u8 {
        if self.loc_types.contains(&SCENERY_LOC_TYPE) {
            SCENERY_LOC_TYPE
        } else {
            self.loc_types.first().copied().unwrap_or(SCENERY_LOC_TYPE)
        }
    }

    /// Footprint in tiles for an orientation (quarter turns swap the axes).
    pub fn footprint(&self, orientation: u8) -> (u16, u16) {
        if orientation & 1 == 1 {
            (self.size_y, self.size_x)
        } else {
            (self.size_x, self.size_y)
        }
    }

    /// Human-readable label for UI lists: cache name, else `[symbol]`, else `Object <id>`.
    pub fn display_name(&self) -> String {
        if !self.name.is_empty() {
            self.name.clone()
        } else if let Some(symbol) = &self.symbol {
            format!("[{symbol}]")
        } else {
            format!("Object {}", self.id)
        }
    }

    /// Whether any model of this definition occupies the named slot family.
    pub fn matches_slot(&self, slot: &str) -> bool {
        match slot {
            "wall" => self.loc_types.contains(&0),
            "corner" | "wallcorner" => self.loc_types.contains(&2),
            "pillar" => self.loc_types.iter().any(|&t| t == 1 || t == 3),
            "decor" | "walldecor" => self.loc_types.iter().any(|&t| (4..=8).contains(&t)),
            "diagonal" => self.loc_types.contains(&9),
            "prop" | "scenery" => self.loc_types.iter().any(|&t| t == 10 || t == 11),
            "roof" => self.loc_types.iter().any(|&t| (12..=21).contains(&t)),
            "floor" | "floordecor" => self.loc_types.contains(&22),
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObjectCatalog {
    entries: Vec<ObjectEntry>,
}

impl ObjectCatalog {
    pub fn from_entries(mut entries: Vec<ObjectEntry>) -> Self {
        entries.sort_by_key(|entry| entry.id);
        Self { entries }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> &[ObjectEntry] {
        &self.entries
    }

    pub fn get(&self, id: u32) -> Option<&ObjectEntry> {
        self.entries
            .binary_search_by_key(&id, |entry| entry.id)
            .ok()
            .map(|index| &self.entries[index])
    }

    /// Attach readable gameval symbols (`loc.*`) to matching entries in place.
    pub fn apply_symbols(&mut self, symbols: impl Fn(u32) -> Option<String>) {
        for entry in &mut self.entries {
            if let Some(symbol) = symbols(entry.id).filter(|s| !s.is_empty()) {
                entry.symbol = Some(symbol);
            }
        }
    }

    /// Up to `limit` entries matching `query`: an exact id first, then names starting with the
    /// query, then names containing it, then symbol/filter matches (case-insensitive, every word
    /// and filter token must match). Supports `category:<id>`, `scene:<id>`, `icon:<id>`, and
    /// `slot:<wall|corner|pillar|decor|diagonal|prop|roof|floor>`. An empty query matches nothing.
    pub fn search(&self, query: &str, limit: usize) -> Vec<&ObjectEntry> {
        let query = query.trim().to_lowercase();
        if query.is_empty() || limit == 0 {
            return Vec::new();
        }
        let mut found: Vec<&ObjectEntry> = Vec::new();
        if let Some(entry) = query.parse::<u32>().ok().and_then(|id| self.get(id)) {
            found.push(entry);
        }

        let mut text_words: Vec<&str> = Vec::new();
        let mut req_category: Option<u32> = None;
        let mut req_scene: Option<u32> = None;
        let mut req_icon: Option<u32> = None;
        let mut req_slot: Option<&str> = None;

        for token in query.split_whitespace() {
            if let Some(val) = token
                .strip_prefix("category:")
                .or_else(|| token.strip_prefix("cat:"))
            {
                let Ok(id) = val.parse::<u32>() else {
                    return found;
                };
                req_category = Some(id);
            } else if let Some(val) = token.strip_prefix("scene:") {
                let Ok(id) = val.parse::<u32>() else {
                    return found;
                };
                req_scene = Some(id);
            } else if let Some(val) = token.strip_prefix("icon:") {
                let Ok(id) = val.parse::<u32>() else {
                    return found;
                };
                req_icon = Some(id);
            } else if let Some(val) = token.strip_prefix("slot:") {
                req_slot = Some(val);
            } else {
                text_words.push(token);
            }
        }

        let text_query = text_words.join(" ");
        let mut prefix = Vec::new();
        let mut contains = Vec::new();
        let mut fallback = Vec::new();

        for entry in &self.entries {
            if found.iter().any(|f| f.id == entry.id) {
                continue;
            }
            if let Some(cat) = req_category
                && entry.category != Some(cat)
            {
                continue;
            }
            if let Some(scene) = req_scene
                && entry.map_scene != Some(scene)
            {
                continue;
            }
            if let Some(icon) = req_icon
                && entry.map_icon != Some(icon)
            {
                continue;
            }
            if let Some(slot) = req_slot
                && !entry.matches_slot(slot)
            {
                continue;
            }

            if text_words.is_empty() {
                if !entry.name.is_empty() {
                    contains.push(entry);
                } else {
                    fallback.push(entry);
                }
                continue;
            }

            let name = entry.name.to_lowercase();
            if !name.is_empty() && text_words.iter().all(|word| name.contains(word)) {
                if name.starts_with(&text_query) {
                    prefix.push(entry);
                } else {
                    contains.push(entry);
                }
            } else if let Some(symbol) = &entry.symbol {
                let sym = symbol.to_lowercase();
                if text_words.iter().all(|word| sym.contains(word)) {
                    fallback.push(entry);
                }
            }
        }
        found.extend(prefix);
        found.extend(contains);
        found.extend(fallback);
        found.truncate(limit);
        found
    }
}

/// Read every model-bearing object from the cache.
pub fn load_object_catalog(
    cache: impl Into<osrs_cache::source::CacheSource>,
) -> Result<ObjectCatalog, WorldError> {
    let profiles = osrs_cache::registry::bundled_profiles()?;
    let (cache, profile) = CacheRepository::open_matching(cache, &profiles)?;
    let context = DecoderContext::from_profile(&profile)?;
    let mut entries = Vec::new();
    for (id, file) in cache.read_group_files(CONFIG_INDEX, OBJECT_GROUP)? {
        let Ok(definition) =
            decode_object_definition(ObjectId::new(id), &file.bytes, &context, &file.provenance)
        else {
            continue;
        };
        let name = definition
            .name
            .filter(|name| !name.is_empty() && !name.eq_ignore_ascii_case("null"))
            .unwrap_or_default();
        let mut loc_types: Vec<u8> = match &definition.models {
            Some(ObjectModels::Typed(models)) if !models.is_empty() => {
                models.iter().map(|model| model.loc_type.get()).collect()
            }
            Some(ObjectModels::Untyped(models)) if !models.is_empty() => vec![SCENERY_LOC_TYPE],
            _ => continue,
        };
        loc_types.sort_unstable();
        loc_types.dedup();
        entries.push(ObjectEntry {
            id,
            name,
            symbol: None,
            size_x: definition.size_x.max(1),
            size_y: definition.size_y.max(1),
            loc_types,
            category: definition.category.map(|c| c.get()),
            map_scene: definition.map_scene.map(|s| s.get()),
            map_icon: definition.map_icon.map(|i| i.get()),
            support_items: definition.support_items,
            blocking_mask: definition.blocking_mask,
        });
    }
    Ok(ObjectCatalog::from_entries(entries))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: u32, name: &str, loc_types: &[u8]) -> ObjectEntry {
        ObjectEntry {
            id,
            name: name.to_owned(),
            symbol: None,
            size_x: 2,
            size_y: 1,
            loc_types: loc_types.to_vec(),
            category: None,
            map_scene: None,
            map_icon: None,
            support_items: None,
            blocking_mask: 0,
        }
    }

    fn catalog() -> ObjectCatalog {
        let mut unnamed_wall = entry(99, "", &[0]);
        unnamed_wall.map_scene = Some(12);
        unnamed_wall.category = Some(207);
        ObjectCatalog::from_entries(vec![
            entry(30, "Oak tree", &[10]),
            entry(10, "Tree", &[10]),
            entry(20, "Dead tree", &[10]),
            entry(5, "Wooden door", &[0, 9]),
            entry(7, "Bank booth", &[10]),
            unnamed_wall,
        ])
    }

    #[test]
    fn search_ranks_exact_id_then_prefix_then_contains() {
        let catalog = catalog();
        let names = |query: &str| -> Vec<String> {
            catalog
                .search(query, 10)
                .into_iter()
                .map(|e| e.name.clone())
                .collect()
        };
        assert_eq!(names("tree"), vec!["Tree", "Dead tree", "Oak tree"]);
        assert_eq!(names("10"), vec!["Tree"]);
        assert_eq!(names("wooden door"), vec!["Wooden door"]);
        assert_eq!(names("DOOR"), vec!["Wooden door"]);
        assert!(names("   ").is_empty());
        assert!(names("zzz").is_empty());
    }

    #[test]
    fn search_respects_the_limit_and_id_lookup_does_not_duplicate() {
        let catalog = catalog();
        assert_eq!(catalog.search("tree", 2).len(), 2);
        assert_eq!(catalog.get(7).map(|e| e.name.as_str()), Some("Bank booth"));
        assert_eq!(catalog.get(8), None);
        // "5" matches id 5 by id; it must not also appear as a name hit.
        let hits = catalog.search("5", 10);
        assert_eq!(hits.iter().filter(|e| e.id == 5).count(), 1);
    }

    #[test]
    fn default_loc_type_prefers_scenery_and_footprint_turns_with_orientation() {
        assert_eq!(entry(1, "A", &[0, 9, 10]).default_loc_type(), 10);
        assert_eq!(entry(1, "A", &[2, 9]).default_loc_type(), 2);
        let e = entry(1, "A", &[10]);
        assert_eq!(e.footprint(0), (2, 1));
        assert_eq!(e.footprint(1), (1, 2));
        assert_eq!(e.footprint(2), (2, 1));
    }

    #[test]
    fn unnamed_architectural_entries_match_by_id_symbol_slot_and_metadata() {
        let mut catalog = catalog();
        assert_eq!(
            catalog.get(99).map(ObjectEntry::display_name),
            Some("Object 99".to_owned())
        );
        assert_eq!(
            catalog
                .search("99", 5)
                .into_iter()
                .map(|e| e.id)
                .collect::<Vec<_>>(),
            vec![99]
        );
        assert_eq!(
            catalog
                .search("slot:wall", 5)
                .into_iter()
                .map(|e| e.id)
                .collect::<Vec<_>>(),
            vec![5, 99]
        );
        assert_eq!(
            catalog
                .search("scene:12 category:207", 5)
                .into_iter()
                .map(|e| e.id)
                .collect::<Vec<_>>(),
            vec![99]
        );
        catalog.apply_symbols(|id| (id == 99).then(|| "stone wall straight".to_owned()));
        assert_eq!(
            catalog.get(99).map(ObjectEntry::display_name),
            Some("[stone wall straight]".to_owned())
        );
        assert_eq!(
            catalog
                .search("stone wall", 5)
                .into_iter()
                .map(|e| e.id)
                .collect::<Vec<_>>(),
            vec![99]
        );
    }
}
