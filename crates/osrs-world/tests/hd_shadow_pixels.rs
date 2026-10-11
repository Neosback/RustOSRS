//! Phase 4 of the 117 HD port on a real scene: the directional shadow map. Run:
//! `RUSTOSRS_TARGET_CACHE_DIR=$HOME/RustOSRS-cache/b241/cache cargo test --release -p osrs-world
//! --test hd_shadow_pixels -- --ignored`.

use osrs_hd::{
    settings::HdSettings,
    shadows::{ShadowFiltering, ShadowMode, ShadowSettings},
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
            pitch: 0.75,
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

fn luminance(pixel: &[u8]) -> f64 {
    0.2126 * f64::from(pixel[0]) + 0.7152 * f64::from(pixel[1]) + 0.0722 * f64::from(pixel[2])
}

fn mean_luminance(pixels: &[u8]) -> f64 {
    pixels.as_chunks::<4>().0.iter().map(|p| luminance(p)).sum::<f64>()
        / (pixels.len() / 4) as f64
}

/// Share of pixels that are darker in `shadowed` than in `lit` by more than `margin`.
fn darker_share(lit: &[u8], shadowed: &[u8], margin: f64) -> f64 {
    let count = lit
        .as_chunks::<4>()
        .0
        .iter()
        .zip(shadowed.as_chunks::<4>().0)
        .filter(|(a, b)| luminance(*a) - luminance(*b) > margin)
        .count();
    count as f64 / (lit.len() / 4) as f64
}

#[test]
#[ignore = "requires the pinned build-241 cache (RUSTOSRS_TARGET_CACHE_DIR) and a GPU"]
fn the_shadow_map_darkens_the_lee_of_buildings_and_trees() -> Result<(), Box<dyn std::error::Error>>
{
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
    renderer.set_hd_lighting(HdLighting::default());

    let mut render = |settings: ShadowSettings| -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        renderer.set_hd_shadows(settings);
        Ok(renderer.render_to_rgba(params(), WIDTH, HEIGHT)?)
    };

    let off = render(ShadowSettings {
        mode: ShadowMode::Off,
        ..ShadowSettings::default()
    })?;
    let detailed = render(ShadowSettings::default())?;
    // Shadows only remove light: the frame gets darker, and a visible share of it (the lee of the
    // castle walls, the trees) clearly so.
    let (lit_mean, shadow_mean) = (mean_luminance(&off), mean_luminance(&detailed));
    assert!(shadow_mean < lit_mean * 0.97, "{shadow_mean:.1} vs {lit_mean:.1}");
    let dark = darker_share(&off, &detailed, 20.0);
    assert!(dark > 0.03 && dark < 0.6, "{dark:.3} of the frame is shadowed");
    // Nothing gets brighter (up to filtering noise).
    assert!(darker_share(&detailed, &off, 20.0) < 0.002);

    // Every filter and the cheaper modes produce a plausible, similar shadow.
    for filtering in ShadowFiltering::ALL {
        let frame = render(ShadowSettings {
            filtering,
            ..ShadowSettings::default()
        })?;
        let mean = mean_luminance(&frame);
        assert!(
            (mean - shadow_mean).abs() < shadow_mean * 0.04,
            "{filtering:?}: {mean:.1} vs {shadow_mean:.1}"
        );
    }
    for settings in [
        ShadowSettings {
            mode: ShadowMode::Fast,
            ..ShadowSettings::default()
        },
        ShadowSettings {
            transparency: false,
            terrain_shadows: false,
            ..ShadowSettings::default()
        },
    ] {
        let frame = render(settings)?;
        assert!(mean_luminance(&frame) < lit_mean * 0.98);
    }
    Ok(())
}
