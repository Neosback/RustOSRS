//! List all locs in a world tile rectangle. usage: decor_at <cache> <x0> <y0> <x1> <y1>
use osrs_core::coords::RegionCoord;
use osrs_world::{TerrainPresentation, WorldDefinitions, build_region_geometry};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    let mut defs = WorldDefinitions::open(&a[1])?;
    let (x0, y0, x1, y1): (i32, i32, i32, i32) =
        (a[2].parse()?, a[3].parse()?, a[4].parse()?, a[5].parse()?);
    let built = build_region_geometry(
        &mut defs,
        RegionCoord::new(x0.div_euclid(64), y0.div_euclid(64)),
        TerrainPresentation::default(),
    )?
    .ok_or("none")?;
    for loc in &built.info.locs {
        if loc.tile.0 >= x0
            && loc.tile.0 <= x1
            && loc.tile.1 >= y0
            && loc.tile.1 <= y1
            && loc.plane == 0
        {
            println!(
                "{} t{} r{} at {:?} slots={:?}",
                loc.definition.identity.id.get(),
                loc.loc_type,
                loc.orientation,
                loc.tile,
                loc.slots.iter().map(|s| (s.min, s.max)).collect::<Vec<_>>()
            );
        }
    }
    Ok(())
}
