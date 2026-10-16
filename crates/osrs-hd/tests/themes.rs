#![allow(clippy::unwrap_used)] // Test fixtures intentionally fail at the offending value.
use osrs_core::coords::{MapTile, PlaneIndex};
use osrs_hd::theme::*;

fn number(value: f32) -> ThemeNumber {
    ThemeNumber::new(value).unwrap()
}
fn query(x: i32) -> AppearanceQuery {
    AppearanceQuery {
        tile: MapTile::new(x, 3200),
        plane: PlaneIndex::new(0).unwrap(),
        underlay: None,
        overlay: None,
        object: None,
        kind: None,
        orientation: None,
    }
}
#[test]
fn ordered_rules_override_only_authored_properties_and_explain_winners() {
    let mut theme = ThemeDefinition::stock();
    theme.overrides.ambient_strength = Some(number(1.0));
    theme.rules.push(ThemeRule {
        id: "cave".into(),
        name: "Cave".into(),
        enabled: true,
        target: ThemeTarget::Area {
            min: MapTile::new(3000, 3000),
            max: MapTile::new(3100, 3300),
            min_plane: PlaneIndex::new(0).unwrap(),
            max_plane: PlaneIndex::new(1).unwrap(),
        },
        values: ThemeOverrides {
            ambient_strength: Some(number(0.2)),
            ..Default::default()
        },
    });
    theme.rules.push(ThemeRule {
        id: "fog".into(),
        name: "Mist".into(),
        enabled: true,
        target: ThemeTarget::Global,
        values: ThemeOverrides {
            fog_color: Some([100, 120, 140]),
            ..Default::default()
        },
    });
    let project = ThemeOverrides {
        exposure: Some(number(1.3)),
        ..Default::default()
    };
    let inside = theme.resolve(&project, &query(3050));
    assert_eq!(inside.values.ambient_strength, Some(number(0.2)));
    assert_eq!(inside.values.exposure, project.exposure);
    assert_eq!(inside.winning_rules["ambient_strength"], "Cave");
    assert_eq!(inside.winning_rules["fog_color"], "Mist");
    assert_eq!(
        theme
            .resolve(&project, &query(3200))
            .values
            .ambient_strength,
        Some(number(1.0))
    );
}
#[test]
fn pack_roundtrip_is_deterministic_self_contained_and_detects_corruption() {
    let mut pack = ThemePack::default();
    pack.assets
        .insert("textures/grass.png".into(), ThemeAsset::new(vec![1, 2, 3]));
    let bytes = pack.to_zip().unwrap();
    assert_eq!(pack.to_zip().unwrap(), bytes);
    let restored = ThemePack::from_zip(&bytes).unwrap();
    assert_eq!(restored, pack);
    assert_eq!(restored.identity().unwrap(), pack.identity().unwrap());
    std::sync::Arc::make_mut(&mut pack.assets.get_mut("textures/grass.png").unwrap().bytes)[0] ^= 1;
    assert!(pack.to_zip().is_err());
}
#[test]
fn pack_rejects_missing_assets_and_nonportable_paths() {
    let mut pack = ThemePack::default();
    for path in ["../outside", "/absolute", "a\\b", "C:/file", "a//b"] {
        pack.assets.insert(path.into(), ThemeAsset::new(vec![]));
        assert!(pack.validate().is_err());
        pack.assets.clear();
    }
    pack.definition.materials.push(ThemeMaterial {
        name: "grass".into(),
        parent: "GRASS".into(),
        diffuse: Some("textures/missing.png".into()),
        normal: None,
        height: None,
        roughness: None,
        ambient_occlusion: None,
        flow: None,
    });
    assert!(
        pack.validate()
            .unwrap_err()
            .contains("Missing material asset")
    );
}
#[test]
fn stale_patch_is_atomic_and_inverse_restores_exact_pack() {
    let before = ThemePack::default();
    let mut after = before.clone();
    after.definition.name = "My theme".into();
    let patch = ThemePatch::plan(&before, after.clone()).unwrap();
    let mut doc = before.clone();
    patch.apply(&mut doc).unwrap();
    assert!(patch.apply(&mut doc).is_err());
    assert_eq!(doc, after);
    patch.inverse().apply(&mut doc).unwrap();
    assert_eq!(doc, before);
    assert!(
        ThemePatch::plan(&before, before.clone())
            .unwrap()
            .is_empty()
    );
}
#[test]
fn authored_numbers_and_area_bounds_are_validated() {
    assert!(ThemeNumber::new(f32::NAN).is_err());
    assert!(ThemeNumber::new(f32::INFINITY).is_err());
    assert_eq!(number(-0.0), number(0.0));
    let mut theme = ThemeDefinition::stock();
    theme.overrides.exposure = Some(number(-1.0));
    assert!(theme.validate().is_err());
}
