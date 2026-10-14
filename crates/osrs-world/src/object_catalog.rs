//! A searchable list of the objects an editor can place, read from the cache's object configs.
//!
//! Only objects with a name and at least one model are listed: unnamed or model-less definitions
//! (morph shells, placeholders) cannot be meaningfully placed from a palette.

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
    pub size_x: u16,
    pub size_y: u16,
    /// Loc types the definition has models for, ascending (untyped models count as scenery).
    pub loc_types: Vec<u8>,
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

    pub fn get(&self, id: u32) -> Option<&ObjectEntry> {
        self.entries
            .binary_search_by_key(&id, |entry| entry.id)
            .ok()
            .map(|index| &self.entries[index])
    }

    /// Up to `limit` entries matching `query`: an exact id first, then names starting with the
    /// query, then names containing it (case-insensitive, every word must match). An empty query
    /// matches nothing.
    pub fn search(&self, query: &str, limit: usize) -> Vec<&ObjectEntry> {
        let query = query.trim().to_lowercase();
        if query.is_empty() || limit == 0 {
            return Vec::new();
        }
        let mut found: Vec<&ObjectEntry> = Vec::new();
        if let Some(entry) = query.parse::<u32>().ok().and_then(|id| self.get(id)) {
            found.push(entry);
        }
        let words: Vec<&str> = query.split_whitespace().collect();
        let mut prefix = Vec::new();
        let mut contains = Vec::new();
        for entry in &self.entries {
            let name = entry.name.to_lowercase();
            if !words.iter().all(|word| name.contains(word))
                || found.iter().any(|f| f.id == entry.id)
            {
                continue;
            }
            if name.starts_with(&query) {
                prefix.push(entry);
            } else {
                contains.push(entry);
            }
        }
        found.extend(prefix);
        found.extend(contains);
        found.truncate(limit);
        found
    }
}

/// Read every placeable object from the cache.
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
        let Some(name) = definition
            .name
            .filter(|name| !name.is_empty() && name != "null")
        else {
            continue;
        };
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
            size_x: definition.size_x.max(1),
            size_y: definition.size_y.max(1),
            loc_types,
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
            size_x: 2,
            size_y: 1,
            loc_types: loc_types.to_vec(),
        }
    }

    fn catalog() -> ObjectCatalog {
        ObjectCatalog::from_entries(vec![
            entry(30, "Oak tree", &[10]),
            entry(10, "Tree", &[10]),
            entry(20, "Dead tree", &[10]),
            entry(5, "Wooden door", &[0, 9]),
            entry(7, "Bank booth", &[10]),
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
}
