//! Portable navigation. The collision provider must reject stale/unloaded cells;
//! client flags are translated explicitly instead of sharing pathfinder bit IDs.
pub mod flags;
mod route;
mod step;
use osrs_core::scenario::{ActorLocation, MovementRestriction};
pub use route::{Route, RouteTarget, find_path};

#[derive(Debug, Clone, Copy)]
pub struct NavigationCell {
    pub flags: u32,
    pub roof: bool,
}
impl NavigationCell {
    /// Client UNLOADED (0x40000000) collides with the pathfinder's route-object
    /// flag. Never copy it into navigation or treat unloaded land as open.
    pub fn from_client(flags: u32, loaded: bool, roof: bool) -> Option<Self> {
        (loaded && flags & 0x40000000 == 0).then_some(Self {
            flags: flags & 0x27ffff,
            roof,
        })
    }
}
/// Immutable accepted world view; actor occupancy can be layered by a caller.
pub trait Navigation {
    fn cell(&self, location: ActorLocation) -> Option<NavigationCell>;
}

/// Size-aware directional stencil, including both orthogonal corner checks.
pub fn can_travel(
    view: &dyn Navigation,
    from: ActorLocation,
    dx: i32,
    dz: i32,
    size: u16,
    restriction: MovementRestriction,
    extra_flag: u32,
) -> bool {
    if from.tile.x.checked_add(i32::from(size) + 2).is_none()
        || from.tile.y.checked_add(i32::from(size) + 2).is_none()
        || from.tile.x.checked_sub(2).is_none()
        || from.tile.y.checked_sub(2).is_none()
        || from.tile.x.checked_add(dx).is_none()
        || from.tile.y.checked_add(dz).is_none()
    {
        return false;
    }
    step::Validator { view, restriction }.can_travel(from, dx, dz, size, extra_flag)
}

/// OpenRune StepFactory order: diagonal, horizontal, then vertical. This is
/// intentionally distinct from breadth-first routing around obstacles.
pub fn validated_step(
    view: &dyn Navigation,
    from: ActorLocation,
    destination: ActorLocation,
    size: u16,
    restriction: MovementRestriction,
    extra_flag: u32,
) -> Option<ActorLocation> {
    if from.plane != destination.plane || from == destination {
        return None;
    }
    let dx = (i64::from(destination.tile.x) - i64::from(from.tile.x)).signum() as i32;
    let dz = (i64::from(destination.tile.y) - i64::from(from.tile.y)).signum() as i32;
    for (dx, dz) in [(dx, dz), (dx, 0), (0, dz)] {
        if (dx != 0 || dz != 0) && can_travel(view, from, dx, dz, size, restriction, extra_flag) {
            return Some(ActorLocation {
                tile: osrs_core::coords::MapTile::new(from.tile.x + dx, from.tile.y + dz),
                plane: from.plane,
            });
        }
    }
    None
}
