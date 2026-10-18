//! The names of the objects in some buildings, as the building census and the trainer read them:
//! the game's name, plus anything a person said (a name, or what the object is).

use crate::app::EditorApp;
use osrs_core::ids::ObjectId;
use osrs_edit::Prefab;
use osrs_scene::building_census::{ObjectInfo, ObjectTable};
use std::collections::BTreeSet;

/// Names for every object used by `prefabs`. Also gives the cache name and the game's internal
/// name for each id, for remembering what the object was called when it was named.
pub(super) fn table_for(
    app: &EditorApp,
    prefabs: &[&Prefab],
) -> (ObjectTable, Vec<(u32, String, String)>) {
    let ids: BTreeSet<u32> = prefabs
        .iter()
        .flat_map(|p| p.objects.iter().map(|o| o.id))
        .collect();
    let book = app.names.custom().book();
    let mut table = ObjectTable::new();
    let mut raw = Vec::new();
    for id in ids {
        let entry = app.placer.catalog_entry(id);
        let cache_name = entry.map(|e| e.name.clone()).unwrap_or_default();
        let symbol = app
            .names
            .book()
            .and_then(|b| b.object(id))
            .unwrap_or_default()
            .to_owned();
        let (name, sym) = osrs_edit::census_name(book, id, &cache_name, &symbol);
        table.insert(
            ObjectId::new(id),
            ObjectInfo {
                name,
                symbol: sym,
                actions: Vec::new(),
                size: entry.map_or((1, 1), |e| {
                    (e.size_x.min(255) as u8, e.size_y.min(255) as u8)
                }),
            },
        );
        raw.push((id, cache_name, symbol));
    }
    (table, raw)
}
