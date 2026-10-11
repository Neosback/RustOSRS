//! Audit: relation of every wall decoration plate to the straight wall slab on its tile.
//! usage: decor_audit <cache> <region-x> <region-y>
use osrs_core::coords::RegionCoord;
use osrs_scene::placement::PlacementKind;
use osrs_world::{TerrainPresentation, WorldDefinitions, build_region_geometry};
use std::collections::BTreeMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    let mut defs = WorldDefinitions::open(&a[1])?;
    let region = RegionCoord::new(a[2].parse()?, a[3].parse()?);
    let built = build_region_geometry(&mut defs, region, TerrainPresentation::default())?
        .ok_or("no region")?;
    let info = &built.info;
    let mut stats: BTreeMap<(u8, String), usize> = BTreeMap::new();
    let mut examples: BTreeMap<(u8, String), String> = BTreeMap::new();
    for loc in &info.locs {
        let PlacementKind::WallDecoration(_) = loc.plan.kind else {
            continue;
        };
        // host: a straight boundary (type 0/2) on the same tile and plane
        let host = info.locs_at(loc.tile).find(|h| {
            h.plane == loc.plane
                && matches!(h.plan.kind, PlacementKind::Boundary(_))
                && matches!(h.loc_type, 0 | 2)
        });
        let Some(host) = host else {
            *stats
                .entry((loc.loc_type, "no straight host".into()))
                .or_default() += 1;
            continue;
        };
        for (si, slot) in loc.slots.iter().enumerate() {
            for hs in &host.slots {
                let ext = |s: &osrs_world::SlotInfo, axis: usize| (s.min[axis], s.max[axis]);
                // thin axis of the wall slab (the smaller extent of its x / z bounds)
                let wx = hs.max[0] - hs.min[0];
                let wz = hs.max[2] - hs.min[2];
                let axis = if wx <= wz { 0 } else { 2 };
                let (wl, wh) = ext(hs, axis);
                let (pl, ph) = ext(slot, axis);
                if wh - wl > 40 {
                    continue;
                } // full-size slab (corner pieces), skip
                let gap = (pl - wh).max(wl - ph).max(0);
                let overlap = (ph.min(wh) - pl.max(wl)).max(0);
                let thick = (ph - pl).max(1);
                let class = if gap > 2 {
                    format!("floating {gap}")
                } else if overlap * 2 > thick {
                    "embedded".to_string()
                } else {
                    "flush".to_string()
                };
                let class_key = if class.starts_with("floating") {
                    "floating".to_string()
                } else {
                    class.clone()
                };
                if class_key != "flush" || loc.definition.identity.id.get() == 1938 { println!("  {class} : obj {} t{} r{} at {:?} host {} t{} r{} {}  plate[{pl},{ph}] wall[{wl},{wh}]", loc.definition.identity.id.get(), loc.loc_type, loc.orientation, loc.tile, host.definition.identity.id.get(), host.loc_type, host.orientation, host.definition.decoration_displacement); }
                let key = (loc.loc_type, format!("{class_key} (slot {si})"));
                *stats.entry(key.clone()).or_default() += 1;
                examples.entry(key).or_insert_with(|| format!("obj {} r{} at {:?} host {} r{} {} plate[{pl},{ph}] wall[{wl},{wh}] axis {axis}", loc.definition.identity.id.get(), loc.orientation, loc.tile, host.definition.identity.id.get(), host.orientation, class));
            }
        }
    }
    for (key, count) in &stats {
        println!(
            "type {:>2} {:<24} {count:>5}   e.g. {}",
            key.0,
            key.1,
            examples.get(key).map_or("", String::as_str)
        );
    }
    Ok(())
}
