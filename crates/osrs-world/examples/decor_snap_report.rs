//! Report: snapped vs client offsets of diagonal decorations in a scene window.
//! usage: decor_snap_report <cache> <base-x> <base-y>
use osrs_scene::placement::PlacementKind;
use osrs_world::{SceneWindow, TerrainPresentation, WorldDefinitions, build_world_scene};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    let mut defs = WorldDefinitions::open(&a[1])?;
    let window = SceneWindow::new(a[2].parse()?, a[3].parse()?).ok_or("align")?;
    let world = build_world_scene(
        &mut defs,
        window,
        TerrainPresentation {
            flush_diagonal_decorations: true,
            ..TerrainPresentation::default()
        },
    )?;
    let (mut same, mut changed) = (0, 0);
    for loc in &world.locs {
        let PlacementKind::WallDecoration(decor) = loc.plan.kind else {
            continue;
        };
        let Some(offsets) = loc.slot_offsets else {
            continue;
        };
        let client = [(decor.offset_x, decor.offset_z), (0, 0)];
        if offsets == client {
            same += 1;
        } else {
            changed += 1;
            if changed <= 12 {
                println!(
                    "obj {} t{} r{} at ({}, {}): client {:?} -> snapped {:?}",
                    loc.object_id.get(),
                    loc.loc_type,
                    loc.orientation,
                    window.base_x + loc.tile.x as i32,
                    window.base_y + loc.tile.y as i32,
                    client,
                    offsets
                );
            }
        }
    }
    println!("snapped decorations: {same} unchanged, {changed} moved");
    Ok(())
}
