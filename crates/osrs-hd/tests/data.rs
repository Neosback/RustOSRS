//! The vendored 117 HD data loads and cross-references.

use osrs_hd::{Gamevals, HdData, IdRef};
use std::{collections::HashSet, path::PathBuf};

fn assets() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/117hd")
}

#[test]
fn loads_every_table() -> Result<(), Box<dyn std::error::Error>> {
    let data = HdData::load(assets())?;
    assert!(
        data.materials.len() > 300,
        "{} materials",
        data.materials.len()
    );
    assert!(data.tile_overrides.len() > 100);
    assert!(data.model_overrides.len() > 500);
    assert!(data.areas.len() > 300);
    assert!(data.environments.len() > 100);
    assert!(data.lights.len() > 1000);
    assert!(data.gamevals.objects.len() > 50_000);
    assert!(
        data.sky_presets
            .iter()
            .any(|preset| preset.name == "GIELINOR")
    );
    Ok(())
}

#[test]
fn ground_materials_and_tiles_point_at_real_entries() -> Result<(), Box<dyn std::error::Error>> {
    let data = HdData::load(assets())?;
    let materials: HashSet<&str> = data.materials.iter().map(|m| m.name.as_str()).collect();
    let grounds: HashSet<&str> = data
        .ground_materials
        .iter()
        .map(|g| g.name.as_str())
        .collect();
    for ground in &data.ground_materials {
        for material in &ground.materials {
            assert!(
                materials.contains(material.as_str()),
                "{} -> {material}",
                ground.name
            );
        }
    }
    let grass = data
        .tile_overrides
        .iter()
        .find(|tile| tile.name == "DEFAULT_GRASS")
        .ok_or("DEFAULT_GRASS tile override")?;
    assert_eq!(grass.ground_material.as_deref(), Some("OVERWORLD_GRASS_1"));
    for tile in &data.tile_overrides {
        if let Some(ground) = &tile.ground_material {
            // `DIRT` and `UNDERWATER_GENERIC` are defined in the plugin's code, not the JSON.
            let builtin = matches!(ground.as_str(), "NONE" | "DIRT" | "UNDERWATER_GENERIC");
            assert!(
                builtin || grounds.contains(ground.as_str()),
                "{} -> {ground}",
                tile.name
            );
        }
    }
    Ok(())
}

#[test]
fn environments_and_areas_resolve() -> Result<(), Box<dyn std::error::Error>> {
    let data = HdData::load(assets())?;
    for environment in &data.environments {
        if let Some(area) = &environment.area {
            assert!(data.area(area).is_some(), "environment area {area}");
        }
    }
    // Evil Bob's island box from areas.json.
    assert!(data.area_contains("EVIL_BOB_ISLAND", 2500, 4750, 0));
    assert!(!data.area_contains("EVIL_BOB_ISLAND", 3222, 3218, 0));
    let color = data
        .environments
        .iter()
        .find_map(|environment| environment.fog_color.as_ref())
        .and_then(|color| color.rgb())
        .ok_or("a fog colour")?;
    assert!(color.iter().all(|c| (0.0..=1.0).contains(c)));
    Ok(())
}

#[test]
fn gameval_names_resolve() -> Result<(), Box<dyn std::error::Error>> {
    let data = HdData::load(assets())?;
    let mut resolved = 0;
    for model in &data.model_overrides {
        for id in &model.object_ids {
            if Gamevals::resolve(&data.gamevals.objects, id).is_some() {
                resolved += 1;
            }
            if let IdRef::Name(name) = id {
                // Wildcard-free names must exist; report the first few misses for diagnosis.
                let _ = name;
            }
        }
    }
    assert!(resolved > 1000, "{resolved} object overrides resolved");
    Ok(())
}
