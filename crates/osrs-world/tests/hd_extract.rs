//! HD extraction against the real Lumbridge scene (needs the pinned build-241 cache):
//! `RUSTOSRS_TARGET_CACHE_DIR=$HOME/RustOSRS-cache/b241/cache cargo test --release -p osrs-world
//! --test hd_extract -- --ignored`.

use osrs_hd::settings::HdSettings;
use osrs_render::{HD_FLAG_TERRAIN, HD_MATERIAL_INDEX_MASK, SceneGeometry, oct_decode};
use osrs_world::{
    SceneWindow, TerrainPresentation, WorldDefinitions, WorldScene, build_world_scene,
    hd::{HdContext, default_assets_dir, extract::extract_owned_geometry_hd},
};
use std::collections::BTreeMap;

fn lumbridge() -> Result<(WorldDefinitions, WorldScene), Box<dyn std::error::Error>> {
    let dir = std::env::var("RUSTOSRS_TARGET_CACHE_DIR")?;
    let mut definitions = WorldDefinitions::open(dir)?;
    let window = SceneWindow::new(3176, 3176).ok_or("window alignment")?;
    let world = build_world_scene(&mut definitions, window, TerrainPresentation::default())?;
    Ok((definitions, world))
}

fn context(definitions: &WorldDefinitions) -> Result<HdContext, Box<dyn std::error::Error>> {
    let textures = definitions
        .textures()
        .images
        .iter()
        .enumerate()
        .filter(|(_, image)| image.is_some())
        .map(|(id, _)| id as i32);
    Ok(HdContext::load(
        &default_assets_dir(),
        HdSettings::default(),
        textures,
    )?)
}

/// Face counts by material name, split into terrain and model faces.
fn material_counts(
    geometry: &SceneGeometry,
    hd: &HdContext,
) -> (BTreeMap<String, usize>, BTreeMap<String, usize>) {
    let (mut terrain, mut models) = (BTreeMap::new(), BTreeMap::new());
    for zone in &geometry.zones {
        for group in &zone.groups {
            assert_eq!(group.geometry.opaque.len(), group.geometry.hd_opaque.len());
            assert_eq!(group.geometry.alpha.len(), group.geometry.hd_alpha.len());
            for face in group
                .geometry
                .hd_opaque
                .chunks(3)
                .chain(group.geometry.hd_alpha.chunks(3))
            {
                let vertex = &face[0];
                let index = usize::from(vertex.material_water[0] & HD_MATERIAL_INDEX_MASK);
                let name = hd.materials.get(index).map_or("NONE", |m| m.name.as_str());
                let table = if vertex.hsl_flags[3] & HD_FLAG_TERRAIN != 0 {
                    &mut terrain
                } else {
                    &mut models
                };
                *table.entry(name.to_owned()).or_default() += 1;
            }
        }
    }
    (terrain, models)
}

#[test]
#[ignore = "requires the pinned build-241 cache (RUSTOSRS_TARGET_CACHE_DIR)"]
fn lumbridge_terrain_brick_and_wood_use_the_hd_materials() -> Result<(), Box<dyn std::error::Error>>
{
    let (definitions, world) = lumbridge()?;
    let hd = context(&definitions)?;
    let geometry = extract_owned_geometry_hd(&world, None, &hd);
    let (terrain, models) = material_counts(&geometry, &hd);
    let count = |table: &BTreeMap<String, usize>, names: &[&str]| -> usize {
        names.iter().map(|name| table.get(*name).copied().unwrap_or(0)).sum()
    };
    let terrain_total: usize = terrain.values().sum();
    assert!(terrain_total > 10_000, "{terrain_total} terrain faces");

    // Terrain: grass and dirt/path ground, no un-materialled terrain to speak of.
    let grass = count(&terrain, &["GRASS_1", "GRASS_2", "GRASS_3"]);
    let dirt = count(&terrain, &["DIRT_1", "DIRT_2"]);
    assert!(grass * 2 > terrain_total, "grass {grass} of {terrain_total}: {terrain:?}");
    assert!(dirt > 100, "dirt {dirt}: {terrain:?}");
    assert!(
        terrain.get("NONE").copied().unwrap_or(0) * 10 < terrain_total,
        "untextured terrain: {terrain:?}"
    );

    // Brick and wood: the vanilla textures 2 and 3 are replaced by their HD materials, and the
    // castle's faces carry them.
    let brick = hd.materials.get(hd.materials.from_vanilla_texture(2)).ok_or("brick")?;
    assert_eq!(brick.name, "HD_BRICK");
    let wood = hd.materials.get(hd.materials.from_vanilla_texture(3)).ok_or("wood")?;
    assert_eq!(wood.name, "HD_WOOD_PLANKS_1");
    let brick_faces = count(&models, &[brick.name.as_str()]);
    // Wooden objects take their wood from model overrides (`WOOD_GRAIN_*`, `*_WOOD`) as well as
    // from the vanilla plank texture.
    let wood_faces: usize = models
        .iter()
        .filter(|(name, _)| name.contains("WOOD"))
        .map(|(_, faces)| faces)
        .sum();
    assert!(brick_faces > 2_000, "{brick_faces} {} faces", brick.name);
    assert!(wood_faces > 10_000, "{wood_faces} wood faces: {models:?}");
    Ok(())
}

#[test]
#[ignore = "requires the pinned build-241 cache (RUSTOSRS_TARGET_CACHE_DIR)"]
fn hd_normals_point_the_way_the_triangles_face() -> Result<(), Box<dyn std::error::Error>> {
    let (definitions, world) = lumbridge()?;
    let hd = context(&definitions)?;
    let geometry = extract_owned_geometry_hd(&world, None, &hd);
    // Horizontal terrain triangles wound counter-clockwise seen from above (the front-facing
    // winding) must have an upward (negative y) normal.
    let (mut up, mut down) = (0, 0);
    for zone in &geometry.zones {
        for group in &zone.groups {
            for (tri, hd_tri) in group
                .geometry
                .opaque
                .chunks(3)
                .zip(group.geometry.hd_opaque.chunks(3))
            {
                if hd_tri[0].hsl_flags[3] & HD_FLAG_TERRAIN == 0 {
                    continue;
                }
                let p = |i: usize| tri[i].position.map(i64::from);
                let (a, b, c) = (p(0), p(1), p(2));
                if a[1] != b[1] || b[1] != c[1] {
                    continue;
                }
                let orientation = (b[0] - a[0]) * (c[2] - a[2]) - (b[2] - a[2]) * (c[0] - a[0]);
                let normal = oct_decode(hd_tri[0].normal);
                if orientation > 0 {
                    if normal[1] < -0.5 {
                        up += 1;
                    } else if normal[1] > 0.5 {
                        down += 1;
                    }
                }
            }
        }
    }
    assert!(up > 1_000, "{up} up-facing");
    assert!(down * 20 < up, "{down} of {} front faces point down", up + down);
    Ok(())
}
