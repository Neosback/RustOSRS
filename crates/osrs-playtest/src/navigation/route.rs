use super::{Navigation, can_travel};
use osrs_core::{
    coords::MapTile,
    scenario::{ActorLocation, MovementRestriction},
};
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy)]
pub struct RouteTarget {
    pub location: ActorLocation,
    pub width: u16,
    pub height: u16,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    AlreadyThere,
    Found {
        waypoints: Vec<ActorLocation>,
        exact: bool,
    },
    Unreachable,
}
/// Bounded 128×128 BFS with RS Mod's neighbour order and nearest-reachable
/// tie breaking. A full queue is represented safely rather than overwriting it.
pub fn find_path(
    view: &dyn Navigation,
    from: ActorLocation,
    target: RouteTarget,
    size: u16,
    restriction: MovementRestriction,
    move_near: bool,
) -> Route {
    if from.plane != target.location.plane
        || size == 0
        || size > 128
        || target.width == 0
        || target.height == 0
    {
        return Route::Unreachable;
    }
    if from == target.location {
        return Route::AlreadyThere;
    }
    let Some(base_x) = from.tile.x.checked_sub(64) else {
        return Route::Unreachable;
    };
    let Some(base_y) = from.tile.y.checked_sub(64) else {
        return Route::Unreachable;
    };
    let mut distances = vec![u32::MAX; 128 * 128];
    let mut parents = vec![None; 128 * 128];
    let start = 64 * 128 + 64;
    distances[start] = 0;
    let mut queue = VecDeque::from([start]);
    let mut found = None;
    while let Some(index) = queue.pop_front() {
        let x = (index % 128) as i32;
        let y = (index / 128) as i32;
        let location = ActorLocation {
            tile: MapTile::new(base_x + x, base_y + y),
            plane: from.plane,
        };
        if location == target.location {
            found = Some(index);
            break;
        }
        for (dx, dy) in [
            (-1, 0),
            (1, 0),
            (0, -1),
            (0, 1),
            (-1, -1),
            (1, -1),
            (-1, 1),
            (1, 1),
        ] {
            let (nx, ny) = (x + dx, y + dy);
            if nx < 0 || ny < 0 || nx + i32::from(size) > 128 || ny + i32::from(size) > 128 {
                continue;
            }
            let next = ny as usize * 128 + nx as usize;
            if distances[next] == u32::MAX
                && can_travel(view, location, dx, dy, size, restriction, 0)
            {
                distances[next] = distances[index] + 1;
                parents[next] = Some(index);
                queue.push_back(next);
            }
        }
    }
    let exact = found.is_some();
    if found.is_none() && move_near {
        let mut best = (i64::MAX, u32::MAX);
        for x in 0..128 {
            for y in 0..128 {
                let index = y * 128 + x;
                if distances[index] >= 100 {
                    continue;
                }
                let wx = base_x + x as i32;
                let wy = base_y + y as i32;
                let tx = i64::from(target.location.tile.x);
                let ty = i64::from(target.location.tile.y);
                let dx = (tx - i64::from(wx))
                    .max(0)
                    .max(i64::from(wx) - tx - i64::from(target.width) + 1);
                let dy = (ty - i64::from(wy))
                    .max(0)
                    .max(i64::from(wy) - ty - i64::from(target.height) + 1);
                if dx > 10 || dy > 10 {
                    continue;
                }
                let score = (dx * dx + dy * dy, distances[index]);
                if score < best {
                    best = score;
                    found = Some(index);
                }
            }
        }
    }
    let Some(mut index) = found else {
        return Route::Unreachable;
    };
    if index == start {
        return if exact {
            Route::AlreadyThere
        } else {
            Route::Unreachable
        };
    }
    let mut steps = Vec::new();
    while index != start {
        steps.push(index);
        let Some(parent) = parents[index] else {
            return Route::Unreachable;
        };
        index = parent;
    }
    steps.reverse();
    let mut waypoints = Vec::new();
    let mut previous = start;
    for (i, &index) in steps.iter().enumerate() {
        let direction = (index % 128) as i32 - (previous % 128) as i32;
        let vertical = (index / 128) as i32 - (previous / 128) as i32;
        let next = steps.get(i + 1).map(|n| {
            (
                (n % 128) as i32 - (index % 128) as i32,
                (n / 128) as i32 - (index / 128) as i32,
            )
        });
        if next != Some((direction, vertical)) {
            waypoints.push(ActorLocation {
                tile: MapTile::new(base_x + (index % 128) as i32, base_y + (index / 128) as i32),
                plane: from.plane,
            });
        }
        previous = index;
    }
    waypoints.truncate(25);
    Route::Found { waypoints, exact }
}
