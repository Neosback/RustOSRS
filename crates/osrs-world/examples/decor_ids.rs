//! List wall-decoration object ids of a region with counts and one location.
use osrs_core::coords::RegionCoord;
use osrs_scene::placement::PlacementKind;
use osrs_world::{TerrainPresentation, WorldDefinitions, build_region_geometry};
use std::collections::BTreeMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    let mut defs = WorldDefinitions::open(&a[1])?;
    let built = build_region_geometry(
        &mut defs,
        RegionCoord::new(a[2].parse()?, a[3].parse()?),
        TerrainPresentation::default(),
    )?
    .ok_or("none")?;
    #[allow(clippy::type_complexity)]
    let mut map: BTreeMap<(u32, u8), (usize, (i32, i32), u8)> = BTreeMap::new();
    for loc in &built.info.locs {
        if matches!(loc.plan.kind, PlacementKind::WallDecoration(_)) {
            let e = map
                .entry((loc.definition.identity.id.get(), loc.loc_type))
                .or_insert((0, loc.tile, loc.plane));
            e.0 += 1;
        }
    }
    for ((id, t), (n, tile, plane)) in map {
        println!("obj {id} type {t}: {n} e.g. {tile:?} plane {plane}");
    }
    Ok(())
}
