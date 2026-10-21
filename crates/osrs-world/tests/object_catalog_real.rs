//! The object catalog against the real pinned cache (skipped without `RUSTOSRS_TARGET_CACHE_DIR`).

use osrs_cache::source::CacheSource;
use osrs_world::{SCENERY_LOC_TYPE, load_object_catalog};
use std::path::PathBuf;

#[test]
fn the_real_cache_lists_thousands_of_named_placeable_objects()
-> Result<(), Box<dyn std::error::Error>> {
    let Some(dir) = std::env::var_os("RUSTOSRS_TARGET_CACHE_DIR") else {
        return Ok(());
    };
    let source = CacheSource::Dir(PathBuf::from(dir));
    let catalog = load_object_catalog(source.clone())?;
    assert!(
        catalog.len() > 10_000,
        "only {} objects listed",
        catalog.len()
    );

    let trees = catalog.search("oak tree", 50);
    assert!(!trees.is_empty(), "no oak trees found");
    assert!(
        trees
            .iter()
            .all(|entry| entry.name.to_lowercase().contains("oak"))
    );
    assert!(trees.iter().all(|entry| !entry.loc_types.is_empty()));
    assert!(
        trees
            .iter()
            .any(|entry| entry.default_loc_type() == SCENERY_LOC_TYPE),
        "an oak tree is scenery"
    );
    // Every listed entry has a usable footprint.
    for entry in catalog.search("door", 200) {
        assert!(entry.size_x >= 1 && entry.size_y >= 1);
    }
    // Definitions without direct models remain absent from search, but planning must still be
    // able to resolve their actual dimensions. Compare an unlisted multi-tile definition with
    // the canonical definition loader rather than assuming every absence means one tile.
    let upper = catalog.entries().last().ok_or("empty catalog")?.id;
    let (id, size) = (0..upper)
        .find_map(|id| {
            let size = catalog.definition_size(id)?;
            (catalog.get(id).is_none() && (size.0 > 1 || size.1 > 1)).then_some((id, size))
        })
        .ok_or("pinned cache has no unlisted multi-tile definition")?;
    let mut definitions = osrs_world::WorldDefinitions::open(source)?;
    let definition = definitions
        .object(osrs_core::ids::ObjectId::new(id))?
        .ok_or("definition")?;
    assert_eq!(
        (u16::from(size.0), u16::from(size.1)),
        (definition.size_x.max(1), definition.size_y.max(1))
    );
    assert!(catalog.search(&id.to_string(), 10).is_empty());
    eprintln!("unlisted definition {id}: {size:?}");
    Ok(())
}
