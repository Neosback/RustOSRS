//! Rebuildable initial-placement clipping, independent of renderable admission.
//!
//! [`CollisionSnapshotBuilder`] consumes loaded terrain and the existing side-effect plans.
//! Snapshots retain original placement provenance; bridge collision projection never rewrites it.
//! See `docs/specs/collision.md` for source pins, padding and supported scope.

mod snapshot;
mod stencil;

pub use snapshot::{CollisionCell, CollisionObject, CollisionSnapshot, CollisionSnapshotBuilder};
pub use stencil::{CollisionPlanError, CollisionStencil};

/// Object footprint blocks walking.
pub const OBJECT: u32 = 256;
/// Object footprint blocks projectiles independently of walking walls.
pub const PROJECTILE_OBJECT: u32 = 131_072;
/// Interactable floor decoration blocks walking.
pub const FLOOR_DECORATION: u32 = 262_144;
/// Terrain setting bit 1 blocks walking on its projected collision plane.
pub const FLOOR: u32 = 2_097_152;
/// Terrain has not been loaded into the reference collision map.
pub const UNLOADED: u32 = 1_073_741_824;

/// Walking wall flags in counter-clockwise order, starting north-west.
pub const WALL_DIRECTIONS: [(u32, &str); 8] = [
    (1, "North-west"),
    (2, "North"),
    (4, "North-east"),
    (8, "East"),
    (16, "South-east"),
    (32, "South"),
    (64, "South-west"),
    (128, "West"),
];

#[cfg(test)]
mod tests;
