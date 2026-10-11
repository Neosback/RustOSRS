//! Phase 5 of the 117 HD port on a real scene: water surfaces, shoreline foam, underwater
//! terrain. Run: `RUSTOSRS_TARGET_CACHE_DIR=$HOME/RustOSRS-cache/b241/cache cargo test --release -p
//! osrs-world --test hd_water_pixels -- --ignored`.

use osrs_hd::settings::HdSettings;
use osrs_render::{
    HD_FLAG_TERRAIN,
    gpu::{
        Colorblind, Fog, FrameParams, HdLighting, HdParams, ReferenceCamera, RenderProfile,
        SceneRenderer,
    },
};
use osrs_world::{
    SceneWindow, TerrainPresentation, WorldDefinitions, build_world_scene,
    hd::{HdContext, default_assets_dir, extract::extract_owned_geometry_hd, gpu::HdGpuData},
    texture_layers,
};

const WIDTH: u32 = 320;
const HEIGHT: u32 = 180;

/// A top-down view of the River Lum east of Lumbridge castle.
fn river_camera() -> ReferenceCamera {
    ReferenceCamera {
        x: (3235 - 3176) as f32 * 128.0,
        y: -9000.0,
        z: (3235 - 3176) as f32 * 128.0,
        yaw: 0.0,
        pitch: 1.5,
        scale: 160.0 / 30f32.to_radians().tan(),
    }
}

fn params() -> FrameParams {
    FrameParams {
        camera: river_camera(),
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

/// Share of pixels that are clearly blue (blue above red and green).
fn blue_share(pixels: &[u8]) -> f64 {
    let blue = pixels
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| p[2] > p[0].saturating_add(25) && p[2] >= p[1])
        .count();
    blue as f64 / (pixels.len() / 4) as f64
}

#[test]
#[ignore = "requires the pinned build-241 cache (RUSTOSRS_TARGET_CACHE_DIR) and a GPU"]
fn the_river_has_a_water_surface_and_underwater_terrain() -> Result<(), Box<dyn std::error::Error>>
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
    let geometry = extract_owned_geometry_hd(&world, None, &hd);

    // Surface faces: terrain in the alpha stream with a water type and no depth; the shoreline
    // lightness (foam mask) is `0` or `127`. Underwater ground: opaque, with depth.
    let water = hd.water.index_of("WATER");
    assert_ne!(water, 0);
    let (mut surface, mut shore, mut underwater) = (0, 0, 0);
    for zone in &geometry.zones {
        for group in &zone.groups {
            for vertex in group
                .geometry
                .hd_alpha
                .as_chunks::<3>()
                .0
                .iter()
                .map(|face| &face[0])
            {
                if vertex.hsl_flags[3] & HD_FLAG_TERRAIN != 0
                    && vertex.material_water[3] & 0xFF != 0
                    && vertex.depth[..3] == [0, 0, 0]
                {
                    surface += 1;
                    assert!(
                        vertex.hsl_flags[..3]
                            .iter()
                            .all(|c| *c & 127 == 0 || *c & 127 == 127)
                    );
                    shore += usize::from(vertex.hsl_flags[..3].iter().any(|c| *c & 127 == 0));
                }
            }
            for vertex in group
                .geometry
                .hd_opaque
                .as_chunks::<3>()
                .0
                .iter()
                .map(|face| &face[0])
            {
                if vertex.hsl_flags[3] & HD_FLAG_TERRAIN != 0
                    && vertex.material_water[3] & 0xFF != 0
                    && vertex.depth[..3].iter().any(|d| *d > 0)
                {
                    underwater += 1;
                }
            }
        }
    }
    assert!(surface > 100, "{surface} surface faces");
    assert!(shore > 10, "{shore} shoreline faces");
    assert!(underwater > 100, "{underwater} underwater faces");

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
    renderer.upload_scene(&geometry);
    renderer.set_hd_lighting(HdLighting::default());
    let frame = renderer.render_to_rgba(params(), WIDTH, HEIGHT)?;
    // The river crosses the frame: a visible share of it is blue; grass and paths are not.
    let share = blue_share(&frame);
    assert!(share > 0.04 && share < 0.5, "blue share {share:.3}");

    // The water animates: another clock gives another surface (closer view, over the river).
    let close = FrameParams {
        camera: ReferenceCamera {
            x: (3243 - 3176) as f32 * 128.0,
            y: -1500.0,
            z: (3235 - 3176) as f32 * 128.0,
            yaw: 0.0,
            pitch: 1.1,
            scale: 160.0 / 30f32.to_radians().tan(),
        },
        ..params()
    };
    let before = renderer.render_to_rgba(close, WIDTH, HEIGHT)?;
    renderer.set_hd_lighting(HdLighting {
        elapsed_seconds: 7.3,
        ..HdLighting::default()
    });
    let later = renderer.render_to_rgba(close, WIDTH, HEIGHT)?;
    let differing = before
        .as_chunks::<4>()
        .0
        .iter()
        .zip(later.as_chunks::<4>().0)
        .filter(|(a, b)| a[..3].iter().zip(&b[..3]).any(|(x, y)| x.abs_diff(*y) > 3))
        .count();
    assert!(
        differing > 300,
        "{differing} pixels changed with the water clock"
    );
    Ok(())
}
