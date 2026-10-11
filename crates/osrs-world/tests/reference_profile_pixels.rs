//! Pixel-diff guard for the Reference render profile.
//!
//! Renders the Lumbridge window from a fixed camera and compares every pixel with the golden image
//! captured before the render-profile switch existed. The Reference profile must stay pixel-identical
//! to it: `RUSTOSRS_TARGET_CACHE_DIR=$HOME/RustOSRS-cache/b241/cache cargo test --release -p osrs-world
//! --test reference_profile_pixels -- --ignored`. Set `UPDATE_GOLDEN=1` to rewrite the golden image
//! (only when the Reference output is meant to change).

use osrs_render::gpu::{
    Colorblind, Fog, FrameParams, HdParams, ReferenceCamera, RenderProfile, SceneRenderer,
};
use osrs_world::{
    SceneWindow, TerrainPresentation, WorldDefinitions, build_world_scene, extract_scene_geometry,
    texture_layers,
};
use std::{fs, fs::File, path::PathBuf};

const WIDTH: u32 = 640;
const HEIGHT: u32 = 360;

fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/reference_lumbridge_640x360.png")
}

fn render(profile: RenderProfile) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let dir = std::env::var("RUSTOSRS_TARGET_CACHE_DIR")?;
    let mut definitions = WorldDefinitions::open(dir)?;
    let window = SceneWindow::new(3176, 3176).ok_or("window alignment")?;
    let world = build_world_scene(&mut definitions, window, TerrainPresentation::default())?;
    let geometry = extract_scene_geometry(&world);

    let mut renderer = SceneRenderer::new_headless()?;
    renderer.set_render_origin((window.base_x * 128, window.base_y * 128));
    renderer.set_textures(&texture_layers(definitions.textures()));
    renderer.upload_scene(&geometry);

    let params = FrameParams {
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
    };
    renderer.set_hd_params(HdParams {
        exposure: 1.0,
        placeholder_vignette: 0.0,
    });
    Ok(renderer.render_to_rgba(params, WIDTH, HEIGHT)?)
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

#[test]
#[ignore = "requires the pinned build-241 cache (RUSTOSRS_TARGET_CACHE_DIR) and a GPU"]
fn reference_profile_matches_golden() -> Result<(), Box<dyn std::error::Error>> {
    let pixels = render(RenderProfile::Reference)?;
    let path = golden_path();
    if std::env::var_os("UPDATE_GOLDEN").is_some() || !path.exists() {
        fs::create_dir_all(path.parent().ok_or("golden directory")?)?;
        write_png(&path, &pixels)?;
        println!("wrote {}", path.display());
        return Ok(());
    }
    let golden = read_png(&path)?;
    assert_eq!(golden.len(), pixels.len());
    let differing = golden
        .as_chunks::<4>()
        .0
        .iter()
        .zip(pixels.as_chunks::<4>().0.iter())
        .filter(|(a, b)| a != b)
        .count();
    let max_delta = golden
        .iter()
        .zip(&pixels)
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap_or(0);
    assert_eq!(
        differing, 0,
        "{differing} pixels differ from the golden image (max channel delta {max_delta})"
    );
    Ok(())
}

/// The HD profile's HDR round trip with neutral resolve settings keeps the scene within rounding
/// of the Reference output (the HD scene shading itself is not ported yet).
#[test]
#[ignore = "requires the pinned build-241 cache (RUSTOSRS_TARGET_CACHE_DIR) and a GPU"]
fn hd_profile_placeholder_tracks_the_reference_scene() -> Result<(), Box<dyn std::error::Error>> {
    let reference = render(RenderProfile::Reference)?;
    let hd = render(RenderProfile::Hd)?;
    assert_eq!(reference.len(), hd.len());
    let large = reference
        .iter()
        .zip(&hd)
        .filter(|(a, b)| a.abs_diff(**b) > 3)
        .count();
    println!(
        "{large} of {} channel values differ by more than 3",
        hd.len()
    );
    // MSAA resolve in fp16 versus unorm8 may move a few edge samples; the bulk must match.
    assert!(
        large * 1000 < hd.len(),
        "{large} channel values differ by more than 3"
    );
    Ok(())
}
