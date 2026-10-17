//! Headless semantic building-generation contracts and the first deterministic layout planner.
//!
//! This module intentionally stops before object-id realization. It plans architecture in semantic
//! map-tile space so later WFC, palette, validation, and realization stages can evolve without
//! coupling procedural generation to renderer, cache, worker, or UI representations.

use osrs_core::coords::{MapTile, SourcePlane};

const EXTERIOR_WALL_INSET: u16 = 1;
const DEFAULT_MIN_ROOM_SPAN: u16 = 2;
const DEFAULT_CORRIDOR_WIDTH: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildingArchetype {
    House,
    Shop,
    Tavern,
    Bank,
    Church,
    Warehouse,
    Castle,
    Dungeon,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildingStyle {
    Generic,
    Lumbridge,
    Varrock,
    Falador,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RectSite {
    origin: MapTile,
    width: u16,
    height: u16,
}

impl RectSite {
    pub fn new(origin: MapTile, width: u16, height: u16) -> Option<Self> {
        if width == 0 || height == 0 {
            return None;
        }
        origin.x.checked_add(i32::from(width))?;
        origin.y.checked_add(i32::from(height))?;
        Some(Self {
            origin,
            width,
            height,
        })
    }

    pub const fn origin(self) -> MapTile {
        self.origin
    }

    pub const fn width(self) -> u16 {
        self.width
    }

    pub const fn height(self) -> u16 {
        self.height
    }

    pub fn contains(self, tile: MapTile) -> bool {
        let max_x = self.origin.x + i32::from(self.width);
        let max_y = self.origin.y + i32::from(self.height);
        tile.x >= self.origin.x && tile.x < max_x && tile.y >= self.origin.y && tile.y < max_y
    }

    fn local_tile(self, x: u16, y: u16) -> MapTile {
        MapTile::new(self.origin.x + i32::from(x), self.origin.y + i32::from(y))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BuildingConstraints {
    pub min_room_span: u16,
    pub corridor_width: u16,
}

impl Default for BuildingConstraints {
    fn default() -> Self {
        Self {
            min_room_span: DEFAULT_MIN_ROOM_SPAN,
            corridor_width: DEFAULT_CORRIDOR_WIDTH,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BuildingRequest {
    pub seed: u64,
    pub site: RectSite,
    pub base_plane: SourcePlane,
    pub floors: u8,
    pub archetype: BuildingArchetype,
    pub style: BuildingStyle,
    pub constraints: BuildingConstraints,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RoomId(u16);

impl RoomId {
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RoomKind {
    Hallway,
    Living,
    Kitchen,
    Bedroom,
    Utility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RoomRect {
    pub origin: MapTile,
    pub width: u16,
    pub height: u16,
}

impl RoomRect {
    pub fn contains(self, tile: MapTile) -> bool {
        let max_x = self.origin.x + i32::from(self.width);
        let max_y = self.origin.y + i32::from(self.height);
        tile.x >= self.origin.x && tile.x < max_x && tile.y >= self.origin.y && tile.y < max_y
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RoomSpec {
    pub id: RoomId,
    pub kind: RoomKind,
    pub rect: RoomRect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RoomConnection {
    /// `None` represents the exterior entrance.
    pub from: Option<RoomId>,
    pub to: RoomId,
    pub doorway: MapTile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildingPlan {
    pub request: BuildingRequest,
    pub rooms: Vec<RoomSpec>,
    pub connections: Vec<RoomConnection>,
    pub entrance: MapTile,
}

impl BuildingPlan {
    pub fn room(&self, id: RoomId) -> Option<&RoomSpec> {
        self.rooms.iter().find(|room| room.id == id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildingPlanError {
    UnsupportedArchetype {
        archetype: BuildingArchetype,
    },
    UnsupportedFloorCount {
        floors: u8,
    },
    CorridorWidthMustBeOne {
        width: u16,
    },
    SiteTooSmall {
        width: u16,
        height: u16,
        required_width: u16,
        required_height: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum OccupancyCell {
    #[default]
    Free,
    Reserved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OccupancyGrid {
    site: RectSite,
    cells: Vec<OccupancyCell>,
}

impl OccupancyGrid {
    pub fn new(site: RectSite) -> Self {
        let cell_count = usize::from(site.width) * usize::from(site.height);
        Self {
            site,
            cells: vec![OccupancyCell::Free; cell_count],
        }
    }

    pub const fn site(&self) -> RectSite {
        self.site
    }

    pub fn get(&self, tile: MapTile) -> Option<OccupancyCell> {
        self.index(tile).map(|index| self.cells[index])
    }

    pub fn set(&mut self, tile: MapTile, value: OccupancyCell) -> bool {
        let Some(index) = self.index(tile) else {
            return false;
        };
        self.cells[index] = value;
        true
    }

    fn index(&self, tile: MapTile) -> Option<usize> {
        if !self.site.contains(tile) {
            return None;
        }
        let local_x = usize::try_from(tile.x - self.site.origin.x).ok()?;
        let local_y = usize::try_from(tile.y - self.site.origin.y).ok()?;
        Some(local_y * usize::from(self.site.width) + local_x)
    }
}

/// Plan the first end-to-end building vertical slice: one deterministic, connected house.
///
/// The plan reserves a one-tile exterior wall ring, places a one-tile hallway through the longer
/// site axis, creates four rooms around it, and connects every room to that hallway. Later WFC
/// checkpoints will replace local architectural detail without weakening these global guarantees.
pub fn plan_basic_house(request: BuildingRequest) -> Result<BuildingPlan, BuildingPlanError> {
    if request.archetype != BuildingArchetype::House {
        return Err(BuildingPlanError::UnsupportedArchetype {
            archetype: request.archetype,
        });
    }
    if request.floors != 1 {
        return Err(BuildingPlanError::UnsupportedFloorCount {
            floors: request.floors,
        });
    }
    if request.constraints.corridor_width != 1 {
        return Err(BuildingPlanError::CorridorWidthMustBeOne {
            width: request.constraints.corridor_width,
        });
    }

    let min_room = request.constraints.min_room_span.max(1);
    let required_split_axis = EXTERIOR_WALL_INSET * 2 + min_room * 2;
    let required_cross_axis = EXTERIOR_WALL_INSET * 2 + min_room * 2 + 1;
    let required_width = required_split_axis.max(6);
    let required_height = required_cross_axis.max(7);
    if request.site.width < required_width || request.site.height < required_height {
        return Err(BuildingPlanError::SiteTooSmall {
            width: request.site.width,
            height: request.site.height,
            required_width,
            required_height,
        });
    }

    let mut rng = SplitMix64::new(request.seed);
    if request.site.width >= request.site.height {
        plan_horizontal_house(request, min_room, &mut rng)
    } else {
        plan_vertical_house(request, min_room, &mut rng)
    }
}

fn plan_horizontal_house(
    request: BuildingRequest,
    min_room: u16,
    rng: &mut SplitMix64,
) -> Result<BuildingPlan, BuildingPlanError> {
    let site = request.site;
    let interior_width = site.width - 2;
    let interior_height = site.height - 2;
    let hall_local_y = 1 + interior_height / 2;
    let lower_height = hall_local_y - 1;
    let upper_start = hall_local_y + 1;
    let upper_height = site.height - 1 - upper_start;
    if lower_height < min_room || upper_height < min_room || interior_width < min_room * 2 {
        return Err(BuildingPlanError::SiteTooSmall {
            width: site.width,
            height: site.height,
            required_width: min_room * 2 + 2,
            required_height: min_room * 2 + 3,
        });
    }

    let split = choose_split(interior_width, min_room, rng);
    let hall = room(
        0,
        RoomKind::Hallway,
        site,
        1,
        hall_local_y,
        interior_width,
        1,
    );
    let lower_left = room(1, RoomKind::Living, site, 1, 1, split, lower_height);
    let lower_right = room(
        2,
        RoomKind::Kitchen,
        site,
        1 + split,
        1,
        interior_width - split,
        lower_height,
    );
    let upper_left = room(
        3,
        RoomKind::Bedroom,
        site,
        1,
        upper_start,
        split,
        upper_height,
    );
    let upper_right = room(
        4,
        RoomKind::Utility,
        site,
        1 + split,
        upper_start,
        interior_width - split,
        upper_height,
    );

    let (entrance, exterior_doorway) = if rng.next_u64() & 1 == 0 {
        (
            site.local_tile(0, hall_local_y),
            site.local_tile(1, hall_local_y),
        )
    } else {
        (
            site.local_tile(site.width - 1, hall_local_y),
            site.local_tile(site.width - 2, hall_local_y),
        )
    };
    let hall_id = hall.id;
    let connections = vec![
        RoomConnection {
            from: None,
            to: hall_id,
            doorway: exterior_doorway,
        },
        connect(
            hall_id,
            lower_left.id,
            site.local_tile(1 + split / 2, hall_local_y),
        ),
        connect(
            hall_id,
            lower_right.id,
            site.local_tile(1 + split + (interior_width - split) / 2, hall_local_y),
        ),
        connect(
            hall_id,
            upper_left.id,
            site.local_tile(1 + split / 2, hall_local_y),
        ),
        connect(
            hall_id,
            upper_right.id,
            site.local_tile(1 + split + (interior_width - split) / 2, hall_local_y),
        ),
    ];

    Ok(BuildingPlan {
        request,
        rooms: vec![hall, lower_left, lower_right, upper_left, upper_right],
        connections,
        entrance,
    })
}

fn plan_vertical_house(
    request: BuildingRequest,
    min_room: u16,
    rng: &mut SplitMix64,
) -> Result<BuildingPlan, BuildingPlanError> {
    let site = request.site;
    let interior_width = site.width - 2;
    let interior_height = site.height - 2;
    let hall_local_x = 1 + interior_width / 2;
    let left_width = hall_local_x - 1;
    let right_start = hall_local_x + 1;
    let right_width = site.width - 1 - right_start;
    if left_width < min_room || right_width < min_room || interior_height < min_room * 2 {
        return Err(BuildingPlanError::SiteTooSmall {
            width: site.width,
            height: site.height,
            required_width: min_room * 2 + 3,
            required_height: min_room * 2 + 2,
        });
    }

    let split = choose_split(interior_height, min_room, rng);
    let hall = room(
        0,
        RoomKind::Hallway,
        site,
        hall_local_x,
        1,
        1,
        interior_height,
    );
    let lower_left = room(1, RoomKind::Living, site, 1, 1, left_width, split);
    let upper_left = room(
        2,
        RoomKind::Kitchen,
        site,
        1,
        1 + split,
        left_width,
        interior_height - split,
    );
    let lower_right = room(
        3,
        RoomKind::Bedroom,
        site,
        right_start,
        1,
        right_width,
        split,
    );
    let upper_right = room(
        4,
        RoomKind::Utility,
        site,
        right_start,
        1 + split,
        right_width,
        interior_height - split,
    );

    let (entrance, exterior_doorway) = if rng.next_u64() & 1 == 0 {
        (
            site.local_tile(hall_local_x, 0),
            site.local_tile(hall_local_x, 1),
        )
    } else {
        (
            site.local_tile(hall_local_x, site.height - 1),
            site.local_tile(hall_local_x, site.height - 2),
        )
    };
    let hall_id = hall.id;
    let connections = vec![
        RoomConnection {
            from: None,
            to: hall_id,
            doorway: exterior_doorway,
        },
        connect(
            hall_id,
            lower_left.id,
            site.local_tile(hall_local_x, 1 + split / 2),
        ),
        connect(
            hall_id,
            upper_left.id,
            site.local_tile(hall_local_x, 1 + split + (interior_height - split) / 2),
        ),
        connect(
            hall_id,
            lower_right.id,
            site.local_tile(hall_local_x, 1 + split / 2),
        ),
        connect(
            hall_id,
            upper_right.id,
            site.local_tile(hall_local_x, 1 + split + (interior_height - split) / 2),
        ),
    ];

    Ok(BuildingPlan {
        request,
        rooms: vec![hall, lower_left, upper_left, lower_right, upper_right],
        connections,
        entrance,
    })
}

fn room(
    id: u16,
    kind: RoomKind,
    site: RectSite,
    local_x: u16,
    local_y: u16,
    width: u16,
    height: u16,
) -> RoomSpec {
    RoomSpec {
        id: RoomId::new(id),
        kind,
        rect: RoomRect {
            origin: site.local_tile(local_x, local_y),
            width,
            height,
        },
    }
}

const fn connect(from: RoomId, to: RoomId, doorway: MapTile) -> RoomConnection {
    RoomConnection {
        from: Some(from),
        to,
        doorway,
    }
}

fn choose_split(span: u16, min_room: u16, rng: &mut SplitMix64) -> u16 {
    let min = min_room;
    let max = span - min_room;
    rng.range_inclusive(min, max)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    fn range_inclusive(&mut self, min: u16, max: u16) -> u16 {
        if min >= max {
            return min;
        }
        let span = u64::from(max - min) + 1;
        min + u16::try_from(self.next_u64() % span).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plane() -> SourcePlane {
        SourcePlane::new(0).unwrap_or_else(|| unreachable!("plane zero is valid"))
    }

    fn request(seed: u64, width: u16, height: u16) -> BuildingRequest {
        BuildingRequest {
            seed,
            site: RectSite::new(MapTile::new(3200, 3200), width, height)
                .unwrap_or_else(|| unreachable!("test site is valid")),
            base_plane: plane(),
            floors: 1,
            archetype: BuildingArchetype::House,
            style: BuildingStyle::Generic,
            constraints: BuildingConstraints::default(),
        }
    }

    #[test]
    fn same_request_produces_identical_plan() {
        let request = request(42, 12, 10);
        let first = plan_basic_house(request);
        let second = plan_basic_house(request);
        assert_eq!(first, second);
    }

    #[test]
    fn rooms_stay_inside_site_and_have_positive_area() {
        let plan = plan_basic_house(request(7, 12, 10))
            .unwrap_or_else(|error| unreachable!("valid test plan: {error:?}"));
        for room in &plan.rooms {
            assert!(room.rect.width > 0);
            assert!(room.rect.height > 0);
            assert!(plan.request.site.contains(room.rect.origin));
            let far = MapTile::new(
                room.rect.origin.x + i32::from(room.rect.width - 1),
                room.rect.origin.y + i32::from(room.rect.height - 1),
            );
            assert!(plan.request.site.contains(far));
        }
    }

    #[test]
    fn every_non_hall_room_connects_to_hallway() {
        let plan = plan_basic_house(request(99, 12, 10))
            .unwrap_or_else(|error| unreachable!("valid test plan: {error:?}"));
        let hall = plan
            .rooms
            .iter()
            .find(|room| room.kind == RoomKind::Hallway)
            .unwrap_or_else(|| unreachable!("house always has a hallway"));

        for room in plan.rooms.iter().filter(|room| room.id != hall.id) {
            assert!(plan.connections.iter().any(|connection| {
                connection.from == Some(hall.id) && connection.to == room.id
            }));
        }
        assert!(
            plan.connections
                .iter()
                .any(|connection| { connection.from.is_none() && connection.to == hall.id })
        );
    }

    #[test]
    fn exterior_entrance_is_cardinally_adjacent_to_hall_doorway() {
        for (width, height) in [(12, 10), (10, 12)] {
            let plan = plan_basic_house(request(123, width, height))
                .unwrap_or_else(|error| unreachable!("valid test plan: {error:?}"));
            let exterior = plan
                .connections
                .iter()
                .find(|connection| connection.from.is_none())
                .unwrap_or_else(|| unreachable!("house has one exterior connection"));
            let dx = (plan.entrance.x - exterior.doorway.x).abs();
            let dy = (plan.entrance.y - exterior.doorway.y).abs();
            assert_eq!(dx + dy, 1);
        }
    }

    #[test]
    fn occupancy_grid_rejects_tiles_outside_site() {
        let site = request(1, 12, 10).site;
        let mut occupancy = OccupancyGrid::new(site);
        let inside = site.origin();
        assert!(occupancy.set(inside, OccupancyCell::Blocked));
        assert_eq!(occupancy.get(inside), Some(OccupancyCell::Blocked));

        let outside = MapTile::new(inside.x - 1, inside.y);
        assert!(!occupancy.set(outside, OccupancyCell::Reserved));
        assert_eq!(occupancy.get(outside), None);
    }

    #[test]
    fn unsupported_floor_count_is_rejected_before_layout() {
        let mut request = request(1, 12, 10);
        request.floors = 2;
        assert_eq!(
            plan_basic_house(request),
            Err(BuildingPlanError::UnsupportedFloorCount { floors: 2 })
        );
    }
}
