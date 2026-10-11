use osrs_hd::{
    HdData,
    materials::MaterialSet,
    settings::HdSettings,
};
use std::{collections::HashSet, path::PathBuf};

fn assets_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/117hd")
}

fn vanilla_texture_ids(data: &HdData) -> Vec<i32> {
    data.materials
        .iter()
        .filter_map(|material| material.vanilla_texture_index)
        .map(|index| index as i32)
        .filter(|index| *index >= 0)
        .collect()
}

#[test]
fn default_settings_replace_vanilla_brick_with_hd_brick() {
    let data = HdData::load(assets_dir()).expect("load checked-in 117HD data");
    let set = MaterialSet::build(
        &data,
        &HdSettings::default(),
        vanilla_texture_ids(&data),
    );

    let index = set.from_vanilla_texture(2);
    let material = set
        .get(index)
        .expect("vanilla texture 2 should resolve to a material");

    assert_eq!(material.name, "HD_BRICK");
    assert_eq!(index, set.index_of("BRICK"));
    assert_eq!(index, set.index_of("HD_BRICK"));
    assert!(!set.is_fallback_vanilla(index));
    assert_eq!(material.normal_map.as_deref(), Some("HD_BRICK_N"));
    assert_eq!(material.displacement_map.as_deref(), Some("HD_BRICK_D"));
}

#[test]
fn disabling_model_textures_restores_vanilla_brick_resolution() {
    let data = HdData::load(assets_dir()).expect("load checked-in 117HD data");
    let mut settings = HdSettings::default();
    settings.model_textures = false;
    let set = MaterialSet::build(&data, &settings, vanilla_texture_ids(&data));

    let index = set.from_vanilla_texture(2);
    let material = set
        .get(index)
        .expect("vanilla texture 2 should resolve to BRICK");

    assert_eq!(material.name, "BRICK");
    assert_eq!(index, set.index_of("BRICK"));
    assert_eq!(set.index_of("HD_BRICK"), 0);
    assert!(!set.is_fallback_vanilla(index));
}

#[test]
fn checked_in_ground_materials_reference_known_materials() {
    let data = HdData::load(assets_dir()).expect("load checked-in 117HD data");
    let material_names: HashSet<&str> = data
        .materials
        .iter()
        .map(|material| material.name.as_str())
        .collect();

    let mut missing = Vec::new();
    for ground in &data.ground_materials {
        for material in &ground.materials {
            if material != "NONE" && !material_names.contains(material.as_str()) {
                missing.push(format!("{} -> {}", ground.name, material));
            }
        }
    }

    assert!(
        missing.is_empty(),
        "ground-material references missing from materials.json: {}",
        missing.join(", ")
    );
}
