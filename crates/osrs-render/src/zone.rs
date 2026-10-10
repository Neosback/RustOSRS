use crate::SemanticGeneration;
use osrs_core::coords::StoragePlane;
use std::{collections::BTreeMap, error::Error, fmt};

/// Number of semantic tiles along one renderer zone axis.
pub const RENDER_ZONE_SIZE_TILES: i32 = 8;

/// Stable renderer zone identity in semantic tile space.
///
/// Zone coordinates use Euclidean division so negative world/local tile
/// coordinates partition exactly like positive coordinates. Storage plane is
/// part of the identity because bridge relinking changes scene ownership
/// without rewriting source-plane provenance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RenderZoneKey {
    plane: StoragePlane,
    x: i32,
    z: i32,
}

impl RenderZoneKey {
    pub const fn new(plane: StoragePlane, x: i32, z: i32) -> Self {
        Self { plane, x, z }
    }

    pub fn from_tile(plane: StoragePlane, tile_x: i32, tile_z: i32) -> Self {
        Self::new(
            plane,
            tile_x.div_euclid(RENDER_ZONE_SIZE_TILES),
            tile_z.div_euclid(RENDER_ZONE_SIZE_TILES),
        )
    }

    pub const fn plane(self) -> StoragePlane {
        self.plane
    }

    pub const fn x(self) -> i32 {
        self.x
    }

    pub const fn z(self) -> i32 {
        self.z
    }
}

/// Inclusive semantic tile bounds used to derive every touched 8x8 zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderTileBounds {
    min_x: i32,
    min_z: i32,
    max_x: i32,
    max_z: i32,
}

impl RenderTileBounds {
    pub fn from_min_size(
        min_x: i32,
        min_z: i32,
        width_tiles: u32,
        depth_tiles: u32,
    ) -> Result<Self, RenderZoneError> {
        if width_tiles == 0 {
            return Err(RenderZoneError::ZeroWidth);
        }
        if depth_tiles == 0 {
            return Err(RenderZoneError::ZeroDepth);
        }

        let x_extent = i32::try_from(width_tiles - 1)
            .map_err(|_| RenderZoneError::WidthTooLarge { width_tiles })?;
        let z_extent = i32::try_from(depth_tiles - 1)
            .map_err(|_| RenderZoneError::DepthTooLarge { depth_tiles })?;
        let max_x = min_x
            .checked_add(x_extent)
            .ok_or(RenderZoneError::XBoundsOverflow)?;
        let max_z = min_z
            .checked_add(z_extent)
            .ok_or(RenderZoneError::ZBoundsOverflow)?;

        Ok(Self {
            min_x,
            min_z,
            max_x,
            max_z,
        })
    }

    pub const fn min_x(self) -> i32 {
        self.min_x
    }

    pub const fn min_z(self) -> i32 {
        self.min_z
    }

    pub const fn max_x(self) -> i32 {
        self.max_x
    }

    pub const fn max_z(self) -> i32 {
        self.max_z
    }

    /// Enumerate every touched zone in deterministic z-major, then x-major order.
    pub fn zones(self, plane: StoragePlane) -> impl Iterator<Item = RenderZoneKey> {
        let min_zone_x = self.min_x.div_euclid(RENDER_ZONE_SIZE_TILES);
        let max_zone_x = self.max_x.div_euclid(RENDER_ZONE_SIZE_TILES);
        let min_zone_z = self.min_z.div_euclid(RENDER_ZONE_SIZE_TILES);
        let max_zone_z = self.max_z.div_euclid(RENDER_ZONE_SIZE_TILES);

        (min_zone_z..=max_zone_z).flat_map(move |z| {
            (min_zone_x..=max_zone_x).map(move |x| RenderZoneKey::new(plane, x, z))
        })
    }
}

/// Immutable ownership token for one in-flight zone build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ZoneBuildTicket {
    key: RenderZoneKey,
    required_generation: SemanticGeneration,
}

impl ZoneBuildTicket {
    pub const fn key(self) -> RenderZoneKey {
        self.key
    }

    pub const fn required_generation(self) -> SemanticGeneration {
        self.required_generation
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ZoneState {
    required_generation: SemanticGeneration,
    dirty: bool,
}

/// Renderer-owned per-zone invalidation state.
///
/// A newer global semantic generation invalidates only the zones supplied by
/// the caller's semantic diff. Unchanged clean zones therefore remain reusable.
/// A build ticket is accepted only if its zone still requires the same semantic
/// generation, preventing late stale work from replacing newer zone content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneInvalidationTracker {
    current_generation: SemanticGeneration,
    zones: BTreeMap<RenderZoneKey, ZoneState>,
}

impl ZoneInvalidationTracker {
    pub fn new(initial_generation: SemanticGeneration) -> Self {
        Self {
            current_generation: initial_generation,
            zones: BTreeMap::new(),
        }
    }

    pub const fn current_generation(&self) -> SemanticGeneration {
        self.current_generation
    }

    /// Mark one zone dirty for the current semantic generation.
    ///
    /// Repeating this within the same generation is intentionally idempotent.
    pub fn invalidate_zone(&mut self, key: RenderZoneKey) {
        self.invalidate_for_generation(key, self.current_generation);
    }

    /// Mark every zone touched by `bounds` dirty for the current generation.
    pub fn invalidate_bounds(&mut self, plane: StoragePlane, bounds: RenderTileBounds) {
        for key in bounds.zones(plane) {
            self.invalidate_zone(key);
        }
    }

    /// Advance semantic scene ownership and invalidate only changed zones.
    pub fn advance_generation(
        &mut self,
        next: SemanticGeneration,
        invalidated_zones: impl IntoIterator<Item = RenderZoneKey>,
    ) -> Result<(), RenderZoneError> {
        if !next.supersedes(self.current_generation) {
            return Err(RenderZoneError::GenerationNotNewer {
                current_serial: self.current_generation.serial(),
                next_serial: next.serial(),
            });
        }

        self.current_generation = next;
        for key in invalidated_zones {
            self.invalidate_for_generation(key, next);
        }
        Ok(())
    }

    pub fn is_dirty(&self, key: RenderZoneKey) -> bool {
        self.zones.get(&key).is_some_and(|state| state.dirty)
    }

    pub fn required_generation(&self, key: RenderZoneKey) -> Option<SemanticGeneration> {
        self.zones.get(&key).map(|state| state.required_generation)
    }

    /// Iterate dirty zones in stable key order.
    pub fn dirty_zones(&self) -> impl Iterator<Item = RenderZoneKey> + '_ {
        self.zones
            .iter()
            .filter_map(|(key, state)| state.dirty.then_some(*key))
    }

    /// Start a build for the generation that currently owns this dirty zone.
    pub fn begin_build(&self, key: RenderZoneKey) -> Option<ZoneBuildTicket> {
        let state = self.zones.get(&key)?;
        state.dirty.then_some(ZoneBuildTicket {
            key,
            required_generation: state.required_generation,
        })
    }

    /// Mark a zone clean only if the ticket still owns its required generation.
    pub fn complete_build(&mut self, ticket: ZoneBuildTicket) -> Result<(), RenderZoneError> {
        let state = self
            .zones
            .get_mut(&ticket.key)
            .ok_or(RenderZoneError::UnknownZone { key: ticket.key })?;

        if state.required_generation != ticket.required_generation {
            return Err(RenderZoneError::StaleBuildTicket {
                key: ticket.key,
                expected_serial: state.required_generation.serial(),
                ticket_serial: ticket.required_generation.serial(),
            });
        }
        if !state.dirty {
            return Err(RenderZoneError::ZoneAlreadyClean { key: ticket.key });
        }

        state.dirty = false;
        Ok(())
    }

    fn invalidate_for_generation(&mut self, key: RenderZoneKey, generation: SemanticGeneration) {
        self.zones.insert(
            key,
            ZoneState {
                required_generation: generation,
                dirty: true,
            },
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderZoneError {
    ZeroWidth,
    ZeroDepth,
    WidthTooLarge {
        width_tiles: u32,
    },
    DepthTooLarge {
        depth_tiles: u32,
    },
    XBoundsOverflow,
    ZBoundsOverflow,
    GenerationNotNewer {
        current_serial: u64,
        next_serial: u64,
    },
    UnknownZone {
        key: RenderZoneKey,
    },
    StaleBuildTicket {
        key: RenderZoneKey,
        expected_serial: u64,
        ticket_serial: u64,
    },
    ZoneAlreadyClean {
        key: RenderZoneKey,
    },
}

impl fmt::Display for RenderZoneError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroWidth => write!(formatter, "render tile bounds require nonzero width"),
            Self::ZeroDepth => write!(formatter, "render tile bounds require nonzero depth"),
            Self::WidthTooLarge { width_tiles } => {
                write!(
                    formatter,
                    "render tile width {width_tiles} exceeds signed tile range"
                )
            }
            Self::DepthTooLarge { depth_tiles } => {
                write!(
                    formatter,
                    "render tile depth {depth_tiles} exceeds signed tile range"
                )
            }
            Self::XBoundsOverflow => write!(formatter, "render tile bounds overflow on x axis"),
            Self::ZBoundsOverflow => write!(formatter, "render tile bounds overflow on z axis"),
            Self::GenerationNotNewer {
                current_serial,
                next_serial,
            } => write!(
                formatter,
                "semantic generation {next_serial} does not supersede current generation {current_serial}"
            ),
            Self::UnknownZone { key } => write!(
                formatter,
                "zone ({}, {}, {}) has no invalidation state",
                key.plane().index().get(),
                key.x(),
                key.z()
            ),
            Self::StaleBuildTicket {
                key,
                expected_serial,
                ticket_serial,
            } => write!(
                formatter,
                "zone ({}, {}, {}) requires generation {expected_serial}, but build ticket owns generation {ticket_serial}",
                key.plane().index().get(),
                key.x(),
                key.z()
            ),
            Self::ZoneAlreadyClean { key } => write!(
                formatter,
                "zone ({}, {}, {}) is already clean",
                key.plane().index().get(),
                key.x(),
                key.z()
            ),
        }
    }
}

impl Error for RenderZoneError {}
