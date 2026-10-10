//! Full cache -> world scene assembly on the real Lumbridge window.
//!
//! `RUSTOSRS_TARGET_CACHE_DIR=... cargo test --release -p osrs-world -- --ignored --nocapture`

use osrs_world::{SceneWindow, TerrainPresentation, WorldDefinitions, build_world_scene};

#[test]
#[ignore = "requires the pinned build-241 cache (RUSTOSRS_TARGET_CACHE_DIR)"]
fn lumbridge_window_assembles_end_to_end() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::var("RUSTOSRS_TARGET_CACHE_DIR")?;
    let mut definitions = WorldDefinitions::open(dir)?;
    let window = SceneWindow::new(3176, 3176).ok_or("window alignment")?;
    let scene = build_world_scene(&mut definitions, window, TerrainPresentation::default())?;

    println!("loc stats: {:?}", scene.loc_stats);
    println!("merge report: {:?}", scene.merge_report);
    println!("lit flat models: {}", scene.lit.len());
    println!("locs recorded: {}", scene.locs.len());

    assert!(scene.loc_stats.decoded > 1_000);
    assert!(scene.loc_stats.placed > 500);
    assert!(!scene.lit.is_empty());
    Ok(())
}
