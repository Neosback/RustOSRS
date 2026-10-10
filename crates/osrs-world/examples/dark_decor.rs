//! Find wall decorations whose model has many very dark faces (arrow slits / crosses).
use osrs_core::{coords::RegionCoord, definitions::LocType};
use osrs_scene::placement::PlacementKind;
use osrs_world::{TerrainPresentation, WorldDefinitions, build_region_geometry};
use std::collections::BTreeSet;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    let mut defs = WorldDefinitions::open(&a[1])?;
    let mut seen = BTreeSet::new();
    for pair in a[2..].chunks(2) {
        let built = build_region_geometry(
            &mut defs,
            RegionCoord::new(pair[0].parse()?, pair[1].parse()?),
            TerrainPresentation::default(),
        )?
        .ok_or("none")?;
        for loc in &built.info.locs {
            if matches!(loc.plan.kind, PlacementKind::WallDecoration(_))
                && seen.insert(loc.definition.identity.id.get())
            {
                let def = loc.definition.clone();
                if let Some(m) = defs.resolve_model(&def, LocType::new(4), 0)? {
                    let colors = m.face_colors();
                    let dark = colors.iter().filter(|c| (**c & 127) < 12).count();
                    println!(
                        "obj {} faces={} dark={} (first at {:?})",
                        def.identity.id.get(),
                        colors.len(),
                        dark,
                        loc.tile
                    );
                }
            }
        }
    }
    Ok(())
}
