//! Print a generated building as text: walls, doors and windows on a tile grid, then the roof.
//!
//! usage: building_ascii [kind] [seed] [width] [height] [variation]
//! kind: house|shop|tavern|bank|church|warehouse|castle|dungeon
use osrs_core::coords::{MapTile, SourcePlane};
use osrs_edit::{
    BuildingArchetype, BuildingConstraints, BuildingRequest, BuildingStyle, HeightVertexCoord,
    KitFile, MapDocument, MapOp, RectSite, capture_generation_snapshot,
};

const KIT: &str = include_str!("../src/app/prefab_runtime/kits/lumbridge-brick.kit.json");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg =
        |index: usize, default: &str| args.get(index).map_or(default, String::as_str).to_owned();
    let archetype = match arg(0, "house").as_str() {
        "shop" => BuildingArchetype::Shop,
        "tavern" => BuildingArchetype::Tavern,
        "bank" => BuildingArchetype::Bank,
        "church" => BuildingArchetype::Church,
        "warehouse" => BuildingArchetype::Warehouse,
        "castle" => BuildingArchetype::Castle,
        "dungeon" => BuildingArchetype::Dungeon,
        _ => BuildingArchetype::House,
    };
    let (seed, width, height, variation): (u64, u16, u16, u32) = (
        arg(1, "1").parse()?,
        arg(2, "12").parse()?,
        arg(3, "10").parse()?,
        arg(4, "0").parse()?,
    );
    let kit: KitFile = serde_json::from_str(KIT)?;
    let door = kit.door.id;
    let window = kit
        .finish
        .as_ref()
        .and_then(|f| f.window_decor.as_ref())
        .and_then(|w| w.inside.as_ref())
        .map(|d| d.id);
    let wall_decor: Vec<u32> = kit
        .finish
        .iter()
        .flat_map(|f| f.wall_decor.iter().map(|d| d.id))
        .collect();
    let compiled = kit.compile()?;
    let plane = SourcePlane::new(0).ok_or("plane")?;
    let origin = MapTile::new(3000, 3000);
    let request = BuildingRequest {
        seed,
        site: RectSite::new(origin, width, height).ok_or("site")?,
        base_plane: plane,
        floors: 1,
        archetype,
        style: BuildingStyle::Generic,
        constraints: BuildingConstraints::default(),
    };
    let mut heights = Vec::new();
    for y in -1..=i32::from(height) + 1 {
        for x in -1..=i32::from(width) + 1 {
            heights.push((
                HeightVertexCoord::new(MapTile::new(origin.x + x, origin.y + y), plane),
                -64,
            ));
        }
    }
    let document = MapDocument::from_parts([], heights, []);
    let snapshot = capture_generation_snapshot(&document, request, []);
    let patch = compiled
        .generate(&snapshot, variation)
        .map_err(|e| format!("{e:?}"))?;

    let (w, h) = (usize::from(width), usize::from(height));
    // Tile (x, y) is drawn at column 2x+1, row 2(h-1-y)+1 (north up); edges sit between.
    let mut canvas = vec![vec![' '; 2 * w + 1]; 2 * h + 1];
    let mut floor = vec![vec![false; w]; h];
    let mut roof = vec![vec!['.'; w]; h];
    let mut furniture: Vec<(usize, usize, char)> = Vec::new();
    for op in patch.ops() {
        match *op {
            MapOp::SetTerrain { coord, .. } => {
                let (x, y) = (
                    (coord.tile.x - origin.x) as usize,
                    (coord.tile.y - origin.y) as usize,
                );
                if x < w && y < h {
                    floor[y][x] = true;
                }
            }
            MapOp::AddObject { placement } => {
                let (x, y) = (
                    placement.position.tile.x - origin.x,
                    placement.position.tile.y - origin.y,
                );
                if x < 0 || y < 0 || x as usize >= w || y as usize >= h {
                    continue;
                }
                let (x, y) = (x as usize, y as usize);
                let kind = placement.kind.get();
                if kind == 4 || kind == 5 {
                    let (cx, cy) = (2 * x + 1, 2 * (h - 1 - y) + 1);
                    let (ex, ey) = match placement.orientation.get() & 3 {
                        0 => (cx - 1, cy),
                        1 => (cx, cy - 1),
                        2 => (cx + 1, cy),
                        _ => (cx, cy + 1),
                    };
                    let id = placement.object_id.get();
                    if Some(id) == window {
                        canvas[ey][ex] = 'W';
                    } else if wall_decor.contains(&id) && canvas[ey][ex] == '#' {
                        canvas[ey][ex] = 't';
                    }
                } else if kind == 10 {
                    let letter = match placement.object_id.get() {
                        10355 => 'B',
                        422 => 'b',
                        612 => 'c',
                        604 => 'T',
                        1102 => 's',
                        362 => 'r',
                        354 | 365 => 'x',
                        375 => 'h',
                        26181 => 'R',
                        873 => 'n',
                        1013 | 1015 => 'f',
                        348 => 'd',
                        381 => 'l',
                        409 => 'A',
                        _ => 'o',
                    };
                    furniture.push((x, y, letter));
                } else if kind == 0 {
                    let symbol = if placement.object_id.get() == door {
                        'D'
                    } else {
                        '#'
                    };
                    let (cx, cy) = (2 * x + 1, 2 * (h - 1 - y) + 1);
                    // Orientation 0 west, 1 north, 2 east, 3 south.
                    let (ex, ey) = match placement.orientation.get() & 3 {
                        0 => (cx - 1, cy),
                        1 => (cx, cy - 1),
                        2 => (cx + 1, cy),
                        _ => (cx, cy + 1),
                    };
                    canvas[ey][ex] = symbol;
                } else if (12..=21).contains(&kind) {
                    roof[y][x] = match kind {
                        12 => 's',
                        15 => 'i',
                        16 => 'c',
                        17 => 'F',
                        18 => 'e',
                        19 => 'E',
                        20 => 'I',
                        _ => '?',
                    };
                }
            }
            _ => {}
        }
    }
    for y in 0..h {
        for x in 0..w {
            if floor[y][x] {
                canvas[2 * (h - 1 - y) + 1][2 * x + 1] = '.';
            }
        }
    }
    for (x, y, letter) in furniture {
        canvas[2 * (h - 1 - y) + 1][2 * x + 1] = letter;
    }
    println!(
        "{archetype:?} seed {seed} {width}x{height}: walls # (t torch) door D window W, furniture B booth b bed c counter T table s stool r barrel x crate h chest R range n sink f shelves d drawers l bookcase A altar, floor ."
    );
    for row in &canvas {
        println!("{}", row.iter().collect::<String>());
    }
    println!("roof (north up): s slope, c outer corner, i inner corner, F flat, e/E/I edge");
    for y in (0..h).rev() {
        println!("{}", roof[y].iter().collect::<String>());
    }
    Ok(())
}
