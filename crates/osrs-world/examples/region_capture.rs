//! Deterministic real-cache render regression capture, independent of editor layout/wall time.
//! Usage: region_capture CACHE OUTPUT.png reference|hd
use osrs_core::coords::RegionCoord;
use osrs_hd::settings::HdSettings;
use osrs_render::gpu::{
    Colorblind, Fog, FrameParams, HdLighting, ReferenceCamera, RenderProfile, SceneRenderer,
};
use osrs_world::hd::{HdContext, default_assets_dir};
use osrs_world::{
    TerrainPresentation, WorldDefinitions, build_region_geometry_with, texture_layers,
};
use std::{error::Error, fs::File, io::BufWriter, path::Path};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 {
        return Err("usage: region_capture CACHE OUTPUT.png reference|hd".into());
    }
    let profile = match args[3].as_str() {
        "reference" => RenderProfile::Reference,
        "hd" => RenderProfile::Hd,
        _ => return Err("profile must be reference or hd".into()),
    };
    let mut definitions = WorldDefinitions::open(&args[1])?;
    let mut renderer = SceneRenderer::new_headless()?;
    renderer.set_render_origin((49 * 64 * 128, 49 * 64 * 128));
    renderer.set_textures(&texture_layers(definitions.textures()));
    let assets = default_assets_dir();
    let hd = if profile == RenderProfile::Hd {
        let context = HdContext::load(
            &assets,
            HdSettings::default(),
            definitions
                .textures()
                .images
                .iter()
                .enumerate()
                .filter_map(|(index, image)| image.as_ref().map(|_| index as i32)),
        )?;
        context
            .prepare_gpu(&assets, definitions.textures(), 256)?
            .install(&mut renderer)?;
        renderer.set_hd_lighting(HdLighting {
            elapsed_seconds: 10.0,
            ..Default::default()
        });
        Some(context)
    } else {
        None
    };
    let presentation = TerrainPresentation {
        hd_geometry: hd.is_some(),
        ..Default::default()
    };
    for x in 49..=51 {
        for y in 49..=51 {
            if let Some(region) = build_region_geometry_with(
                &mut definitions,
                RegionCoord::new(x, y),
                presentation,
                hd.as_ref(),
            )? {
                renderer.upload_region((x, y), &region.geometry);
            }
        }
    }
    let (width, height) = (1280, 720);
    let pixels = renderer.render_to_rgba(
        FrameParams {
            camera: ReferenceCamera {
                x: (3228 - 49 * 64) as f32 * 128.0,
                y: -2000.0,
                z: (3219 - 49 * 64) as f32 * 128.0,
                yaw: 0.0,
                pitch: 0.75,
                scale: 1108.5125,
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
        },
        width,
        height,
    )?;
    let mut encoder = png::Encoder::new(
        BufWriter::new(File::create(Path::new(&args[2]))?),
        width,
        height,
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&pixels)?;
    eprintln!("owned GPU bytes: {:?}", renderer.resource_bytes());
    Ok(())
}
