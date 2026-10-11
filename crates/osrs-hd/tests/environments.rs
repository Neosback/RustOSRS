//! Environment selection and blending against the vendored plugin data.

use osrs_hd::{
    HdData,
    environment::{EnvironmentSet, EnvironmentSettings, EnvironmentState, Lighting},
    expr::NoVariables,
    tiles::Areas,
};
use std::path::PathBuf;

fn assets() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/117hd")
}

fn load() -> Result<EnvironmentSet, Box<dyn std::error::Error>> {
    let data = HdData::load(assets())?;
    let areas = Areas::new(&data);
    Ok(EnvironmentSet::new(&data, &areas))
}

#[test]
fn lumbridge_and_falador_use_the_overworld_environment() -> Result<(), Box<dyn std::error::Error>> {
    let set = load()?;
    for (x, y) in [(3222, 3218), (2965, 3380)] {
        let environment = set.environment_at(x, y, 0);
        assert_eq!(environment.key.as_deref(), Some("OVERWORLD"), "{x},{y}");
        assert!(environment.is_overworld);
        assert_eq!(environment.values.directional_strength, 4.0);
        assert_eq!(environment.values.ambient_strength, 1.0);
        assert_eq!(environment.values.fog_depth, 6.5);
        // `#97baff` in linear sRGB.
        let ambient = environment.values.ambient_color;
        assert!((ambient[0] - 0.309).abs() < 0.01 && (ambient[2] - 1.0).abs() < 0.01);
        // The overworld defines no fog colour of its own: it follows the configured sky.
        assert!(!environment.has_fog_color_override);
    }
    Ok(())
}

#[test]
fn underground_and_sea_areas_have_their_own_environments() -> Result<(), Box<dyn std::error::Error>>
{
    let set = load()?;
    let caves = set.environment_at(3170, 9570, 0);
    assert_eq!(caves.area, "LUMBRIDGE_SWAMP_CAVES");
    assert!(!caves.is_overworld && caves.has_fog_color_override);
    assert_eq!(caves.values.ambient_strength, 0.9);
    assert_eq!(caves.values.fog_depth, 50.0);
    // Dark, bluish-green fog instead of the sky colour.
    assert!(caves.values.fog_color.iter().all(|c| *c < 0.01));

    // Far out at sea nothing matches: the default environment.
    let sea = set.environment_at(2000, 2000, 0);
    assert_eq!(sea.key.as_deref(), Some("DEFAULT"));
    Ok(())
}

#[test]
fn walking_from_the_surface_into_the_wilderness_blends() -> Result<(), Box<dyn std::error::Error>> {
    let set = load()?;
    let settings = EnvironmentSettings::default();
    let mut state = EnvironmentState::new();
    let mut now = 0.0;
    let step = |state: &mut EnvironmentState, x: i32, y: i32, now: f64| {
        state.update(&set, &settings, [x, y, 0], &NoVariables, now, 0.016);
    };
    step(&mut state, 3100, 3450, now);
    let before = *state.current();
    assert_eq!(state.target(&set).key.as_deref(), Some("OVERWORLD"));

    // Walk north one tile at a time (well under the teleport threshold) until the area changes.
    let (x, mut y) = (3100, 3450);
    while state.target(&set).key.as_deref() == Some("OVERWORLD") && y < 3800 {
        y += 1;
        now += 0.1;
        step(&mut state, x, y, now);
    }
    assert_ne!(state.target(&set).key.as_deref(), Some("OVERWORLD"));
    // The blend starts: one tick after the change it has hardly begun.
    assert!(state.transition_progress() < 0.05);
    assert!(state.transition_id() >= 2);
    now += 1.5;
    step(&mut state, x, y, now);
    let halfway = *state.current();
    let after = set.get(state.target_index().ok_or("target")?).values;
    assert!(state.transition_progress() > 0.3 && state.transition_progress() < 0.7);
    for axis in 0..3 {
        let (low, high) = (
            before.directional_color[axis].min(after.directional_color[axis]),
            before.directional_color[axis].max(after.directional_color[axis]),
        );
        assert!(halfway.directional_color[axis] >= low - 1e-6);
        assert!(halfway.directional_color[axis] <= high + 1e-6);
    }
    assert!(
        halfway.directional_strength < before.directional_strength.max(after.directional_strength)
    );
    now += 5.0;
    step(&mut state, x, y, now);
    assert_eq!(state.transition_progress(), 1.0);
    assert_eq!(
        state.current().directional_strength,
        after.directional_strength
    );

    let lighting: Lighting = state.lighting(&set, &settings, 50.0);
    assert!(lighting.casts_shadows);
    assert!((lighting.light_dir.iter().map(|c| c * c).sum::<f32>() - 1.0).abs() < 1e-4);
    Ok(())
}

#[test]
fn every_environment_normalises_to_finite_values() -> Result<(), Box<dyn std::error::Error>> {
    let set = load()?;
    assert!(set.len() > 200);
    for index in 0..set.len() {
        let environment = set.get(index);
        let lighting = Lighting::from_values(
            &environment.values,
            &EnvironmentSettings::default(),
            50.0,
            environment.is_underwater,
            environment.allow_roof_shadows,
            0.0,
        );
        for value in [
            lighting.ambient_strength,
            lighting.light_strength,
            lighting.fog_depth,
            lighting.light_dir[0],
            lighting.water_color_mid[1],
            lighting.ambient_color[2],
        ] {
            assert!(value.is_finite(), "{}: {lighting:?}", environment.label());
        }
    }
    Ok(())
}
