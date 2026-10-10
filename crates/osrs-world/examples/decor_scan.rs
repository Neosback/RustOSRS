//! Debug: wall decorations (types 4-8) of a region with their host walls.
//! usage: decor_scan <cache> <region-x> <region-y>
use osrs_core::coords::RegionCoord;
use osrs_scene::placement::PlacementKind;
use osrs_world::{TerrainPresentation, WorldDefinitions, build_region_geometry};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    let mut defs = WorldDefinitions::open(&a[1])?;
    let region = RegionCoord::new(a[2].parse()?, a[3].parse()?);
    let built = build_region_geometry(&mut defs, region, TerrainPresentation::default())?
        .ok_or("no region")?;
    let info = &built.info;
    for loc in &info.locs {
        let PlacementKind::WallDecoration(decor) = loc.plan.kind else {
            continue;
        };
        if loc.loc_type < 6 {
            continue;
        }
        let hosts: Vec<String> = info
            .locs_at(loc.tile)
            .filter(|h| {
                h.plane == loc.plane && matches!(h.plan.kind, PlacementKind::Boundary(_))
                    || (h.plane == loc.plane && h.loc_type == 9)
            })
            .map(|h| {
                format!(
                    "{}(t{} r{} disp{})",
                    h.definition.identity.id.get(),
                    h.loc_type,
                    h.orientation,
                    h.definition.decoration_displacement
                )
            })
            .collect();
        println!(
            "decor {} t{} r{} at {:?} plane {} offset=({}, {}) flag={} hosts=[{}]",
            loc.definition.identity.id.get(),
            loc.loc_type,
            loc.orientation,
            loc.tile,
            loc.plane,
            decor.offset_x,
            decor.offset_z,
            decor.orientation_flag,
            hosts.join(", ")
        );
    }
    Ok(())
}
