//! `TERRAIN-004` differential test against the pinned client terrain oracle.
//!
//! The fixture pair under `reference-fixtures/terrain/` was produced by
//! `tools/deob-harness/run-terrain-oracle.sh`, which runs the real `class264.loadTerrain`,
//! `ScriptFrame.method749`, and `class470.method9712` from `melxin/runelite@1ad572d7...` on real
//! build-241 Lumbridge-area terrain. The Rust loader/builder must reproduce every height, setting,
//! tile paint/model, minimum plane, and link-below tile byte-for-byte.

use osrs_cache::decode::{
    ArchiveFileProvenance, DecoderContext, EncodedTileHeight, decode_floor_overlay,
    decode_floor_underlay, decode_terrain,
};
use osrs_cache::profile::TargetProfile;
use osrs_core::color_palette::build_color_palette;
use osrs_core::coords::{RegionCoord, SceneTile, StoragePlane};
use osrs_core::floor_color::UnderlayHsl;
use osrs_core::ids::{FloorOverlayId, FloorUnderlayId};
use osrs_scene::terrain::TerrainSurface;
use osrs_scene::terrain_build::{
    FloorLookup, OverlayFloor, TerrainJitter, build_terrain, link_bridge_tiles,
};
use osrs_scene::terrain_load::{
    RegionTerrain, RegionTerrainTile, RegionTileHeight, RegionTileOverlay, TerrainLoadGrid,
};
use osrs_scene::{SceneGrid, SemanticTile};
use std::collections::HashMap;
use std::fmt::Write as _;

const PROFILE_YAML: &str =
    include_str!("../../../profiles/osrs-live-241-2026-09-30-openrs2-2727.yaml");
const ORACLE_INPUT: &str =
    include_str!("../../../reference-fixtures/terrain/lumbridge-3x3.oracle-in");
const ORACLE_OUTPUT: &str =
    include_str!("../../../reference-fixtures/terrain/lumbridge-3x3.oracle-out");

type TestResult = Result<(), Box<dyn std::error::Error>>;

struct Floors {
    underlays: HashMap<u32, UnderlayHsl>,
    overlays: HashMap<u32, OverlayFloor>,
    texture_averages: HashMap<i32, i32>,
}

impl FloorLookup for Floors {
    fn underlay(&self, index: u32) -> UnderlayHsl {
        self.underlays.get(&index).copied().unwrap_or(UnderlayHsl {
            weighted_hue: 0,
            saturation: 0,
            lightness: 0,
            hue_multiplier: 1,
        })
    }

    fn overlay(&self, index: u32) -> OverlayFloor {
        self.overlays
            .get(&index)
            .copied()
            .unwrap_or_else(OverlayFloor::missing)
    }

    fn texture_average_rgb(&self, texture_id: i32) -> i32 {
        self.texture_averages
            .get(&texture_id)
            .copied()
            .unwrap_or((texture_id * 257) & 0xFFFF)
    }
}

fn hex(text: &str) -> Result<Vec<u8>, std::num::ParseIntError> {
    (0..text.len() / 2)
        .map(|index| u8::from_str_radix(&text[index * 2..index * 2 + 2], 16))
        .collect()
}

fn region_terrain(
    bytes: &[u8],
    context: &DecoderContext,
) -> Result<RegionTerrain, Box<dyn std::error::Error>> {
    let source = ArchiveFileProvenance::new(5, 0, Some(0));
    let decoded = decode_terrain(bytes, context, &source, RegionCoord::new(0, 0))?;
    let tiles = decoded
        .tiles()
        .iter()
        .map(|tile| RegionTerrainTile {
            height: match tile.height {
                EncodedTileHeight::Default => RegionTileHeight::Default,
                EncodedTileHeight::Explicit(value) => RegionTileHeight::Explicit(value),
            },
            overlay: tile.overlay.map(|overlay| RegionTileOverlay {
                raw_id: overlay.encoded_id,
                shape: overlay.shape,
                rotation: overlay.rotation,
            }),
            settings: (tile.settings != 0).then_some(tile.settings),
            underlay: tile.underlay_id,
        })
        .collect();
    Ok(RegionTerrain::new(tiles)?)
}

fn join<T: std::fmt::Display>(values: impl IntoIterator<Item = T>) -> String {
    let mut out = String::new();
    for (index, value) in values.into_iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        let _ = write!(out, "{value}");
    }
    out
}

fn format_tile(tag: &str, plane: u8, x: u32, y: u32, tile: &SemanticTile) -> String {
    let mut line = format!(
        "{tag} {plane} {x} {y} plane={} orig={} min={} linked={}",
        tile.storage_plane().index().get(),
        tile.original_plane().index().get(),
        tile.min_plane(),
        u8::from(tile.linked_below().is_some())
    );
    match &tile.terrain {
        Some(TerrainSurface::Flat(flat)) => {
            let _ = write!(
                line,
                " paint={},{},{},{} tex={} rgb={} flat={}",
                flat.colors.southwest,
                flat.colors.southeast,
                flat.colors.northeast,
                flat.colors.northwest,
                flat.texture_id.unwrap_or(-1),
                flat.rgb,
                u8::from(flat.is_flat)
            );
        }
        Some(TerrainSurface::Shaped(model)) => {
            let textured = model.faces.iter().any(|face| face.texture_id.is_some());
            let _ = write!(
                line,
                " model shape={} rot={} flat={} under={} over={} vx={} vy={} vz={} ca={} cb={} cc={} fx={} fy={} fz={} ft={}",
                model.shape,
                model.rotation,
                u8::from(model.is_flat),
                model.underlay_rgb,
                model.overlay_rgb,
                join(model.vertices.iter().map(|v| v.position.x.units())),
                join(model.vertices.iter().map(|v| v.position.y.units())),
                join(model.vertices.iter().map(|v| v.position.z.units())),
                join(model.faces.iter().map(|f| f.colors[0])),
                join(model.faces.iter().map(|f| f.colors[1])),
                join(model.faces.iter().map(|f| f.colors[2])),
                join(model.faces.iter().map(|f| f.indices[0])),
                join(model.faces.iter().map(|f| f.indices[1])),
                join(model.faces.iter().map(|f| f.indices[2])),
                if textured {
                    join(model.faces.iter().map(|f| f.texture_id.unwrap_or(-1)))
                } else {
                    "-".to_owned()
                },
            );
        }
        None => {}
    }
    line
}

#[test]
fn terrain_loader_and_builder_match_pinned_client_oracle() -> TestResult {
    let profile = TargetProfile::from_yaml_str(PROFILE_YAML)?;
    let context = DecoderContext::from_profile(&profile)?;

    let mut brightness = 0.8_f64;
    let mut noise = (0, 0);
    let mut rotation = 0_u8;
    let mut floors = Floors {
        underlays: HashMap::new(),
        overlays: HashMap::new(),
        texture_averages: HashMap::new(),
    };
    let mut lands: Vec<(i32, i32, RegionTerrain)> = Vec::new();
    let mut empties: Vec<(i32, i32)> = Vec::new();
    let mut shadows: Vec<(usize, usize, usize, u8)> = Vec::new();

    for line in ORACLE_INPUT.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        match parts[0] {
            "brightness" => brightness = parts[1].parse()?,
            "rotation" => rotation = parts[1].parse()?,
            "noise" => noise = (parts[1].parse()?, parts[2].parse()?),
            "texavg" => {
                floors
                    .texture_averages
                    .insert(parts[1].parse()?, parts[2].parse()?);
            }
            "underlay" => {
                let id: u32 = parts[1].parse()?;
                let source = ArchiveFileProvenance::new(2, 1, Some(id));
                let definition = decode_floor_underlay(
                    FloorUnderlayId::new(id),
                    &hex(parts[2])?,
                    &context,
                    &source,
                )?;
                floors.underlays.insert(id, definition.hsl);
            }
            "overlay" => {
                let id: u32 = parts[1].parse()?;
                let source = ArchiveFileProvenance::new(2, 4, Some(id));
                let definition = decode_floor_overlay(
                    FloorOverlayId::new(id),
                    &hex(parts[2])?,
                    &context,
                    &source,
                )?;
                floors.overlays.insert(id, OverlayFloor::from(&definition));
            }
            "land" => lands.push((
                parts[1].parse()?,
                parts[2].parse()?,
                region_terrain(&hex(parts[3])?, &context)?,
            )),
            "empty" => empties.push((parts[1].parse()?, parts[2].parse()?)),
            "shadow" => shadows.push((
                parts[1].parse()?,
                parts[2].parse()?,
                parts[3].parse()?,
                parts[4].parse()?,
            )),
            other => return Err(format!("unknown oracle input record {other}").into()),
        }
    }

    let mut grid = TerrainLoadGrid::reference();
    for (scene_x, scene_y, terrain) in &lands {
        grid.load_region(terrain, (*scene_x, *scene_y), noise, rotation);
    }
    for (scene_x, scene_y) in &empties {
        grid.fill_missing_region(*scene_x, *scene_y, 64, 64);
    }
    for (plane, x, y, value) in &shadows {
        grid.set_shadow(*plane, *x, *y, *value);
    }

    // The oracle reports the jitter the client's random walk actually produced.
    let jitter_line = ORACLE_OUTPUT.lines().next().ok_or("empty oracle output")?;
    let jitter = parse_jitter(jitter_line)?;

    let palette = build_color_palette(brightness);
    let output = build_terrain(&grid, &floors, jitter, &palette)?;
    let mut scene = SceneGrid::new(104, 104, 4)?;
    output.apply_to_scene(&grid, &mut scene)?;
    link_bridge_tiles(&grid, &mut scene)?;

    let mut expected_lines = ORACLE_OUTPUT.lines();
    let _jitter_line = expected_lines.next();
    let _min_plane_line = expected_lines.next();

    // Heights.
    for plane in 0..4 {
        for x in 0..105 {
            let expected = expected_lines.next().ok_or("oracle ended in heights")?;
            let actual = format!(
                "h {plane} {x} {}",
                join((0..105).map(|y| grid.height(plane, x, y)))
            );
            assert_eq!(actual, expected, "height row plane {plane} x {x}");
        }
    }
    // Settings.
    for plane in 0..4 {
        for x in 0..104 {
            let expected = expected_lines.next().ok_or("oracle ended in settings")?;
            let actual = format!(
                "s {plane} {x} {}",
                join((0..104).map(|y| i32::from(grid.settings(plane, x, y) as i8)))
            );
            assert_eq!(actual, expected, "settings row plane {plane} x {x}");
        }
    }

    // Tiles, in plane/x/y order, each followed by its linked-below tile.
    let mut tile_count = 0_usize;
    for plane_index in 0..4_u8 {
        let plane = StoragePlane::new(plane_index).ok_or("plane")?;
        for x in 0..104_u32 {
            for y in 0..104_u32 {
                let Some(tile) = scene.tile(plane, SceneTile::new(x, y)) else {
                    continue;
                };
                tile_count += 1;
                let expected = expected_lines.next().ok_or("oracle ended in tiles")?;
                assert_eq!(
                    format_tile("tile", plane_index, x, y, tile),
                    expected,
                    "tile p{plane_index} ({x},{y})"
                );
                if let Some(below) = tile.linked_below() {
                    let expected = expected_lines.next().ok_or("oracle ended in below")?;
                    assert_eq!(
                        format_tile("below", plane_index, x, y, below),
                        expected,
                        "linked-below p{plane_index} ({x},{y})"
                    );
                }
            }
        }
    }
    assert_eq!(expected_lines.next(), None, "oracle has extra tiles");
    assert!(tile_count > 10_000, "fixture should exercise a real scene");
    Ok(())
}

fn parse_jitter(line: &str) -> Result<TerrainJitter, Box<dyn std::error::Error>> {
    // `rnd hue=<h> lightness=<l>`
    let hue = line
        .split("hue=")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .ok_or("jitter hue")?
        .parse::<i32>()?;
    let lightness = line
        .split("lightness=")
        .nth(1)
        .ok_or("jitter lightness")?
        .trim()
        .parse::<i32>()?;
    Ok(TerrainJitter { hue, lightness })
}
