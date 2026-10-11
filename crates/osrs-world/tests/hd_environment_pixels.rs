//! Phase 3 of the 117 HD port on a real scene: the area environment drives the HD frame's
//! lighting, fog and sky colour. Run: `RUSTOSRS_TARGET_CACHE_DIR=$HOME/RustOSRS-cache/b241/cache
//! cargo test --release -p osrs-world --test hd_environment_pixels -- --ignored`.

use osrs_hd::{
    environment::{EnvironmentSettings, Lighting},
    settings::HdSettings,
};
use osrs_render::gpu::{
    Colorblind, Fog, FrameParams, HdLighting, HdParams, ReferenceCamera, RenderProfile,
    SceneRenderer,
};
use osrs_world::{
    SceneWindow, TerrainPresentation, WorldDefinitions, build_world_scene,
    hd::{HdContext, default_assets_dir, extract::extract_owned_geometry_hd, gpu::HdGpuData},
    texture_layers,
};

const WIDTH: u32 = 320;
const HEIGHT: u32 = 180;

fn params() -> FrameParams {
    FrameParams {
        camera: ReferenceCamera {
            x: 34.0 * 128.0,
            y: -2400.0,
            z: 18.0 * 128.0,
            yaw: 0.0,
            pitch: 0.45,
            scale: 160.0 / 30f32.to_radians().tan(),
        },
        view_plane: 0,
        tick: 0,
        brightness: 0.8,
        remove_color_banding: true,
        clear_color: [0.0; 3],
        bright_textures: false,
        fog: Fog::default(),
        colorblind: Colorblind::default(),
        profile: RenderProfile::Hd,
    }
}

fn mean_luminance(pixels: &[u8]) -> f64 {
    let sum: f64 = pixels
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| 0.2126 * f64::from(p[0]) + 0.7152 * f64::from(p[1]) + 0.0722 * f64::from(p[2]))
        .sum();
    sum / (pixels.len() / 4) as f64
}

/// Mean of `red - (green + blue) / 2`: how red the image is.
fn redness(pixels: &[u8]) -> f64 {
    let sum: f64 = pixels
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| f64::from(p[0]) - (f64::from(p[1]) + f64::from(p[2])) / 2.0)
        .sum();
    sum / (pixels.len() / 4) as f64
}

fn scene() -> Result<(SceneRenderer, HdContext), Box<dyn std::error::Error>> {
    let dir = std::env::var("RUSTOSRS_TARGET_CACHE_DIR")?;
    let mut definitions = WorldDefinitions::open(dir)?;
    let window = SceneWindow::new(3176, 3176).ok_or("window alignment")?;
    let world = build_world_scene(&mut definitions, window, TerrainPresentation::default())?;
    let present = definitions
        .textures()
        .images
        .iter()
        .enumerate()
        .filter(|(_, image)| image.is_some())
        .map(|(id, _)| id as i32);
    let hd = HdContext::load(&default_assets_dir(), HdSettings::default(), present)?;

    let mut renderer = SceneRenderer::new_headless()?;
    renderer.set_render_origin((window.base_x * 128, window.base_y * 128));
    renderer.set_textures(&texture_layers(definitions.textures()));
    renderer.set_hd_params(HdParams {
        exposure: 1.0,
        placeholder_vignette: 0.0,
    });
    hd.prepare_gpu(
        &default_assets_dir(),
        definitions.textures(),
        HdGpuData::RESOLUTION,
    )?
    .install(&mut renderer)
    .map_err(|error| error.to_string())?;
    renderer.upload_scene(&extract_owned_geometry_hd(&world, None, &hd));

    Ok((renderer, hd))
}

#[test]
#[ignore = "requires the pinned build-241 cache (RUSTOSRS_TARGET_CACHE_DIR) and a GPU"]
fn the_area_environment_changes_lighting_fog_and_sky() -> Result<(), Box<dyn std::error::Error>> {
    let (mut renderer, hd) = scene()?;
    let settings = EnvironmentSettings::default();
    let mut render =
        |values: &osrs_hd::environment::EnvValues| -> Result<Vec<u8>, Box<dyn std::error::Error>> {
            renderer.set_hd_lighting(HdLighting {
                lighting: Lighting::from_values(values, &settings, 50.0, false, true, 0.0),
                ..HdLighting::default()
            });
            Ok(renderer.render_to_rgba(params(), WIDTH, HEIGHT)?)
        };

    // Lumbridge is overworld: a bright, sunny frame. The swamp caves are dim and fogged.
    let surface = hd.environments.environment_at(3222, 3218, 0).values;
    let caves = hd.environments.environment_at(3170, 9570, 0).values;
    let surface_frame = render(&surface)?;
    let caves_frame = render(&caves)?;
    let (surface_luminance, caves_luminance) =
        (mean_luminance(&surface_frame), mean_luminance(&caves_frame));
    assert!(
        caves_luminance < surface_luminance * 0.85,
        "caves {caves_luminance:.1} vs surface {surface_luminance:.1}"
    );

    // The sky colour fills the background: a red fog colour reddens the frame, and deep fog
    // swallows the distance.
    let mut red = surface;
    red.fog_color = [1.0, 0.0, 0.0];
    red.fog_depth = 90.0;
    let red_frame = render(&red)?;
    assert!(
        redness(&red_frame) > redness(&surface_frame) + 15.0,
        "{} vs {}",
        redness(&red_frame),
        redness(&surface_frame)
    );

    // No fog depth: the sky colour only shows where nothing is drawn (the camera looks down, so
    // the frame stays unchanged by the fog colour apart from the sky-light tint).
    let mut no_fog = surface;
    no_fog.fog_depth = 0.0;
    no_fog.fog_color = [1.0, 0.0, 0.0];
    let plain = render(&no_fog)?;
    assert!(redness(&plain) < redness(&red_frame));
    Ok(())
}

#[test]
#[ignore = "requires the pinned build-241 cache (RUSTOSRS_TARGET_CACHE_DIR) and a GPU"]
fn fog_and_colour_blindness_settings_reach_the_hd_frame() -> Result<(), Box<dyn std::error::Error>>
{
    let (mut renderer, hd) = scene()?;
    let settings = EnvironmentSettings::default();
    let surface = hd.environments.environment_at(3222, 3218, 0).values;
    // Distance fog is a setting: without it the far scenery keeps its own colours.
    let fogged = {
        let mut values = surface;
        values.fog_color = [1.0, 0.0, 0.0];
        values.fog_depth = 12.0;
        values
    };
    renderer.set_hd_lighting(HdLighting {
        lighting: Lighting::from_values(&fogged, &settings, 50.0, false, true, 0.0),
        ..HdLighting::default()
    });
    let with_fog = renderer.render_to_rgba(params(), WIDTH, HEIGHT)?;
    let no_fog_settings = EnvironmentSettings {
        distance_fog: false,
        ..settings
    };
    renderer.set_hd_lighting(HdLighting {
        lighting: Lighting::from_values(&fogged, &no_fog_settings, 50.0, false, true, 0.0),
        ..HdLighting::default()
    });
    let without_fog = renderer.render_to_rgba(params(), WIDTH, HEIGHT)?;
    assert!(redness(&with_fog) > redness(&without_fog) + 3.0);

    // The colour blindness correction reaches the HD frame like the Reference one.
    renderer.set_hd_lighting(HdLighting::default());
    let plain = renderer.render_to_rgba(params(), WIDTH, HEIGHT)?;
    let corrected = renderer.render_to_rgba(
        FrameParams {
            colorblind: Colorblind {
                mode: osrs_render::gpu::ColorblindMode::Protanope,
                intensity: 100.0,
            },
            ..params()
        },
        WIDTH,
        HEIGHT,
    )?;
    let changed = plain
        .iter()
        .zip(&corrected)
        .filter(|(a, b)| a.abs_diff(**b) > 2)
        .count();
    assert!(
        changed > plain.len() / 20,
        "{changed} channel values changed"
    );

    Ok(())
}
