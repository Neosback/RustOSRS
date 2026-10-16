use crate::{
    CollisionSideEffect,
    collision::{FLOOR_DECORATION, OBJECT, PROJECTILE_OBJECT},
};
use osrs_core::coords::SceneTile;
use std::{error::Error, fmt};

#[derive(Debug, Clone, Copy, Default)]
struct Stamp {
    dx: i32,
    dy: i32,
    width: u32,
    height: u32,
    flags: u32,
}

/// At most three rectangular writes produced by one audited clipping operation.
#[derive(Debug, Clone, Copy)]
pub struct CollisionStencil {
    stamps: [Stamp; 3],
}

/// A manually constructed side-effect recipe was not a supported boundary operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollisionPlanError {
    pub loc_type: u8,
    pub orientation: u8,
}

impl fmt::Display for CollisionPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid boundary clipping type {} / orientation {}",
            self.loc_type, self.orientation
        )
    }
}
impl Error for CollisionPlanError {}

impl CollisionStencil {
    /// Translate an existing placement recipe without interpreting object definitions again.
    ///
    /// # Errors
    /// Rejects boundary kinds outside 0..=3 and orientations outside 0..=3.
    pub fn from_effect(effect: CollisionSideEffect) -> Result<Self, CollisionPlanError> {
        let mut stamps = [Stamp::default(); 3];
        let point = |dx, dy, flags| Stamp {
            dx,
            dy,
            width: 1,
            height: 1,
            flags,
        };
        match effect {
            CollisionSideEffect::None => {}
            CollisionSideEffect::FloorDecorationBlock => stamps[0] = point(0, 0, FLOOR_DECORATION),
            CollisionSideEffect::GameObject {
                footprint,
                blocks_projectiles,
            } => {
                stamps[0] = Stamp {
                    width: u32::from(footprint.width),
                    height: u32::from(footprint.depth),
                    flags: OBJECT
                        | if blocks_projectiles {
                            PROJECTILE_OBJECT
                        } else {
                            0
                        },
                    ..Stamp::default()
                };
            }
            CollisionSideEffect::Boundary {
                loc_type,
                orientation,
                blocks_projectiles,
            } => {
                let angle = usize::from(orientation);
                if angle > 3 || loc_type.get() > 3 {
                    return Err(CollisionPlanError {
                        loc_type: loc_type.get(),
                        orientation,
                    });
                }
                let straight = [
                    (-1, 0, 128, 8),
                    (0, 1, 2, 32),
                    (1, 0, 8, 128),
                    (0, -1, 32, 2),
                ];
                let diagonal = [
                    (-1, 1, 1, 16),
                    (1, 1, 4, 64),
                    (1, -1, 16, 1),
                    (-1, -1, 64, 4),
                ];
                let flags = |walk: u32| walk | if blocks_projectiles { walk << 9 } else { 0 };
                match loc_type.get() {
                    0 | 1 | 3 => {
                        let (dx, dy, own, neighbour) = if loc_type.get() == 0 {
                            straight[angle]
                        } else {
                            diagonal[angle]
                        };
                        stamps[0] = point(0, 0, flags(own));
                        stamps[1] = point(dx, dy, flags(neighbour));
                    }
                    2 => {
                        let (dx, dy, own, neighbour) = straight[angle];
                        let (dx2, dy2, own2, neighbour2) = straight[(angle + 1) % 4];
                        stamps[0] = point(0, 0, flags(own | own2));
                        stamps[1] = point(dx, dy, flags(neighbour));
                        stamps[2] = point(dx2, dy2, flags(neighbour2));
                    }
                    _ => {
                        return Err(CollisionPlanError {
                            loc_type: loc_type.get(),
                            orientation,
                        });
                    }
                }
            }
        }
        Ok(Self { stamps })
    }

    /// Flags this operation contributes to a queried tile. No allocation or grid mutation.
    pub fn flags_at(self, anchor: SceneTile, tile: SceneTile) -> u32 {
        let dx = i64::from(tile.x) - i64::from(anchor.x);
        let dy = i64::from(tile.y) - i64::from(anchor.y);
        self.stamps
            .iter()
            .filter(|stamp| {
                dx >= i64::from(stamp.dx)
                    && dx < i64::from(stamp.dx) + i64::from(stamp.width)
                    && dy >= i64::from(stamp.dy)
                    && dy < i64::from(stamp.dy) + i64::from(stamp.height)
            })
            .fold(0, |flags, stamp| flags | stamp.flags)
    }

    pub(super) fn visit(
        self,
        anchor: SceneTile,
        width: u32,
        height: u32,
        mut write: impl FnMut(SceneTile, u32),
    ) {
        for stamp in self.stamps {
            let x = i64::from(anchor.x) + i64::from(stamp.dx);
            let y = i64::from(anchor.y) + i64::from(stamp.dy);
            for sx in x.max(0)..(x + i64::from(stamp.width)).min(i64::from(width)) {
                for sy in y.max(0)..(y + i64::from(stamp.height)).min(i64::from(height)) {
                    write(SceneTile::new(sx as u32, sy as u32), stamp.flags);
                }
            }
        }
    }
}
