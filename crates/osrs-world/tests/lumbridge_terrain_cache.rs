//! End-to-end cache -> terrain scene check against the pinned-client oracle fixture.
//!
//! Requires the pinned build-241 cache: `RUSTOSRS_TARGET_CACHE_DIR=... cargo test -p osrs-world
//! -- --ignored`. The cache path reads regions through `osrs-cache` itself (no exported input),
//! so this also proves the cache -> `RegionTerrain` conversion.

use osrs_core::color_palette::build_color_palette;
use osrs_core::floor_color::UnderlayHsl;
use osrs_scene::{
    SceneGrid,
    terrain_build::{FloorLookup, OverlayFloor, TerrainJitter, build_terrain, link_bridge_tiles},
};
use osrs_world::{
    FloorTable, SceneWindow, WorldDefinitions, load_window_terrain, oracle_dump::terrain_body_lines,
};

const ORACLE_OUTPUT: &str =
    include_str!("../../../reference-fixtures/terrain/lumbridge-3x3.oracle-out");

/// The fixture oracle used a deterministic texture-average stub, not real texture data.
struct StubTextureFloors<'a>(&'a FloorTable);

impl FloorLookup for StubTextureFloors<'_> {
    fn underlay(&self, index: u32) -> UnderlayHsl {
        self.0.underlay(index)
    }

    fn overlay(&self, index: u32) -> OverlayFloor {
        self.0.overlay(index)
    }

    fn texture_average_rgb(&self, texture_id: i32) -> i32 {
        (texture_id * 257) & 0xFFFF
    }
}

#[test]
#[ignore = "requires the pinned build-241 cache (RUSTOSRS_TARGET_CACHE_DIR)"]
fn cache_built_lumbridge_terrain_matches_pinned_client_oracle()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::var("RUSTOSRS_TARGET_CACHE_DIR")?;
    let definitions = WorldDefinitions::open(dir)?;
    let window = SceneWindow::new(3176, 3176).ok_or("window alignment")?;
    let load = load_window_terrain(&definitions, window)?;

    let header = ORACLE_OUTPUT.lines().next().ok_or("empty fixture")?;
    let number = |key: &str| -> Result<i32, Box<dyn std::error::Error>> {
        Ok(header
            .split(key)
            .nth(1)
            .and_then(|rest| rest.split_whitespace().next())
            .ok_or("missing jitter")?
            .parse()?)
    };
    let jitter = TerrainJitter {
        hue: number("hue=")?,
        lightness: number("lightness=")?,
    };

    let palette = build_color_palette(0.8);
    let floors = StubTextureFloors(definitions.floors());
    let output = build_terrain(&load, &floors, jitter, &palette)?;
    let mut scene = SceneGrid::new(104, 104, 4)?;
    output.apply_to_scene(&load, &mut scene)?;
    link_bridge_tiles(&load, &mut scene)?;

    let expected: Vec<&str> = ORACLE_OUTPUT.lines().skip(2).collect();
    let actual = terrain_body_lines(&load, &scene);
    for (index, (actual_line, expected_line)) in actual.iter().zip(expected.iter()).enumerate() {
        assert_eq!(actual_line, expected_line, "oracle body line {index}");
    }
    assert_eq!(actual.len(), expected.len());
    Ok(())
}
