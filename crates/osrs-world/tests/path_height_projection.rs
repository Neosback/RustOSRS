//! Real-cache verification of absolute editor heights across a region boundary and a rebuild.
use osrs_world::{SceneWindow, TileEdit, WorldDefinitions, load_window_terrain};

#[test]
#[ignore = "requires the pinned build-241 cache (RUSTOSRS_TARGET_CACHE_DIR)"]
fn path_height_precision_survives_window_loading_and_rebuild_across_region_edges()
-> Result<(), Box<dyn std::error::Error>> {
    let mut definitions = WorldDefinitions::open(std::env::var("RUSTOSRS_TARGET_CACHE_DIR")?)?;
    let window = SceneWindow::new(3176, 3176).ok_or("window")?;
    let before = load_window_terrain(&definitions, window)?;
    let mut edits = Vec::new();
    for x in [3263, 3264] {
        for plane in 0..4 {
            edits.push(TileEdit {
                plane,
                tile: (x, 3210),
                underlay: 1,
                overlay: 1,
                shape: 0,
                rotation: 0,
                flags: 0,
                height: Some(-53 - i32::from(plane) * 161),
            });
        }
    }
    definitions.apply_edits(&edits);
    for _ in 0..2 {
        let grid = load_window_terrain(&definitions, window)?;
        for edit in &edits {
            assert_eq!(
                grid.height(
                    usize::from(edit.plane),
                    (edit.tile.0 - window.base_x) as usize,
                    (edit.tile.1 - window.base_y) as usize
                ),
                edit.height.unwrap_or_else(|| unreachable!("valid fixture"))
            );
        }
        assert_eq!(grid.height(0, 80, 34), before.height(0, 80, 34));
    }
    Ok(())
}
