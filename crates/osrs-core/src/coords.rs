//! Canonical semantic coordinate and plane value types.
//!
//! Map/world tiles, scene-storage tiles, 128-unit semantic coordinates, and
//! model-local vertices are intentionally distinct. Renderer camera/view/clip
//! coordinates do not belong in this module.

/// Number of semantic local units in one map tile (`COORD-001`).
pub const LOCAL_UNITS_PER_TILE: i32 = 128;
/// Exact semantic half-tile offset.
pub const HALF_TILE: i32 = 64;
/// Exact semantic quarter-tile offset.
pub const QUARTER_TILE: i32 = 32;
/// Exact semantic three-quarter-tile offset.
pub const THREE_QUARTER_TILE: i32 = 96;
/// Number of tiles along one region axis.
pub const REGION_SIZE_TILES: i32 = 64;
/// Highest ordinary OSRS semantic plane index.
pub const MAX_PLANE_INDEX: u8 = 3;

/// Global/world map tile coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MapTile {
    pub x: i32,
    pub y: i32,
}

impl MapTile {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// Split this world tile into its Euclidean region and 0..63 in-region tile.
    pub fn split_region(self) -> (RegionCoord, RegionTile) {
        let region = RegionCoord::new(
            self.x.div_euclid(REGION_SIZE_TILES),
            self.y.div_euclid(REGION_SIZE_TILES),
        );
        let tile = RegionTile {
            x: self.x.rem_euclid(REGION_SIZE_TILES) as u8,
            y: self.y.rem_euclid(REGION_SIZE_TILES) as u8,
        };
        (region, tile)
    }

    /// Reconstruct a world tile from a region and validated in-region tile.
    pub fn from_region(region: RegionCoord, tile: RegionTile) -> Option<Self> {
        let x = region
            .x
            .checked_mul(REGION_SIZE_TILES)?
            .checked_add(i32::from(tile.x))?;
        let y = region
            .y
            .checked_mul(REGION_SIZE_TILES)?
            .checked_add(i32::from(tile.y))?;
        Some(Self::new(x, y))
    }

    /// Convert this map tile to its semantic local-unit horizontal origin.
    ///
    /// Map `y` becomes semantic horizontal `z`; semantic `y` is vertical.
    pub fn local_origin(self) -> Option<LocalXZ> {
        Some(LocalXZ::new(
            LocalCoord::from_tiles(self.x)?,
            LocalCoord::from_tiles(self.y)?,
        ))
    }

    /// Convert to scene-storage coordinates relative to a supplied scene origin.
    pub fn to_scene(self, scene_origin: Self) -> Option<SceneTile> {
        let x = self.x.checked_sub(scene_origin.x)?;
        let y = self.y.checked_sub(scene_origin.y)?;
        Some(SceneTile::new(
            u32::try_from(x).ok()?,
            u32::try_from(y).ok()?,
        ))
    }
}

/// Global region coordinate. One region is 64x64 map tiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegionCoord {
    pub x: i32,
    pub y: i32,
}

impl RegionCoord {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// Validated tile coordinate inside a 64x64 region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegionTile {
    x: u8,
    y: u8,
}

impl RegionTile {
    pub const fn new(x: u8, y: u8) -> Option<Self> {
        if x < REGION_SIZE_TILES as u8 && y < REGION_SIZE_TILES as u8 {
            Some(Self { x, y })
        } else {
            None
        }
    }

    pub const fn x(self) -> u8 {
        self.x
    }

    pub const fn y(self) -> u8 {
        self.y
    }
}

/// Tile position inside a constructed semantic scene/storage grid.
///
/// No universal scene width/height is encoded in this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SceneTile {
    pub x: u32,
    pub y: u32,
}

impl SceneTile {
    pub const fn new(x: u32, y: u32) -> Self {
        Self { x, y }
    }

    pub fn to_map(self, scene_origin: MapTile) -> Option<MapTile> {
        let x = scene_origin.x.checked_add(i32::try_from(self.x).ok()?)?;
        let y = scene_origin.y.checked_add(i32::try_from(self.y).ok()?)?;
        Some(MapTile::new(x, y))
    }
}

/// One signed semantic coordinate measured in exact 128-unit tile space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocalCoord(i32);

impl LocalCoord {
    pub const fn from_units(units: i32) -> Self {
        Self(units)
    }

    pub fn from_tiles(tiles: i32) -> Option<Self> {
        tiles.checked_mul(LOCAL_UNITS_PER_TILE).map(Self)
    }

    pub const fn units(self) -> i32 {
        self.0
    }

    /// Tile containing this coordinate, using Euclidean floor semantics.
    pub const fn tile_floor(self) -> i32 {
        self.0.div_euclid(LOCAL_UNITS_PER_TILE)
    }

    /// Exact non-negative offset within the containing tile (`0..127`).
    pub const fn offset_in_tile(self) -> i32 {
        self.0.rem_euclid(LOCAL_UNITS_PER_TILE)
    }

    pub const fn checked_add(self, other: Self) -> Option<Self> {
        match self.0.checked_add(other.0) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
}

/// Horizontal semantic local-unit coordinate. `y` is intentionally absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocalXZ {
    pub x: LocalCoord,
    pub z: LocalCoord,
}

impl LocalXZ {
    pub const fn new(x: LocalCoord, z: LocalCoord) -> Self {
        Self { x, z }
    }
}

/// Three-dimensional semantic local position after scene placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocalPoint {
    pub x: LocalCoord,
    pub y: LocalCoord,
    pub z: LocalCoord,
}

impl LocalPoint {
    pub const fn new(x: LocalCoord, y: LocalCoord, z: LocalCoord) -> Self {
        Self { x, y, z }
    }
}

/// Asset-relative model vertex position before scene placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModelPoint {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl ModelPoint {
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    /// Place this model-local point at a semantic local origin without changing
    /// the model-local source value.
    pub fn place(self, origin: LocalXZ, base_y: LocalCoord) -> Option<LocalPoint> {
        Some(LocalPoint::new(
            origin.x.checked_add(LocalCoord::from_units(self.x))?,
            base_y.checked_add(LocalCoord::from_units(self.y))?,
            origin.z.checked_add(LocalCoord::from_units(self.z))?,
        ))
    }
}

/// Validated ordinary OSRS plane index (`0..=3`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlaneIndex(u8);

impl PlaneIndex {
    pub const fn new(value: u8) -> Option<Self> {
        if value <= MAX_PLANE_INDEX {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

macro_rules! typed_plane {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(PlaneIndex);

        impl $name {
            pub const fn new(value: u8) -> Option<Self> {
                match PlaneIndex::new(value) {
                    Some(plane) => Some(Self(plane)),
                    None => None,
                }
            }

            pub const fn index(self) -> PlaneIndex {
                self.0
            }
        }
    };
}

typed_plane!(SourcePlane, "Encoded/source plane from map/location data.");
typed_plane!(
    StoragePlane,
    "Plane on which a tile is stored after semantic relinking."
);
typed_plane!(
    RenderLevel,
    "Semantic render/height level, distinct from storage and collision planes."
);

/// Collision plane can be absent when a semantic adjustment would move below plane zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CollisionPlane {
    None,
    Plane(PlaneIndex),
}

impl CollisionPlane {
    pub const fn from_index(value: u8) -> Option<Self> {
        match PlaneIndex::new(value) {
            Some(plane) => Some(Self::Plane(plane)),
            None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_constants_are_exact() {
        assert_eq!(LOCAL_UNITS_PER_TILE, 128);
        assert_eq!(HALF_TILE, 64);
        assert_eq!(QUARTER_TILE, 32);
        assert_eq!(THREE_QUARTER_TILE, 96);
    }

    #[test]
    fn local_coordinates_round_trip_positive_and_negative_tiles() {
        for tile in [-10, -1, 0, 1, 10] {
            let local = LocalCoord::from_tiles(tile);
            assert_eq!(local.map(LocalCoord::tile_floor), Some(tile));
            assert_eq!(local.map(LocalCoord::offset_in_tile), Some(0));
        }

        let negative_one = LocalCoord::from_units(-1);
        assert_eq!(negative_one.tile_floor(), -1);
        assert_eq!(negative_one.offset_in_tile(), 127);
    }

    #[test]
    fn region_split_round_trips_across_zero_and_borders() {
        for tile in [
            MapTile::new(-65, -64),
            MapTile::new(-1, -1),
            MapTile::new(0, 0),
            MapTile::new(63, 63),
            MapTile::new(64, 64),
            MapTile::new(12_345, 6_789),
        ] {
            let (region, within) = tile.split_region();
            assert_eq!(MapTile::from_region(region, within), Some(tile));
            assert!(within.x() < 64);
            assert!(within.y() < 64);
        }
    }

    #[test]
    fn map_and_scene_coordinates_round_trip_without_mutating_world_identity() {
        let origin = MapTile::new(3_200, 3_200);
        let world = MapTile::new(3_263, 3_264);
        let scene = world.to_scene(origin);
        assert_eq!(scene, Some(SceneTile::new(63, 64)));
        assert_eq!(scene.and_then(|value| value.to_map(origin)), Some(world));
    }

    #[test]
    fn map_tile_origin_uses_exact_128_unit_horizontal_mapping() {
        let origin = MapTile::new(50, 51).local_origin();
        assert_eq!(
            origin,
            Some(LocalXZ::new(
                LocalCoord::from_units(6_400),
                LocalCoord::from_units(6_528),
            ))
        );
    }

    #[test]
    fn model_local_and_scene_local_points_remain_distinct_and_place_exactly() {
        let model = ModelPoint::new(-32, 64, 96);
        let placed = model.place(
            LocalXZ::new(LocalCoord::from_units(1_280), LocalCoord::from_units(2_560)),
            LocalCoord::from_units(-100),
        );
        assert_eq!(
            placed,
            Some(LocalPoint::new(
                LocalCoord::from_units(1_248),
                LocalCoord::from_units(-36),
                LocalCoord::from_units(2_656),
            ))
        );
        assert_eq!(model, ModelPoint::new(-32, 64, 96));
    }

    #[test]
    fn plane_types_reject_out_of_range_values_and_remain_distinct() {
        assert_eq!(PlaneIndex::new(3).map(PlaneIndex::get), Some(3));
        assert_eq!(PlaneIndex::new(4), None);
        assert_eq!(
            SourcePlane::new(2).map(|value| value.index().get()),
            Some(2)
        );
        assert_eq!(
            StoragePlane::new(2).map(|value| value.index().get()),
            Some(2)
        );
        assert_eq!(
            RenderLevel::new(2).map(|value| value.index().get()),
            Some(2)
        );
        assert_eq!(CollisionPlane::from_index(4), None);
        assert_eq!(CollisionPlane::None, CollisionPlane::None);
    }
}
