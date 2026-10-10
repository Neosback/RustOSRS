//! Colour discontinuity across shared tile vertices of flat terrain paint.
use osrs_core::coords::{SceneTile, StoragePlane};
use osrs_scene::terrain::TerrainSurface;
use osrs_world::{SceneWindow, TerrainPresentation, WorldDefinitions, build_world_scene};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    let mut defs = WorldDefinitions::open(&a[1])?;
    let window = SceneWindow::new(a[2].parse()?, a[3].parse()?).ok_or("align")?;
    let world = build_world_scene(&mut defs, window, TerrainPresentation::default())?;
    let plane = StoragePlane::new(0).ok_or("plane")?;
    let flat = |x: u32, y: u32| match world
        .scene
        .tile(plane, SceneTile::new(x, y))
        .and_then(|t| t.terrain.as_ref())
    {
        Some(TerrainSurface::Flat(f))
            if f.colors.northeast != 12345678 && f.texture_id.is_none() =>
        {
            Some(*f)
        }
        _ => None,
    };
    let (mut pairs, mut hue_diff, mut sat_diff, mut light_diff) = (0u64, 0u64, 0u64, 0u64);
    let mut worst_light = 0;
    for x in 1..101 {
        for y in 1..102 {
            let (Some(l), Some(r)) = (flat(x, y), flat(x + 1, y)) else {
                continue;
            };
            // shared edge: l.se/l.ne vs r.sw/r.nw
            for (c1, c2) in [
                (l.colors.southeast, r.colors.southwest),
                (l.colors.northeast, r.colors.northwest),
            ] {
                pairs += 1;
                let (h1, s1, v1) = ((c1 >> 10) & 63, (c1 >> 7) & 7, c1 & 127);
                let (h2, s2, v2) = ((c2 >> 10) & 63, (c2 >> 7) & 7, c2 & 127);
                if h1 != h2 {
                    hue_diff += 1
                }
                if s1 != s2 {
                    sat_diff += 1
                }
                if v1 != v2 {
                    light_diff += 1;
                    worst_light = worst_light.max((v1 - v2).abs());
                }
            }
        }
    }
    println!(
        "shared-vertex pairs {pairs}: hue differs {hue_diff}, saturation differs {sat_diff}, lightness differs {light_diff} (worst {worst_light})"
    );
    Ok(())
}
