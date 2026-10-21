//! Finishing a generated building the way real ones are finished, from what was measured on real
//! towns: the floor under each kind of room, windows as decoration on both faces of a wall,
//! torches and lanterns on interior walls, furniture standing against walls facing the way real
//! furniture does, and a site graded flat with a blended edge so the building sits on the map.
//!
//! The data lives in a kit (`KitFinish`); this stage reads the kit, the plan and the facade edges
//! and adds to the generator's patch. Everything is deterministic in the building seed.

mod furnish;
mod grade;

use crate::{
    BspBuildingPlan, BuildingRoomRole, CardinalDirection, EdgeFeature, FloorMaterial,
    GenerationSnapshot, LocKind, MapOp, MapPatch, MapPosition, ObjectOrientation, ObjectPlacement,
    OverlayRotation, OverlayShape, RoomKind, SemanticEdge, TerrainOverlay,
};
use osrs_core::{
    coords::MapTile,
    ids::{FloorOverlayId, FloorUnderlayId, ObjectId},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KitDecor {
    pub id: u32,
    /// Wall decoration kind (4 for decoration against the wall, 5 for one standing off it).
    pub kind: u8,
    pub weight: u32,
}

/// Windows as real towns build them: the wall stays, and a decoration hangs on each face.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KitWindowDecor {
    pub outside: Option<KitDecor>,
    pub inside: Option<KitDecor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KitPlace {
    /// On a tile touching a wall, facing as `relative` says compared with that wall.
    AgainstWall,
    /// Away from the walls, with this absolute orientation.
    Center,
    /// A run of pieces side by side along one wall (bank booths, shelves), all facing the room.
    Row,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KitFurniture {
    pub id: u32,
    pub kind: u8,
    pub place: KitPlace,
    /// Orientation minus the side of the wall it stands against (west 0, north 1, east 2,
    /// south 3), as measured on real rooms; for `center` the orientation itself.
    pub relative: u8,
    pub min: u8,
    pub max: u8,
    /// The object's base size in tiles; a bed is 1 x 2. Turned a quarter, the two swap.
    #[serde(default = "one_by_one")]
    pub size: [u8; 2],
}

fn one_by_one() -> [u8; 2] {
    [1, 1]
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KitFloorFile {
    pub underlay: Option<u32>,
    pub overlay: Option<(u32, u8, u8)>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KitFinish {
    #[serde(default)]
    pub learned_roles: Option<crate::LearnedRoleModel>,
    /// Floor by room role (`living`, `bank_lobby`... see `role_name`) and `hallway`.
    #[serde(default)]
    pub floors: BTreeMap<String, KitFloorFile>,
    #[serde(default)]
    pub window_decor: Option<KitWindowDecor>,
    /// Hung on interior walls, with the chance (percent) that an eligible wall edge gets one.
    #[serde(default)]
    pub wall_decor: Vec<KitDecor>,
    #[serde(default)]
    pub wall_decor_percent: u8,
    /// Furniture by room role.
    #[serde(default)]
    pub furniture: BTreeMap<String, Vec<KitFurniture>>,
    /// Flatten the site under the building and blend the margin into the ground.
    #[serde(default)]
    pub grade: bool,
}

/// Terrain setting bit for a ground tile with a roof above it (`osrs_scene::roofs::UNDER_ROOF`).
const UNDER_ROOF: u8 = 4;

/// A room role as the kit files spell it.
pub fn role_name(role: BuildingRoomRole) -> &'static str {
    role.label()
}

/// The object orientation for a piece on the `direction` side of a tile.
pub(crate) const fn orientation_of(direction: CardinalDirection) -> u8 {
    match direction {
        CardinalDirection::West => 0,
        CardinalDirection::North => 1,
        CardinalDirection::East => 2,
        CardinalDirection::South => 3,
    }
}

pub(crate) fn opposite(direction: CardinalDirection) -> CardinalDirection {
    match direction {
        CardinalDirection::North => CardinalDirection::South,
        CardinalDirection::East => CardinalDirection::West,
        CardinalDirection::South => CardinalDirection::North,
        CardinalDirection::West => CardinalDirection::East,
    }
}

/// A small deterministic generator (SplitMix64).
pub(crate) struct Rng(pub u64);

impl Rng {
    pub(crate) fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A number in `0..bound` (`bound` of zero gives zero).
    pub(crate) fn below(&mut self, bound: usize) -> usize {
        if bound == 0 {
            0
        } else {
            (self.next() % bound as u64) as usize
        }
    }
}

fn placement(
    snapshot: &GenerationSnapshot,
    object: u32,
    kind: u8,
    tile: MapTile,
    orientation: u8,
) -> Option<ObjectPlacement> {
    Some(ObjectPlacement {
        object_id: ObjectId::new(object),
        position: MapPosition::new(tile, snapshot.request().base_plane),
        kind: LocKind::new(kind)?,
        orientation: ObjectOrientation::new(orientation & 3)?,
    })
}

fn pick<'a>(decor: &'a [KitDecor], rng: &mut Rng) -> Option<&'a KitDecor> {
    let total: u64 = decor.iter().map(|d| u64::from(d.weight.max(1))).sum();
    if total == 0 {
        return None;
    }
    let mut roll = rng.next() % total;
    decor.iter().find(|d| {
        let weight = u64::from(d.weight.max(1));
        if roll < weight {
            true
        } else {
            roll -= weight;
            false
        }
    })
}

impl KitFinish {
    fn floor_for(&self, name: &str) -> Option<FloorMaterial> {
        let file = self.floors.get(name)?;
        let overlay = match file.overlay {
            None => None,
            Some((id, shape, rotation)) => Some(TerrainOverlay {
                id: FloorOverlayId::new(id),
                shape: OverlayShape::new(shape)?,
                rotation: OverlayRotation::new(rotation)?,
            }),
        };
        Some(FloorMaterial {
            underlay: file.underlay.map(FloorUnderlayId::new),
            overlay,
        })
    }

    /// Add the finishing to `body`: floors, window and wall decoration, furniture and grading.
    pub fn apply(
        &self,
        body: &MapPatch,
        building: &BspBuildingPlan,
        edges: &[SemanticEdge],
        snapshot: &GenerationSnapshot,
    ) -> MapPatch {
        self.apply_to_floor(body, building, edges, snapshot, true)
    }

    /// [`Self::apply`] for one floor of a building. Only the ground floor grades the site: the
    /// ground under an upper floor is the floor below, not terrain.
    pub fn apply_to_floor(
        &self,
        body: &MapPatch,
        building: &BspBuildingPlan,
        edges: &[SemanticEdge],
        snapshot: &GenerationSnapshot,
        ground: bool,
    ) -> MapPatch {
        let plan = &building.plan;
        let mut rng = Rng(plan.request.seed ^ 0x5bd1_e995_a3c2_f1d7);
        let role_name_of = |tile: MapTile| -> Option<&'static str> {
            let room = plan.rooms.iter().find(|room| room.rect.contains(tile))?;
            if room.kind == RoomKind::Hallway {
                Some("hallway")
            } else {
                building.role_for(room.id).map(role_name)
            }
        };
        let mut ops: Vec<MapOp> = Vec::with_capacity(body.ops().len() + 64);
        // The floor of every room tile, by room role. The body's own floor operations are dropped
        // and rebuilt here from the snapshot, so a kit with no floor of its own still gets one.
        let mut floor_tiles: HashSet<MapTile> = HashSet::new();
        for room in &plan.rooms {
            for y in 0..room.rect.height {
                for x in 0..room.rect.width {
                    floor_tiles.insert(MapTile::new(
                        room.rect.origin.x + i32::from(x),
                        room.rect.origin.y + i32::from(y),
                    ));
                }
            }
        }
        let mut body_floor: HashMap<MapTile, crate::TerrainTile> = HashMap::new();
        for op in body.ops() {
            match *op {
                MapOp::SetTerrain { coord, after, .. } => {
                    body_floor.insert(coord.tile, after);
                }
                other => ops.push(other),
            }
        }
        let mut tiles: Vec<MapTile> = floor_tiles.into_iter().collect();
        tiles.sort();
        for tile in tiles {
            let Some(before) = snapshot.terrain_at(tile) else {
                continue;
            };
            let mut after = body_floor.get(&tile).copied().unwrap_or(before);
            // The tile has a roof above it, like every real roofed building: the game and roof
            // removal read this setting to know which roof goes with which floor.
            after.flags |= UNDER_ROOF;
            if let Some(floor) = role_name_of(tile)
                .and_then(|name| self.floor_for(name))
                .or_else(|| self.floor_for("default"))
            {
                after.underlay = floor.underlay;
                after.overlay = floor.overlay;
            }
            if before != after {
                ops.push(MapOp::SetTerrain {
                    coord: MapPosition::new(tile, snapshot.request().base_plane),
                    before,
                    after,
                });
            }
        }
        let mut placed: HashSet<ObjectPlacement> = body
            .ops()
            .iter()
            .filter_map(|op| match op {
                MapOp::AddObject { placement } => Some(*placement),
                _ => None,
            })
            .collect();
        // The client has one wall-decoration slot per tile, shared by all wall faces.
        // Optional finishing must not emit two distinct decorations that overwrite each other.
        let mut decor_slots: HashSet<MapPosition> = placed
            .iter()
            .filter(|p| (4..=8).contains(&p.kind.get()))
            .map(|p| p.position)
            .collect();
        let mut add = |ops: &mut Vec<MapOp>, placement: Option<ObjectPlacement>| {
            if let Some(placement) = placement {
                if (4..=8).contains(&placement.kind.get())
                    && !decor_slots.insert(placement.position)
                {
                    return;
                }
                if placed.insert(placement) {
                    ops.push(MapOp::AddObject { placement });
                }
            }
        };

        // Which wall edges carry something already, from either side.
        let mut feature: HashMap<(MapTile, CardinalDirection), EdgeFeature> = HashMap::new();
        for edge in edges {
            feature.insert((edge.tile, edge.direction), edge.feature);
            if let Some(next) = edge.direction.neighbor(edge.tile) {
                feature.insert((next, opposite(edge.direction)), edge.feature);
            }
        }
        let mut hung: HashSet<(MapTile, CardinalDirection)> = HashSet::new();
        if let Some(windows) = &self.window_decor {
            for edge in edges.iter().filter(|e| e.feature == EdgeFeature::Window) {
                if let Some(inside) = &windows.inside {
                    add(
                        &mut ops,
                        placement(
                            snapshot,
                            inside.id,
                            inside.kind,
                            edge.tile,
                            orientation_of(edge.direction),
                        ),
                    );
                    hung.insert((edge.tile, edge.direction));
                }
                if let (Some(outside), Some(next)) =
                    (&windows.outside, edge.direction.neighbor(edge.tile))
                {
                    add(
                        &mut ops,
                        placement(
                            snapshot,
                            outside.id,
                            outside.kind,
                            next,
                            orientation_of(opposite(edge.direction)),
                        ),
                    );
                }
            }
        }
        if !self.wall_decor.is_empty() && self.wall_decor_percent > 0 {
            for edge in edges.iter().filter(|e| {
                matches!(
                    e.feature,
                    EdgeFeature::ExteriorWall | EdgeFeature::InteriorWall
                )
            }) {
                if hung.contains(&(edge.tile, edge.direction))
                    || rng.below(100) >= usize::from(self.wall_decor_percent)
                {
                    continue;
                }
                // Not next to a door or window, so the wall does not look crowded.
                let crowded = [(-1, 0), (1, 0), (0, -1), (0, 1)].iter().any(|(dx, dy)| {
                    let near = MapTile::new(edge.tile.x + dx, edge.tile.y + dy);
                    matches!(
                        feature.get(&(near, edge.direction)),
                        Some(EdgeFeature::Door | EdgeFeature::Window)
                    )
                }) || matches!(
                    feature.get(&(edge.tile, edge.direction)),
                    Some(EdgeFeature::Door | EdgeFeature::Window)
                );
                if crowded {
                    continue;
                }
                if let Some(decor) = pick(&self.wall_decor, &mut rng) {
                    add(
                        &mut ops,
                        placement(
                            snapshot,
                            decor.id,
                            decor.kind,
                            edge.tile,
                            orientation_of(edge.direction),
                        ),
                    );
                    hung.insert((edge.tile, edge.direction));
                }
            }
        }
        for piece in furnish::furnish(self, building, &feature, snapshot, &mut rng) {
            add(&mut ops, Some(piece));
        }
        if self.grade && ground {
            ops.extend(grade::grade(building, snapshot));
        }
        MapPatch::new(ops)
    }
}

#[cfg(test)]
mod tests;

impl KitFinish {
    /// Check the section: known role names, valid kinds, floors that make sense.
    pub fn validate(&self) -> Result<(), crate::KitError> {
        if let Some(model) = &self.learned_roles {
            model
                .validate()
                .map_err(|e| crate::building_kit::KitError::Roof(e.to_string()))?;
        }

        let bad = |what: &'static str, value: u32| crate::KitError::Value { what, value };
        for name in self.floors.keys().chain(self.furniture.keys()) {
            if !matches!(name.as_str(), "hallway" | "default")
                && BuildingRoomRole::from_label(name).is_none_or(|role| role.label() != name)
            {
                return Err(crate::KitError::Roof(format!("unknown room role {name:?}")));
            }
        }
        for name in self.floors.keys() {
            if self.floor_for(name).is_none() {
                return Err(bad("floor overlay shape or rotation", 0));
            }
        }
        let decor = self.wall_decor.iter().chain(
            self.window_decor
                .iter()
                .flat_map(|w| w.outside.iter().chain(w.inside.iter())),
        );
        for d in decor {
            if LocKind::new(d.kind).is_none() {
                return Err(bad("decoration kind", u32::from(d.kind)));
            }
        }
        for rule in self.furniture.values().flatten() {
            if LocKind::new(rule.kind).is_none() {
                return Err(bad("furniture kind", u32::from(rule.kind)));
            }
            if rule.min > rule.max {
                return Err(bad(
                    "furniture minimum above its maximum",
                    u32::from(rule.min),
                ));
            }
        }
        Ok(())
    }
}
