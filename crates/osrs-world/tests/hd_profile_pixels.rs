//! Pixel guard for the HD render profile (phase 2: materials, normal maps, ground blending).
//!
//! Renders the Lumbridge window from the camera of `reference_profile_pixels` through the HD
//! pipeline and compares it with a golden image within a tolerance (GPUs differ in the last bits of
//! filtered texture reads). `UPDATE_GOLDEN=1` rewrites the golden image when the look is meant to
//! change. Run: `RUSTOSRS_TARGET_CACHE_DIR=$HOME/RustOSRS-cache/b241/cache cargo test --release -p
//! osrs-world --test hd_profile_pixels -- --ignored`.

use osrs_hd::settings::HdSettings;
use osrs_render::gpu::{
    Colorblind, Fog, FrameParams, HdParams, ReferenceCamera, RenderProfile, SceneRenderer,
};
use osrs_world::{
    SceneWindow, TerrainPresentation, WorldDefinitions, build_world_scene, extract_scene_geometry,
    hd::{HdContext, default_assets_dir, extract::extract_owned_geometry_hd, gpu::HdGpuData},
    texture_layers,
};
use std::{fs, fs::File, path::PathBuf};

const WIDTH: u32 = 640;
const HEIGHT: u32 = 360;

fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/hd_lumbridge_640x360.png")
}

fn params(profile: RenderProfile) -> FrameParams {
    FrameParams {
        camera: ReferenceCamera {
            x: 34.0 * 128.0,
            y: -2400.0,
            z: 18.0 * 128.0,
            yaw: 0.0,
            pitch: 0.75,
            scale: 320.0 / 30f32.to_radians().tan(),
        },
        view_plane: 0,
        tick: 0,
        brightness: 0.8,
        remove_color_banding: true,
        clear_color: [0.55, 0.7, 0.9],
        bright_textures: false,
        fog: Fog::default(),
        colorblind: Colorblind::default(),
        profile,
    }
}

/// `(reference, hd)` frames of the Lumbridge window.
fn render_both() -> Result<(Vec<u8>, Vec<u8>), Box<dyn std::error::Error>> {
    let dir = std::env::var("RUSTOSRS_TARGET_CACHE_DIR")?;
    let mut definitions = WorldDefinitions::open(dir)?;
    let window = SceneWindow::new(3176, 3176).ok_or("window alignment")?;
    let world = build_world_scene(&mut definitions, window, TerrainPresentation::default())?;
    let origin = (window.base_x * 128, window.base_y * 128);

    let mut renderer = SceneRenderer::new_headless()?;
    renderer.set_render_origin(origin);
    renderer.set_textures(&texture_layers(definitions.textures()));
    renderer.set_hd_params(HdParams {
        exposure: 1.0,
        placeholder_vignette: 0.0,
    });
    renderer.upload_scene(&extract_scene_geometry(&world));
    let reference = renderer.render_to_rgba(params(RenderProfile::Reference), WIDTH, HEIGHT)?;

    let present = definitions
        .textures()
        .images
        .iter()
        .enumerate()
        .filter(|(_, image)| image.is_some())
        .map(|(id, _)| id as i32);
    let hd = HdContext::load(&default_assets_dir(), HdSettings::default(), present)?;
    hd.prepare_gpu(
        &default_assets_dir(),
        definitions.textures(),
        HdGpuData::RESOLUTION,
    )?
    .install(&mut renderer)
    .map_err(|error| error.to_string())?;
    renderer.upload_scene(&extract_owned_geometry_hd(&world, None, &hd));
    let hd_frame = renderer.render_to_rgba(params(RenderProfile::Hd), WIDTH, HEIGHT)?;
    Ok((reference, hd_frame))
}

fn write_png(path: &std::path::Path, pixels: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let mut encoder = png::Encoder::new(File::create(path)?, WIDTH, HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(pixels)?;
    Ok(())
}

fn read_png(path: &std::path::Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut reader = png::Decoder::new(std::io::BufReader::new(File::open(path)?)).read_info()?;
    let mut buffer = vec![0; reader.output_buffer_size().ok_or("png too large")?];
    let info = reader.next_frame(&mut buffer)?;
    assert_eq!((info.width, info.height), (WIDTH, HEIGHT));
    buffer.truncate(info.buffer_size());
    Ok(buffer)
}

/// Mean absolute difference over the colour channels, and the share of pixels differing by more
/// than `threshold` in any channel.
fn compare(a: &[u8], b: &[u8], threshold: u8) -> (f64, f64) {
    let mut sum = 0_u64;
    let mut differing = 0_usize;
    for (pa, pb) in a.as_chunks::<4>().0.iter().zip(b.as_chunks::<4>().0) {
        let deltas = [0, 1, 2].map(|i| pa[i].abs_diff(pb[i]));
        sum += deltas.iter().map(|d| u64::from(*d)).sum::<u64>();
        differing += usize::from(deltas.iter().any(|d| *d > threshold));
    }
    let pixels = (a.len() / 4) as f64;
    (sum as f64 / (pixels * 3.0), differing as f64 / pixels)
}

#[test]
#[ignore = "requires the pinned build-241 cache (RUSTOSRS_TARGET_CACHE_DIR) and a GPU"]
fn hd_lumbridge_matches_golden_and_differs_from_reference() -> Result<(), Box<dyn std::error::Error>>
{
    let (reference, hd) = render_both()?;

    // The HD frame is a different look, not a recolour of the Reference one.
    let (mean, _) = compare(&reference, &hd, 0);
    assert!(mean > 8.0, "HD differs from Reference by only {mean:.2} per channel");

    let path = golden_path();
    if std::env::var_os("UPDATE_GOLDEN").is_some() || !path.exists() {
        fs::create_dir_all(path.parent().ok_or("golden directory")?)?;
        write_png(&path, &hd)?;
        println!("wrote {}", path.display());
        return Ok(());
    }
    let golden = read_png(&path)?;
    assert_eq!(golden.len(), hd.len());
    let (mean, share) = compare(&golden, &hd, 12);
    assert!(mean < 1.5, "mean channel difference {mean:.3} from the golden image");
    assert!(share < 0.005, "{:.3}% of pixels differ by more than 12", share * 100.0);
    Ok(())
}
