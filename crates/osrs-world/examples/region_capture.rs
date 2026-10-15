//! Deterministic real-cache render capture for same-backend before/after comparison.
//!
//! Usage: `region_capture CACHE OUT_DIR reference|hd [--compare PREVIOUS_DIR [--tolerance N]]`
//!
//! Renders fixed views of the Lumbridge regions headlessly, with the editor's per-zone upload
//! keys and the animated-loc transient slot at a fixed game cycle, and writes
//! `OUT_DIR/<profile>-<view>.png`. One JSON line per view (FNV-1a hash of the RGBA bytes) goes to
//! stdout. With `--compare`, each view is diffed against the same file in `PREVIOUS_DIR` and the
//! process exits non-zero when a channel differs by more than `--tolerance` (default 0: exact).
//! Reference output is exact run to run; the HD profile was observed to vary by one channel level
//! in a few pixels between identical runs, so use a small tolerance there and read the reported
//! counts. Compare only captures made on the same adapter and driver.
use osrs_core::coords::RegionCoord;
use osrs_hd::settings::HdSettings;
use osrs_render::gpu::{
    Colorblind, Fog, FrameParams, HdLighting, ReferenceCamera, RenderProfile, SceneRenderer,
};
use osrs_world::hd::{HdContext, default_assets_dir};
use osrs_world::{
    AnimationSystem, TerrainPresentation, WorldDefinitions, build_region_geometry_with,
    texture_layers,
};
use std::{
    error::Error,
    fs::File,
    io::{BufReader, BufWriter},
    path::{Path, PathBuf},
};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;
const ZONES_PER_REGION_AXIS: i32 = 8;
const ANIMATED_KEY: (i32, i32) = (i32::MIN, i32::MIN);
/// Game cycle the animated locs are advanced to before capture.
const ANIMATION_CYCLES: i32 = 137;

struct View {
    name: &'static str,
    tile: (i32, i32),
    /// Client height units above the plane (positive up).
    height: f32,
    yaw: f32,
    pitch: f32,
    draw_distance_tiles: u32,
}

const VIEWS: &[View] = &[
    View {
        name: "aerial",
        tile: (3228, 3219),
        height: 2000.0,
        yaw: 0.0,
        pitch: 0.75,
        draw_distance_tiles: 50,
    },
    View {
        name: "street",
        tile: (3222, 3226),
        height: 400.0,
        yaw: 0.35,
        pitch: 0.2,
        draw_distance_tiles: 50,
    },
    View {
        name: "distance",
        tile: (3228, 3219),
        height: 3500.0,
        yaw: 0.0,
        pitch: 0.5,
        draw_distance_tiles: 184,
    },
];

/// Must match `zone_renderer_key` in `osrs-editor/src/app/render_runtime.rs`: draw order of
/// coplanar Reference faces follows this key order.
fn zone_renderer_key(region: (i32, i32), zone: (i32, i32)) -> (i32, i32) {
    let local_x = zone.0 - region.0 * ZONES_PER_REGION_AXIS;
    let local_y = zone.1 - region.1 * ZONES_PER_REGION_AXIS;
    (
        region.0,
        region.1 * ZONES_PER_REGION_AXIS * ZONES_PER_REGION_AXIS
            + local_x * ZONES_PER_REGION_AXIS
            + local_y,
    )
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

fn read_png(path: &Path) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut reader = png::Decoder::new(BufReader::new(File::open(path)?)).read_info()?;
    let size = reader
        .output_buffer_size()
        .ok_or("PNG output size overflows")?;
    let mut buffer = vec![0; size];
    let info = reader.next_frame(&mut buffer)?;
    buffer.truncate(info.buffer_size());
    Ok(buffer)
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let compare = args
        .iter()
        .position(|arg| arg == "--compare")
        .map(|index| {
            let dir = args.get(index + 1).cloned();
            args.drain(index..(index + 2).min(args.len()));
            dir.map(PathBuf::from).ok_or("--compare needs a directory")
        })
        .transpose()?;
    let tolerance = match args.iter().position(|arg| arg == "--tolerance") {
        Some(index) => {
            let value = args
                .get(index + 1)
                .ok_or("--tolerance needs a value")?
                .parse::<u8>()?;
            args.drain(index..index + 2);
            value
        }
        None => 0,
    };
    if args.len() != 3 {
        return Err(
            "usage: region_capture CACHE OUT_DIR reference|hd [--compare DIR [--tolerance N]]"
                .into(),
        );
    }
    let profile_name = args[2].as_str();
    let profile = match profile_name {
        "reference" => RenderProfile::Reference,
        "hd" => RenderProfile::Hd,
        _ => return Err("profile must be reference or hd".into()),
    };
    let out_dir = PathBuf::from(&args[1]);
    std::fs::create_dir_all(&out_dir)?;
    let mut definitions = WorldDefinitions::open(&args[0])?;
    let mut renderer = SceneRenderer::new_headless()?;
    let origin = (49 * 64 * 128, 49 * 64 * 128);
    renderer.set_render_origin(origin);
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
    let mut animations = AnimationSystem::new();
    for x in 49..=51 {
        for y in 49..=51 {
            if let Some(region) = build_region_geometry_with(
                &mut definitions,
                RegionCoord::new(x, y),
                presentation,
                hd.as_ref(),
            )? {
                for zone in &region.geometry.zones {
                    renderer.upload_zones(
                        zone_renderer_key((x, y), (zone.zone_x, zone.zone_z)),
                        std::iter::once(zone),
                    );
                }
                animations.insert_region((x, y), region.animations);
            }
        }
    }
    animations.advance(ANIMATION_CYCLES);
    renderer.upload_region(ANIMATED_KEY, &animations.build_geometry((3228, 3219), 80));
    let mut differing = false;
    for view in VIEWS {
        let pixels = renderer.render_to_rgba(
            FrameParams {
                camera: ReferenceCamera {
                    x: (view.tile.0 * 128 + 64 - origin.0) as f32,
                    y: -view.height,
                    z: (view.tile.1 * 128 + 64 - origin.1) as f32,
                    yaw: view.yaw,
                    pitch: view.pitch,
                    scale: 1108.5125,
                },
                view_plane: 0,
                tick: 0,
                brightness: 0.8,
                remove_color_banding: true,
                clear_color: [0.55, 0.7, 0.9],
                bright_textures: false,
                fog: Fog {
                    draw_distance_tiles: view.draw_distance_tiles,
                    ..Fog::default()
                },
                colorblind: Colorblind::default(),
                profile,
            },
            WIDTH,
            HEIGHT,
        )?;
        let file = format!("{profile_name}-{}.png", view.name);
        let mut encoder = png::Encoder::new(
            BufWriter::new(File::create(out_dir.join(&file))?),
            WIDTH,
            HEIGHT,
        );
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(&pixels)?;
        let mut line = format!(
            "{{\"view\":\"{}\",\"profile\":\"{profile_name}\",\"fnv1a\":\"{:016x}\"",
            view.name,
            fnv1a(&pixels)
        );
        if let Some(previous) = &compare {
            let before = read_png(&previous.join(&file))?;
            let (count, largest) = if before.len() == pixels.len() {
                before
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .zip(pixels.as_chunks::<4>().0.iter())
                    .filter(|(a, b)| a != b)
                    .fold((0u64, 0u8), |(count, largest), (a, b)| {
                        let delta = a
                            .iter()
                            .zip(b)
                            .map(|(x, y)| x.abs_diff(*y))
                            .max()
                            .unwrap_or(0);
                        (count + 1, largest.max(delta))
                    })
            } else {
                (u64::MAX, u8::MAX)
            };
            differing |= count != 0 && largest > tolerance;
            line += &format!(",\"differing_pixels\":{count},\"max_channel_delta\":{largest}");
        }
        println!("{line}}}");
    }
    eprintln!("owned GPU bytes: {:?}", renderer.resource_bytes());
    if differing {
        return Err("captures differ from the comparison directory".into());
    }
    Ok(())
}
