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
    let catalog = load_object_catalog(CacheSource::Dir(PathBuf::from(dir)))?;
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
    Ok(())
}
