//! Turning what the AI drew into tiles the editor can lay on the map, as one undoable change.

use crate::app::editing::PathTile;
use osrs_lab::{PathAttempt, PathMode, path_waypoints};
use std::collections::BTreeSet;

/// The tiles to lay for `attempt`, which was drawn on an area whose south-west tile is `origin`.
/// Lines are fitted with the path tool's own shapes; painted tiles are laid whole.
pub(super) fn tiles_for(
    origin: [i32; 2],
    size: (usize, usize),
    overlay: u16,
    width: f32,
    attempt: &PathAttempt,
) -> Vec<PathTile> {
    let at = |x: i32, y: i32| (origin[0] + x, origin[1] + y);
    match attempt.mode {
        PathMode::Given => {
            let mut seen = BTreeSet::new();
            let mut out = Vec::new();
            for route in attempt.routes.iter().filter(|r| r.len() >= 2) {
                let points: Vec<(f64, f64)> = path_waypoints(route, 1.2)
                    .into_iter()
                    .map(|p| (p[0], p[1]))
                    .collect();
                let line = osrs_scene::terrain_path::smooth(&points);
                let fitted = osrs_scene::terrain_path::fit_bounded(
                    &line,
                    f64::from(width),
                    1.5,
                    (size.0 as i32, size.1 as i32),
                );
                for tile in fitted {
                    if seen.insert((tile.tile_x, tile.tile_y)) {
                        out.push(PathTile {
                            tile: at(tile.tile_x, tile.tile_y),
                            overlay: Some(overlay),
                            shape: tile.shape,
                            rotation: tile.orientation,
                        });
                    }
                }
            }
            out
        }
        PathMode::Alone => (0..size.0 * size.1)
            .filter(|a| attempt.drawn[*a])
            .map(|a| PathTile {
                tile: at((a % size.0) as i32, (a / size.0) as i32),
                overlay: Some(overlay),
                shape: 0,
                rotation: 0,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_and_painted_tiles_become_map_tiles_at_the_right_place() {
        let routes = vec![(2..30).map(|x| (x, 10)).collect::<Vec<_>>()];
        let drawn = {
            let mut d = vec![false; 40 * 40];
            for x in 2..30 {
                d[10 * 40 + x] = true;
            }
            d
        };
        let line = PathAttempt {
            mode: PathMode::Given,
            chances: vec![],
            drawn: drawn.clone(),
            routes,
            score: None,
            baselines: vec![],
        };
        let tiles = tiles_for([3200, 3200], (40, 40), 10, 3.0, &line);
        assert!(
            tiles.len() > 28,
            "a three-wide path covers more than its centre line: {}",
            tiles.len()
        );
        assert!(
            tiles
                .iter()
                .all(|t| t.tile.0 >= 3200 && t.tile.1 >= 3200 + 8 && t.overlay == Some(10))
        );
        let painted = PathAttempt {
            mode: PathMode::Alone,
            chances: vec![],
            drawn,
            routes: vec![],
            score: None,
            baselines: vec![],
        };
        let whole = tiles_for([3200, 3200], (40, 40), 10, 3.0, &painted);
        assert_eq!(whole.len(), 28);
        assert!(whole.iter().all(|t| t.shape == 0 && t.tile.1 == 3210));
    }
}
