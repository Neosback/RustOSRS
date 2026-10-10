//! Render one GPU frame of a real scene window to a PPM (convert with `sips -s format png`).
//!
//! usage: render_frame <cache-dir> <base-x> <base-y> <out.ppm> [cam-x cam-y cam-z yaw pitch scale]
//! Camera values are scene-local units / radians (defaults look at the Lumbridge castle).

use osrs_render::{
    SceneGeometry,
    gpu::{FrameParams, ReferenceCamera, SceneRenderer},
};
use osrs_world::{
    SceneWindow, TerrainPresentation, WorldDefinitions, build_world_scene, extract_scene_geometry,
    texture_layers,
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
        scale: number(10, 512.0),
    };

    let started = Instant::now();
    let world = build_world_scene(&mut definitions, window, TerrainPresentation::default())?;
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
    renderer.set_textures(&texture_layers(definitions.textures()));
    renderer.upload_scene(&geometry);
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
            clear_color: [0.55, 0.7, 0.9],
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
