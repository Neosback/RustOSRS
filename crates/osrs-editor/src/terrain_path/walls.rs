//! Castle walls along a path: the band of tiles, then wall objects on its outward faces (the old
//! WALL brush style).
//!
//! A band tile grows walls only on the sides facing out of the band; diagonal tiles get diagonal
//! walls when the band tangent runs diagonally and a diagonal id is configured. Walls stack
//! `height` planes tall with a cap row on top. Output is [`LocSpec`] placements for the editor's
//! object-edit layer, so a WALL commit undoes together with its overlay paint.

use osrs_world::LocSpec;
use std::collections::{HashMap, HashSet};

/// Wall object ids; 0 = unset (top and door fall back to side, as in the old tool).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WallKit {
    pub side: u32,
    pub top: u32,
    pub door: u32,
    pub diagonal: u32,
}

/// Straight-wall rotation per outward-mask bit (the old `osrsWallRotation` table).
const WALL_ROTATION: [u8; 4] = [1, 0, 3, 2];

/// Highest plane walls may stack to.
const TOP_PLANE: u8 = 3;

/// Whether a band tangent runs diagonally (the old `isDiagonal`: the smaller axis exceeds 0.4 of
/// the larger).
pub fn is_diagonal(tangent: (f32, f32)) -> bool {
    let (x, y) = (tangent.0.abs(), tangent.1.abs());
    if x < 1e-4 && y < 1e-4 {
        return false;
    }
    let (small, large) = if x < y { (x, y) } else { (y, x) };
    large > 0.0 && small / large > 0.4
}

/// Diagonal wall rotation from the tangent's sign (the old `diagonalRotationGuess`).
fn diagonal_rotation(tangent: (f32, f32)) -> u8 {
    if tangent.0 * tangent.1 >= 0.0 { 0 } else { 1 }
}

/// Outward mask of `tile` against `set` (the old `outwardMask`): a bit is set per side whose
/// neighbour is not in the set. 0 means interior (no walls).
fn outward_mask(set: &HashSet<(i32, i32)>, tile: (i32, i32)) -> u8 {
    let mut mask = 0_u8;
    if !set.contains(&(tile.0, tile.1 - 1)) {
        mask |= 1;
    }
    if !set.contains(&(tile.0 + 1, tile.1)) {
        mask |= 2;
    }
    if !set.contains(&(tile.0, tile.1 + 1)) {
        mask |= 4;
    }
    if !set.contains(&(tile.0 - 1, tile.1)) {
        mask |= 8;
    }
    mask
}

/// Spawn specs for every band tile: walls on outward faces (type 0) or diagonal walls (type 9)
/// where the tangent runs diagonally, stacked from `plane` up `height` planes with a cap row.
/// Tiles are visited in sorted order so commits are deterministic. Segments above plane 3 are
/// dropped (the old tool always built from plane 0; walls stand on the commit plane here).
pub fn emit_walls(
    tiles: &HashSet<(i32, i32)>,
    tangents: &HashMap<(i32, i32), (f32, f32)>,
    kit: &WallKit,
    height: u32,
    plane: u8,
) -> Vec<LocSpec> {
    let mut specs = Vec::new();
    if kit.side == 0 || height == 0 {
        return specs;
    }
    let height = height.min(3);
    let top = if kit.top > 0 { kit.top } else { kit.side };
    let mut ordered: Vec<_> = tiles.iter().copied().collect();
    ordered.sort();
    for tile in ordered {
        let mask = outward_mask(tiles, tile);
        if mask == 0 {
            continue;
        }
        let mut push = |object_id: u32, loc_type: u8, orientation: u8, level: u32| {
            if level <= u32::from(TOP_PLANE) {
                specs.push(LocSpec {
                    object_id,
                    tile,
                    plane: level as u8,
                    loc_type,
                    orientation,
                });
            }
        };
        let base = u32::from(plane);
        if tangents
            .get(&tile)
            .is_some_and(|tangent| kit.diagonal > 0 && is_diagonal(*tangent))
        {
            let rotation = diagonal_rotation(tangents[&tile]);
            for level in 0..height {
                push(kit.diagonal, 9, rotation, base + level);
            }
            push(top, 9, rotation, base + height);
        } else {
            for bit in 0..4_u8 {
                if mask & (1 << bit) == 0 {
                    continue;
                }
                let rotation = WALL_ROTATION[usize::from(bit)];
                for level in 0..height {
                    push(kit.side, 0, rotation, base + level);
                }
                push(top, 0, rotation, base + height);
            }
        }
    }
    specs
}

/// A rasterized castle-wall band: the tiles plus the averaged unit tangent per tile (wall
/// direction info for diagonal detection).
pub struct BandTiles {
    pub tiles: HashSet<(i32, i32)>,
    pub tangents: HashMap<(i32, i32), (f32, f32)>,
}

/// Stamp the tiles a dense polyline covers `thickness` tiles wide, walking each sample's
/// perpendicular, and average the unit tangent per tile. Tiles a straight run crosses are stamped
/// symmetrically; even thicknesses shift the far edge in by one so the count stays exact.
pub fn rasterize_band(line: &[(f32, f32)], thickness: f32) -> BandTiles {
    let mut tiles = HashSet::new();
    let mut tangents: HashMap<(i32, i32), (f32, f32)> = HashMap::new();
    if line.len() < 2 {
        return BandTiles { tiles, tangents };
    }
    let count = thickness.max(1.0).round() as i32;
    let half = count / 2;
    let even = count % 2 == 0;
    for (index, &(x, y)) in line.iter().enumerate() {
        // Central difference along the dense polyline; forward/backward at the ends.
        let (tx, ty) = if index == 0 {
            (line[1].0 - x, line[1].1 - y)
        } else if index + 1 == line.len() {
            (x - line[index - 1].0, y - line[index - 1].1)
        } else {
            (
                line[index + 1].0 - line[index - 1].0,
                line[index + 1].1 - line[index - 1].1,
            )
        };
        let (mut dx, mut dy) = (tx, ty);
        let mut length = (tx * tx + ty * ty).sqrt();
        if length < 1e-4 {
            dx = 1.0;
            dy = 0.0;
            length = 1.0;
        }
        let (nx, ny) = (-dy / length, dx / length);
        let (ux, uy) = (dx / length, dy / length);
        // Tiles use floor coordinates throughout the editor (a point at 0.5 sits in tile 0).
        for k in -half..=(half - if even { 1 } else { 0 }) {
            let tile = (
                (x + nx * k as f32).floor() as i32,
                (y + ny * k as f32).floor() as i32,
            );
            tiles.insert(tile);
            tangents
                .entry(tile)
                .and_modify(|tangent| {
                    tangent.0 = (tangent.0 + ux) * 0.5;
                    tangent.1 = (tangent.1 + uy) * 0.5;
                })
                .or_insert((ux, uy));
        }
    }
    BandTiles { tiles, tangents }
}

/// A castle wall's band: whole tiles (the band plus the control-point tower squares) with
/// per-tile tangents for wall emission. Walls carry the visuals; the tiles are laid whole.
pub struct WallPlan {
    pub tiles: Vec<(i32, i32)>,
    pub tangents: HashMap<(i32, i32), (f32, f32)>,
}

/// Lay a castle-wall band `thickness` tiles wide along `line` with `tower_size` squares around
/// each control point (size 1 disables towers).
pub fn generate_walls(
    line: &[(f32, f32)],
    points: &[(f32, f32)],
    thickness: f32,
    tower_size: f32,
) -> WallPlan {
    let mut plan = WallPlan {
        tiles: Vec::new(),
        tangents: HashMap::new(),
    };
    if points.len() < 2 || line.len() < 2 {
        return plan;
    }
    let band = rasterize_band(line, thickness);
    let mut tiles = band.tiles;
    let size = tower_size.max(1.0).round() as i32;
    if size > 1 {
        let half = size / 2;
        let far = size - 1 - half;
        for &(x, y) in points {
            let (cx, cy) = (x.floor() as i32, y.floor() as i32);
            for dx in -half..=far {
                for dy in -half..=far {
                    tiles.insert((cx + dx, cy + dy));
                }
            }
        }
    }
    let mut tiles: Vec<_> = tiles.into_iter().collect();
    // Deterministic order (a hash set has none).
    tiles.sort_unstable();
    plan.tiles = tiles;
    plan.tangents = band.tangents;
    plan
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tile_set(tiles: &[(i32, i32)]) -> HashSet<(i32, i32)> {
        tiles.iter().copied().collect()
    }

    #[test]
    fn outward_mask_flags_only_open_sides() {
        // A horizontal 3-band: middle tile has open south and north.
        let set = tile_set(&[(0, 0), (1, 0), (2, 0)]);
        assert_eq!(outward_mask(&set, (1, 0)), 1 | 4);
        assert_eq!(outward_mask(&set, (0, 0)), 1 | 4 | 8);
        // A lone tile is open on all sides.
        assert_eq!(outward_mask(&tile_set(&[(5, 5)]), (5, 5)), 15);
    }

    #[test]
    fn diagonal_detection_matches_old_threshold() {
        assert!(!is_diagonal((1.0, 0.0)));
        assert!(!is_diagonal((0.0, 1.0)));
        assert!(is_diagonal((1.0, 1.0)));
        assert!(is_diagonal((1.0, -1.0)));
        assert!(!is_diagonal((1.0, 0.39)));
        assert!(is_diagonal((1.0, 0.41)));
        assert!(!is_diagonal((0.0, 0.0)));
    }

    #[test]
    fn straight_band_grows_walls_on_outward_faces_only() {
        let set = tile_set(&[(0, 0), (1, 0), (2, 0)]);
        let kit = WallKit {
            side: 100,
            top: 101,
            door: 0,
            diagonal: 0,
        };
        let specs = emit_walls(&set, &HashMap::new(), &kit, 2, 0);
        // Middle tile: south + north faces, each 2 wall segments + 1 cap.
        let middle: Vec<_> = specs.iter().filter(|s| s.tile == (1, 0)).collect();
        assert_eq!(middle.len(), 6);
        assert!(middle.iter().all(|s| s.loc_type == 0));
        assert!(middle.iter().filter(|s| s.plane < 2).count() == 4);
        assert!(middle.iter().filter(|s| s.plane == 2).count() == 2);
        // Rotations follow the outward-mask-bit table (bit0 -> 1, bit2 -> 3).
        let mut rotations: Vec<u8> = middle.iter().map(|s| s.orientation).collect();
        rotations.sort();
        rotations.dedup();
        assert_eq!(rotations, vec![1, 3]);
        // End tile (0,0): south + north + west faces = 9 specs.
        assert_eq!(specs.iter().filter(|s| s.tile == (0, 0)).count(), 9);
    }

    #[test]
    fn diagonal_band_uses_diagonal_id_and_type_9() {
        let set = tile_set(&[(0, 0)]);
        let mut tangents = HashMap::new();
        tangents.insert((0, 0), (1.0, 1.0));
        let kit = WallKit {
            side: 100,
            top: 0,
            door: 0,
            diagonal: 200,
        };
        let specs = emit_walls(&set, &tangents, &kit, 1, 0);
        // Lone tile: mask 15, but the diagonal branch spawns one stack, not four faces:
        // one diagonal segment plus the cap (top falls back to side when unset).
        assert_eq!(specs.len(), 2);
        assert_eq!(
            specs[0],
            LocSpec {
                object_id: 200,
                tile: (0, 0),
                plane: 0,
                loc_type: 9,
                orientation: 0,
            }
        );
        assert_eq!(
            specs[1],
            LocSpec {
                object_id: 100,
                tile: (0, 0),
                plane: 1,
                loc_type: 9,
                orientation: 0,
            }
        );
    }

    #[test]
    fn no_side_id_spawns_nothing() {
        let set = tile_set(&[(0, 0), (1, 0)]);
        let kit = WallKit {
            side: 0,
            top: 101,
            door: 0,
            diagonal: 200,
        };
        assert!(emit_walls(&set, &HashMap::new(), &kit, 2, 0).is_empty());
    }

    #[test]
    fn walls_stack_is_clamped_to_plane_3() {
        let set = tile_set(&[(0, 0), (1, 0)]);
        let kit = WallKit {
            side: 100,
            top: 101,
            door: 0,
            diagonal: 0,
        };
        let specs = emit_walls(&set, &HashMap::new(), &kit, 3, 2);
        assert!(!specs.is_empty());
        assert!(specs.iter().all(|s| s.plane <= 3));
        // Base plane 2, height 3: segments at 2 and 3, cap would be 4 (dropped).
        assert!(specs.iter().any(|s| s.plane == 2));
        assert!(specs.iter().any(|s| s.plane == 3));
        assert!(specs.iter().all(|s| s.plane != 4));
    }
}
