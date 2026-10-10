//! Check the outline triangles of an object in a region. usage: outline_probe <cache> <rx> <ry> <object-id>
use osrs_core::coords::RegionCoord;
use osrs_world::{
    TerrainPresentation, WorldDefinitions, build_region_geometry, loc_outline_triangles,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    let mut defs = WorldDefinitions::open(&a[1])?;
    let built = build_region_geometry(
        &mut defs,
        RegionCoord::new(a[2].parse()?, a[3].parse()?),
        TerrainPresentation::default(),
    )?
    .ok_or("none")?;
    let id: u32 = a[4].parse()?;
    for (i, loc) in built.info.locs.iter().enumerate() {
        if loc.definition.identity.id.get() == id {
            let tris = loc_outline_triangles(&mut defs, loc);
            println!(
                "loc {i} at {:?}: slots {} -> triangles {:?}",
                loc.tile,
                loc.slots.len(),
                tris.as_ref().map(Vec::len)
            );
            if let Err(e) = tris {
                println!("  error: {e}");
            }
        }
    }
    Ok(())
}
