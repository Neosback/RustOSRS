//! Directional stencil port from rsmod-pathfinder-assemblyscript 5.0.1 (ISC).
//! Keep masks/visit order aligned with the pinned source and differential oracle.
use super::{Navigation, flags};
use osrs_core::{
    coords::{MapTile, PlaneIndex},
    scenario::{ActorLocation, MovementRestriction},
};

pub(super) struct Validator<'a> {
    pub view: &'a dyn Navigation,
    pub restriction: MovementRestriction,
}
impl Validator<'_> {
    pub fn can_travel(
        &self,
        source: ActorLocation,
        dx: i32,
        dz: i32,
        size: u16,
        extra_flag: u32,
    ) -> bool {
        if size == 0 || size > 256 {
            return false;
        }
        let (level, x, z, size) = (
            i32::from(source.plane.get()),
            source.tile.x,
            source.tile.y,
            i32::from(size),
        );
        match (dx, dz) {
            (0, -1) => !self.is_blocked_south(level, x, z, size, extra_flag),
            (0, 1) => !self.is_blocked_north(level, x, z, size, extra_flag),
            (-1, 0) => !self.is_blocked_west(level, x, z, size, extra_flag),
            (1, 0) => !self.is_blocked_east(level, x, z, size, extra_flag),
            (-1, -1) => !self.is_blocked_south_west(level, x, z, size, extra_flag),
            (-1, 1) => !self.is_blocked_north_west(level, x, z, size, extra_flag),
            (1, -1) => !self.is_blocked_south_east(level, x, z, size, extra_flag),
            (1, 1) => !self.is_blocked_north_east(level, x, z, size, extra_flag),
            _ => false,
        }
    }
    fn get(&self, x: i32, z: i32, level: i32) -> Option<super::NavigationCell> {
        self.view.cell(ActorLocation {
            tile: MapTile::new(x, z),
            plane: PlaneIndex::new(u8::try_from(level).ok()?)?,
        })
    }
    fn can_move(&self, cell: Option<super::NavigationCell>, mask: u32) -> bool {
        let Some(cell) = cell else {
            return false;
        };
        let f = cell.flags;
        match self.restriction {
            MovementRestriction::NoMove => false,
            MovementRestriction::Normal | MovementRestriction::PassThru => f & mask == 0,
            MovementRestriction::Blocked => {
                f & (mask & !flags::FLOOR) == 0 && f & flags::FLOOR != 0
            }
            MovementRestriction::Indoors => f & mask == 0 && cell.roof,
            MovementRestriction::Outdoors => f & mask == 0 && !cell.roof,
            MovementRestriction::BlockedNormal => {
                let translated = ((mask & 0x1ff) << 9) | ((mask & 0x7fc00000) >> 13);
                f & translated == 0
            }
        }
    }
    fn is_blocked_south(&self, level: i32, x: i32, z: i32, size: i32, extra_flag: u32) -> bool {
        if size == 1 {
            !self.can_move(self.get(x, z - 1, level), flags::BLOCK_SOUTH | extra_flag)
        } else if size == 2 {
            !self.can_move(
                self.get(x, z - 1, level),
                flags::BLOCK_SOUTH_WEST | extra_flag,
            ) || !self.can_move(
                self.get(x + 1, z - 1, level),
                flags::BLOCK_SOUTH_EAST | extra_flag,
            )
        } else {
            if !self.can_move(
                self.get(x, z - 1, level),
                flags::BLOCK_SOUTH_WEST | extra_flag,
            ) || !self.can_move(
                self.get(x + size - 1, z - 1, level),
                flags::BLOCK_SOUTH_EAST | extra_flag,
            ) {
                return true;
            }
            for mid_x in x + 1..x + size - 1 {
                if !self.can_move(
                    self.get(mid_x, z - 1, level),
                    flags::BLOCK_NORTH_EAST_AND_WEST | extra_flag,
                ) {
                    return true;
                }
            }
            false
        }
    }

    fn is_blocked_north(&self, level: i32, x: i32, z: i32, size: i32, extra_flag: u32) -> bool {
        if size == 1 {
            !self.can_move(self.get(x, z + 1, level), flags::BLOCK_NORTH | extra_flag)
        } else if size == 2 {
            !self.can_move(
                self.get(x, z + 2, level),
                flags::BLOCK_NORTH_WEST | extra_flag,
            ) || !self.can_move(
                self.get(x + 1, z + 2, level),
                flags::BLOCK_NORTH_EAST | extra_flag,
            )
        } else {
            if !self.can_move(
                self.get(x, z + size, level),
                flags::BLOCK_NORTH_WEST | extra_flag,
            ) || !self.can_move(
                self.get(x + size - 1, z + size, level),
                flags::BLOCK_NORTH_EAST | extra_flag,
            ) {
                return true;
            }
            for mid_x in x + 1..x + size - 1 {
                if !self.can_move(
                    self.get(mid_x, z + size, level),
                    flags::BLOCK_SOUTH_EAST_AND_WEST | extra_flag,
                ) {
                    return true;
                }
            }
            false
        }
    }

    fn is_blocked_west(&self, level: i32, x: i32, z: i32, size: i32, extra_flag: u32) -> bool {
        if size == 1 {
            !self.can_move(self.get(x - 1, z, level), flags::BLOCK_WEST | extra_flag)
        } else if size == 2 {
            !self.can_move(
                self.get(x - 1, z, level),
                flags::BLOCK_SOUTH_WEST | extra_flag,
            ) || !self.can_move(
                self.get(x - 1, z + 1, level),
                flags::BLOCK_NORTH_WEST | extra_flag,
            )
        } else {
            if !self.can_move(
                self.get(x - 1, z, level),
                flags::BLOCK_SOUTH_WEST | extra_flag,
            ) || !self.can_move(
                self.get(x - 1, z + size - 1, level),
                flags::BLOCK_NORTH_WEST | extra_flag,
            ) {
                return true;
            }
            for mid_z in z + 1..z + size - 1 {
                if !self.can_move(
                    self.get(x - 1, mid_z, level),
                    flags::BLOCK_NORTH_AND_SOUTH_EAST | extra_flag,
                ) {
                    return true;
                }
            }
            false
        }
    }

    fn is_blocked_east(&self, level: i32, x: i32, z: i32, size: i32, extra_flag: u32) -> bool {
        if size == 1 {
            !self.can_move(self.get(x + 1, z, level), flags::BLOCK_EAST | extra_flag)
        } else if size == 2 {
            !self.can_move(
                self.get(x + 2, z, level),
                flags::BLOCK_SOUTH_EAST | extra_flag,
            ) || !self.can_move(
                self.get(x + 2, z + 1, level),
                flags::BLOCK_NORTH_EAST | extra_flag,
            )
        } else {
            if !self.can_move(
                self.get(x + size, z, level),
                flags::BLOCK_SOUTH_EAST | extra_flag,
            ) || !self.can_move(
                self.get(x + size, z + size - 1, level),
                flags::BLOCK_NORTH_EAST | extra_flag,
            ) {
                return true;
            }
            for mid_z in z + 1..z + size - 1 {
                if !self.can_move(
                    self.get(x + size, mid_z, level),
                    flags::BLOCK_NORTH_AND_SOUTH_WEST | extra_flag,
                ) {
                    return true;
                }
            }
            false
        }
    }

    fn is_blocked_south_west(
        &self,
        level: i32,
        x: i32,
        z: i32,
        size: i32,
        extra_flag: u32,
    ) -> bool {
        if size == 1 {
            !self.can_move(
                self.get(x - 1, z - 1, level),
                flags::BLOCK_SOUTH_WEST | extra_flag,
            ) || !self.can_move(self.get(x - 1, z, level), flags::BLOCK_WEST | extra_flag)
                || !self.can_move(self.get(x, z - 1, level), flags::BLOCK_SOUTH | extra_flag)
        } else if size == 2 {
            !self.can_move(
                self.get(x - 1, z, level),
                flags::BLOCK_NORTH_AND_SOUTH_EAST | extra_flag,
            ) || !self.can_move(
                self.get(x - 1, z - 1, level),
                flags::BLOCK_SOUTH_WEST | extra_flag,
            ) || !self.can_move(
                self.get(x, z - 1, level),
                flags::BLOCK_NORTH_EAST_AND_WEST | extra_flag,
            )
        } else {
            if !self.can_move(
                self.get(x - 1, z - 1, level),
                flags::BLOCK_SOUTH_WEST | extra_flag,
            ) {
                return true;
            }
            for mid in 1..size {
                if !self.can_move(
                    self.get(x - 1, z + mid - 1, level),
                    flags::BLOCK_NORTH_AND_SOUTH_EAST | extra_flag,
                ) || !self.can_move(
                    self.get(x + mid - 1, z - 1, level),
                    flags::BLOCK_NORTH_EAST_AND_WEST | extra_flag,
                ) {
                    return true;
                }
            }
            false
        }
    }

    fn is_blocked_north_west(
        &self,
        level: i32,
        x: i32,
        z: i32,
        size: i32,
        extra_flag: u32,
    ) -> bool {
        if size == 1 {
            !self.can_move(
                self.get(x - 1, z + 1, level),
                flags::BLOCK_NORTH_WEST | extra_flag,
            ) || !self.can_move(self.get(x - 1, z, level), flags::BLOCK_WEST | extra_flag)
                || !self.can_move(self.get(x, z + 1, level), flags::BLOCK_NORTH | extra_flag)
        } else if size == 2 {
            !self.can_move(
                self.get(x - 1, z + 1, level),
                flags::BLOCK_NORTH_AND_SOUTH_EAST | extra_flag,
            ) || !self.can_move(
                self.get(x - 1, z + 2, level),
                flags::BLOCK_NORTH_WEST | extra_flag,
            ) || !self.can_move(
                self.get(x, z + 2, level),
                flags::BLOCK_SOUTH_EAST_AND_WEST | extra_flag,
            )
        } else {
            if !self.can_move(
                self.get(x - 1, z + size, level),
                flags::BLOCK_NORTH_WEST | extra_flag,
            ) {
                return true;
            }
            for mid in 1..size {
                if !self.can_move(
                    self.get(x - 1, z + mid, level),
                    flags::BLOCK_NORTH_AND_SOUTH_EAST | extra_flag,
                ) || !self.can_move(
                    self.get(x + mid - 1, z + size, level),
                    flags::BLOCK_SOUTH_EAST_AND_WEST | extra_flag,
                ) {
                    return true;
                }
            }
            false
        }
    }

    fn is_blocked_south_east(
        &self,
        level: i32,
        x: i32,
        z: i32,
        size: i32,
        extra_flag: u32,
    ) -> bool {
        if size == 1 {
            !self.can_move(
                self.get(x + 1, z - 1, level),
                flags::BLOCK_SOUTH_EAST | extra_flag,
            ) || !self.can_move(self.get(x + 1, z, level), flags::BLOCK_EAST | extra_flag)
                || !self.can_move(self.get(x, z - 1, level), flags::BLOCK_SOUTH | extra_flag)
        } else if size == 2 {
            !self.can_move(
                self.get(x + 1, z - 1, level),
                flags::BLOCK_NORTH_EAST_AND_WEST | extra_flag,
            ) || !self.can_move(
                self.get(x + 2, z - 1, level),
                flags::BLOCK_SOUTH_EAST | extra_flag,
            ) || !self.can_move(
                self.get(x + 2, z, level),
                flags::BLOCK_NORTH_AND_SOUTH_WEST | extra_flag,
            )
        } else {
            for mid in 1..size {
                if !self.can_move(
                    self.get(x + size, z + mid - 1, level),
                    flags::BLOCK_NORTH_AND_SOUTH_WEST | extra_flag,
                ) || !self.can_move(
                    self.get(x + mid, z - 1, level),
                    flags::BLOCK_NORTH_EAST_AND_WEST | extra_flag,
                ) {
                    return true;
                }
            }
            false
        }
    }

    fn is_blocked_north_east(
        &self,
        level: i32,
        x: i32,
        z: i32,
        size: i32,
        extra_flag: u32,
    ) -> bool {
        if size == 1 {
            !self.can_move(
                self.get(x + 1, z + 1, level),
                flags::BLOCK_NORTH_EAST | extra_flag,
            ) || !self.can_move(self.get(x + 1, z, level), flags::BLOCK_EAST | extra_flag)
                || !self.can_move(self.get(x, z + 1, level), flags::BLOCK_NORTH | extra_flag)
        } else if size == 2 {
            !self.can_move(
                self.get(x + 1, z + 2, level),
                flags::BLOCK_SOUTH_EAST_AND_WEST | extra_flag,
            ) || !self.can_move(
                self.get(x + 2, z + 2, level),
                flags::BLOCK_NORTH_EAST | extra_flag,
            ) || !self.can_move(
                self.get(x + 2, z + 1, level),
                flags::BLOCK_NORTH_AND_SOUTH_WEST | extra_flag,
            )
        } else {
            if !self.can_move(
                self.get(x + size, z + size, level),
                flags::BLOCK_NORTH_EAST | extra_flag,
            ) {
                return true;
            }
            for mid in 1..size {
                if !self.can_move(
                    self.get(x + mid, z + size, level),
                    flags::BLOCK_SOUTH_EAST_AND_WEST | extra_flag,
                ) || !self.can_move(
                    self.get(x + size, z + mid, level),
                    flags::BLOCK_NORTH_AND_SOUTH_WEST | extra_flag,
                ) {
                    return true;
                }
            }
            false
        }
    }
}
