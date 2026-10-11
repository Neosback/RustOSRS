//! World map and minimap against the pinned build-241 cache.
//!
//! Requires `RUSTOSRS_TARGET_CACHE_DIR=... cargo test -p osrs-world -- --ignored`.

use osrs_world::{
    WorldDefinitions, minimap,
    worldmap::{Catalog, RegionRenderer},
};

fn definitions() -> Result<WorldDefinitions, Box<dyn std::error::Error>> {
    let dir = std::env::var("RUSTOSRS_TARGET_CACHE_DIR")?;
    Ok(WorldDefinitions::open(dir)?)
}

#[test]
#[ignore = "needs the pinned cache"]
fn catalog_reads_all_areas_and_maps_coordinates() -> Result<(), Box<dyn std::error::Error>> {
    let defs = definitions()?;
    let catalog = Catalog::load(&defs)?;
    assert_eq!(catalog.areas.len(), 52);
    let main = &catalog.areas[catalog.main_area()];
    assert_eq!(main.external_name, "Gielinor Surface");
    // World -> map -> world is the identity on the surface.
    let (x, y) = main
        .position(0, 3222, 3218)
        .ok_or("Lumbridge is on the surface map")?;
    let back = main.world_coord(x, y).ok_or("tile maps back")?;
    assert_eq!((back.x, back.y), (3222, 3218));
    assert!(catalog.elements.len() > 1000);
    Ok(())
}

#[test]
#[ignore = "needs the pinned cache"]
fn region_renders_ground_and_walls() -> Result<(), Box<dyn std::error::Error>> {
    let mut defs = definitions()?;
    let catalog = Catalog::load(&defs)?;
    let mut renderer = RegionRenderer::new();
    let image = renderer
        .render(&mut defs, &catalog, catalog.main_area(), (50, 50), 4)
        .ok_or("Lumbridge has map data")?;
    assert_eq!(image.rgba.len(), 256 * 256 * 4);
    // Every tile is painted; the pale wall lines and red doors exist.
    assert!(image.rgba.chunks(4).all(|pixel| pixel[3] == 255));
    assert!(
        image
            .rgba
            .chunks(4)
            .any(|pixel| pixel[..3] == [0xCC, 0xCC, 0xCC])
    );
    // An ocean-only region has no data.
    assert!(
        renderer
            .render(&mut defs, &catalog, catalog.main_area(), (0, 0), 4)
            .is_none()
    );
    Ok(())
}

#[test]
#[ignore = "needs the pinned cache"]
fn minimap_region_has_terrain_and_icons() -> Result<(), Box<dyn std::error::Error>> {
    let mut defs = definitions()?;
    let catalog = Catalog::load(&defs)?;
    let region = minimap::render_region(&mut defs, &catalog, (50, 50), 0)?.ok_or("data")?;
    assert_eq!(
        region.rgba.len(),
        minimap::REGION_PX * minimap::REGION_PX * 4
    );
    assert!(region.rgba.chunks(4).filter(|pixel| pixel[3] != 0).count() > 60_000);
    assert!(!region.icons.is_empty());
    Ok(())
}
