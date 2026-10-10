//! Render one GPU frame of a real scene window to a PPM (convert with `sips -s format png`).
//!
//! usage: render_frame <cache-dir> <base-x> <base-y> <out.ppm> [cam-x cam-y cam-z yaw pitch scale]
//! Camera values are scene-local units / radians (defaults look at the Lumbridge castle).

use osrs_render::{
    SceneGeometry,
    gpu::{FrameParams, ReferenceCamera, SceneRenderer},
};
use osrs_world::{
    AnimationSystem, SceneWindow, TerrainPresentation, WorldDefinitions, build_world_scene,
    extract_animated_instances, extract_scene_geometry, texture_layers,
};
use std::{env, fs, io::Write, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let mut definitions = WorldDefinitions::open(&args[1])?;
    let window =
        SceneWindow::new(args[2].parse()?, args[3].parse()?).ok_or("window must be 8-aligned")?;
    let number = |index: usize, default: f32| -> f32 {
        args.get(index)
            .and_then(|v| v.parse().ok())
            .unwrap_or(default)
    };
    let camera = ReferenceCamera {
        x: number(5, 34.0 * 128.0),
        y: number(6, -2400.0),
        z: number(7, 18.0 * 128.0),
        yaw: number(8, 0.0),
        pitch: number(9, 0.75),
        // Horizontal FOV of 60 degrees at 1280 pixels wide: (w/2)/tan(30deg).
        scale: number(10, 640.0 / 30f32.to_radians().tan()),
    };

    let started = Instant::now();
    let world = build_world_scene(
        &mut definitions,
        window,
        TerrainPresentation {
            wall_merge_tolerance: env::var("RENDER_WALL_TOL").ok().and_then(|v| v.parse().ok()).unwrap_or(0),
            ..TerrainPresentation::default()
        },
    )?;
    let assembled = started.elapsed();
    let geometry: SceneGeometry = extract_scene_geometry(&world);
    let extracted = started.elapsed();
    println!(
        "assembled in {assembled:?}, extracted in {extracted:?}: {} zones, {} vertices ({} KiB)",
        geometry.zones.len(),
        geometry.vertex_count(),
        geometry.vertex_count() * 20 / 1024
    );

    let mut renderer = SceneRenderer::new_headless()?;
    // Zones are world-aligned; render relative to the window corner so camera values stay
    // scene-local.
    renderer.set_render_origin((window.base_x * 128, window.base_y * 128));
    renderer.set_textures(&texture_layers(definitions.textures()));
    renderer.upload_scene(&geometry);
    let mut animations = AnimationSystem::new();
    animations.insert_region((0, 0), extract_animated_instances(&world, None));
    let cycles: i32 = env::var("RENDER_ANIM_CYCLES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    animations.advance(cycles);
    let animated = animations.build_geometry((window.base_x + 52, window.base_y + 52), 200);
    println!(
        "animated: {} instances, {} vertices",
        animations.instance_count(),
        animated.vertex_count()
    );
    renderer.upload_region((i32::MIN, i32::MIN), &animated);
    println!(
        "resident vertex bytes: {}",
        renderer.resident_vertex_bytes()
    );

    let (width, height) = (1280_u32, 720_u32);
    let started = Instant::now();
    let pixels = renderer.render_to_rgba(
        FrameParams {
            camera,
            view_plane: 0,
            tick: 0,
            brightness: 0.8,
            remove_color_banding: env::var("RENDER_BANDING").is_err(),
            clear_color: if env::var("RENDER_SKY").is_ok() {
                [0.55, 0.7, 0.9]
            } else {
                [0.0, 0.0, 0.0]
            },
        },
        width,
        height,
    )?;
    println!("frame rendered+read back in {:?}", started.elapsed());

    let mut out = fs::File::create(&args[4])?;
    write!(out, "P6\n{width} {height}\n255\n")?;
    for pixel in pixels.as_chunks::<4>().0 {
        out.write_all(&pixel[..3])?;
    }
    Ok(())
}
