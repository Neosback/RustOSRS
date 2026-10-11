//! Material, ground-material and tile-override resolution against the vendored data.

use osrs_hd::{
    HdData,
    materials::MaterialSet,
    settings::{HdSettings, SeasonalTheme},
    tiles::{Areas, GroundMaterials, OVERLAY_FLAG, TileOverrides, TileVariables, WaterTypes},
};
use std::path::PathBuf;

fn assets() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/117hd")
}

#[test]
fn vanilla_textures_map_to_hd_materials() -> Result<(), Box<dyn std::error::Error>> {
    let data = HdData::load(assets())?;
    let set = MaterialSet::build(&data, &HdSettings::default(), 0..200);
    // Texture 2 is the vanilla brick; with object textures on it is replaced by HD_BRICK.
    let brick = set.from_vanilla_texture(2);
    assert_eq!(brick, set.index_of("HD_BRICK"));
    assert_eq!(set.index_of("BRICK"), set.index_of("HD_BRICK"));
    let hd_brick = set.get(brick).ok_or("HD_BRICK")?;
    assert_eq!(hd_brick.normal_map.as_deref(), Some("HD_BRICK_N"));
    assert_eq!(hd_brick.specular_strength, Some(0.2));
    // Wood planks keep their own parameters.
    let wood = set.get(set.from_vanilla_texture(3)).ok_or("wood")?;
    assert!(wood.specular_strength.is_some_and(|s| s > 0.0), "{wood:?}");

    let vanilla_only = MaterialSet::build(
        &data,
        &HdSettings {
            model_textures: false,
            ..HdSettings::default()
        },
        0..200,
    );
    let plain = vanilla_only
        .get(vanilla_only.from_vanilla_texture(2))
        .ok_or("BRICK")?;
    assert_eq!(plain.name, "BRICK");
    // Every vanilla texture resolves to something.
    assert!((0..200).all(|id| set.from_vanilla_texture(id) != 0));
    Ok(())
}

#[test]
fn ground_materials_and_tile_overrides_resolve() -> Result<(), Box<dyn std::error::Error>> {
    let data = HdData::load(assets())?;
    let settings = HdSettings::default();
    let materials = MaterialSet::build(&data, &settings, 0..200);
    let grounds = GroundMaterials::new(&data, &materials);
    let water = WaterTypes::new(&data);
    let areas = Areas::new(&data);
    let overrides = TileOverrides::new(&data, &grounds, &water, &|id| {
        osrs_hd::Gamevals::resolve(&data.gamevals.objects, id)
    });

    assert!(
        grounds
            .get(GroundMaterials::DIRT)
            .materials
            .iter()
            .all(|m| *m != 0)
    );
    assert_eq!(water.seasonal(WaterTypes::NONE, 1), water.index_of("WATER"));
    assert_eq!(water.seasonal(WaterTypes::NONE, -1), WaterTypes::NONE);

    // Some overlay in the overworld must resolve to a non-default ground material.
    let mut found = None;
    for overlay in 1..200 {
        let hit = overrides.lookup(&areas, (3222, 3218, 0), &[OVERLAY_FLAG | overlay]);
        if hit.index != TileOverrides::NONE {
            found = Some((overlay, hit));
            break;
        }
    }
    let (_, hit) = found.ok_or("no overlay override in Lumbridge")?;
    assert!(hit.queried_as_overlay);
    let vars = TileVariables {
        settings: &settings,
        hsl: [8, 3, 40],
    };
    let resolved = overrides.resolve(hit.index, &vars);
    assert_ne!(
        overrides.get(resolved).ground_material,
        GroundMaterials::NONE
    );

    // The seasonal wrappers pick the summer variant unless the theme is winter.
    let seasonal = data
        .tile_overrides
        .iter()
        .position(|o| o.name == "SEASONAL_GRASS")
        .ok_or("SEASONAL_GRASS")?
        + 1;
    let summer = overrides.resolve(seasonal, &vars);
    assert_eq!(overrides.get(summer).name, "DEFAULT_GRASS");
    let winter_settings = HdSettings {
        season: SeasonalTheme::Winter,
        ..settings
    };
    let winter = overrides.resolve(
        seasonal,
        &TileVariables {
            settings: &winter_settings,
            hsl: [8, 3, 40],
        },
    );
    assert_eq!(overrides.get(winter).name, "WINTER_GRASS");
    Ok(())
}
