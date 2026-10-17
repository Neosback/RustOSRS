//! Semantic corner and portal-detail refinement after global topology and facade WFC.
//!
//! This layer still does not let procedural solvers choose RuneScape object IDs. It collapses
//! eligible pairs of exterior shell edges into semantic corner elements and chooses a semantic
//! door-variant slot from caller-supplied weights. Asset IDs are resolved only during realization.

use crate::{
    BuildingArchitectureError, BuildingPalette, BuildingPlan, BuildingValidationReport,
    CardinalDirection, EdgeAsset, EdgeFeature, FacadeRefinementError, FacadeRefinementPolicy,
    FacadeWfcConfig, GenerationSnapshot, LocKind, MapOp, MapPatch, MapPosition, ObjectOrientation,
    ObjectPlacement, RefinedBuildingArchitecture, SemanticEdge, TerrainTile,
    derive_building_architecture, facade_variation_seed, refine_exterior_walls_for_variation,
    validate_building_plan,
};
use osrs_core::coords::{MapTile, SourcePlane};
use osrs_core::ids::ObjectId;
use std::collections::{BTreeMap, BTreeSet, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CornerTurn {
    NorthEast,
    EastSouth,
    SouthWest,
    WestNorth,
}

impl CornerTurn {
    pub const fn index(self) -> usize {
        match self {
            Self::NorthEast => 0,
            Self::EastSouth => 1,
            Self::SouthWest => 2,
            Self::WestNorth => 3,
        }
    }

    pub const fn directions(self) -> [CardinalDirection; 2] {
        match self {
            Self::NorthEast => [CardinalDirection::North, CardinalDirection::East],
            Self::EastSouth => [CardinalDirection::East, CardinalDirection::South],
            Self::SouthWest => [CardinalDirection::South, CardinalDirection::West],
            Self::WestNorth => [CardinalDirection::West, CardinalDirection::North],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CornerAsset {
    pub object_id: ObjectId,
    pub kind: LocKind,
    pub orientations: [ObjectOrientation; 4],
}

impl CornerAsset {
    pub const fn orientation_for(self, turn: CornerTurn) -> ObjectOrientation {
        self.orientations[turn.index()]
    }

    pub fn placement(self, tile: MapTile, turn: CornerTurn, plane: SourcePlane) -> ObjectPlacement {
        ObjectPlacement {
            object_id: self.object_id,
            position: MapPosition::new(tile, plane),
            kind: self.kind,
            orientation: self.orientation_for(turn),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WeightedEdgeAsset {
    pub asset: EdgeAsset,
    pub weight: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildingDetailPalette {
    pub base: BuildingPalette,
    pub exterior_corner: Option<CornerAsset>,
    pub door_variants: Vec<WeightedEdgeAsset>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DetailedBuildingElement {
    Edge(SemanticEdge),
    ExteriorCorner {
        tile: MapTile,
        turn: CornerTurn,
    },
    DoorVariant {
        edge: SemanticEdge,
        variant: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetailedBuildingArchitecture {
    floor_tiles: Vec<MapTile>,
    elements: Vec<DetailedBuildingElement>,
}

impl DetailedBuildingArchitecture {
    pub fn floor_tiles(&self) -> &[MapTile] {
        &self.floor_tiles
    }

    pub fn elements(&self) -> &[DetailedBuildingElement] {
        &self.elements
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildingDetailError {
    InvalidPlan {
        report: BuildingValidationReport,
    },
    Architecture {
        error: BuildingArchitectureError,
    },
    Refinement {
        error: FacadeRefinementError,
    },
    ZeroDoorVariantWeight {
        index: usize,
    },
    DoorVariantWeightOverflow,
    MissingSnapshotTerrain {
        tile: MapTile,
    },
    MissingAsset {
        edge: SemanticEdge,
    },
    MissingDoorVariant {
        edge: SemanticEdge,
        variant: usize,
    },
    DuplicateObjectPlacement {
        placement: ObjectPlacement,
    },
}

pub fn detail_refined_building_architecture(
    refined: &RefinedBuildingArchitecture,
    seed: u64,
    door_variant_weights: &[u32],
) -> Result<DetailedBuildingArchitecture, BuildingDetailError> {
    let total_door_weight = validate_variant_weights(door_variant_weights)?;
    let mut shell_directions = BTreeMap::<MapTile, Vec<CardinalDirection>>::new();
    for edge in refined.edges() {
        if edge.feature == EdgeFeature::ExteriorWall {
            shell_directions
                .entry(edge.tile)
                .or_default()
                .push(edge.direction);
        }
    }

    let corners = shell_directions
        .into_iter()
        .filter_map(|(tile, directions)| corner_turn(&directions).map(|turn| (tile, turn)))
        .collect::<BTreeMap<_, _>>();
    let mut consumed = BTreeSet::<(MapTile, CardinalDirection)>::new();
    let mut elements = Vec::new();

    for (&tile, &turn) in &corners {
        for direction in turn.directions() {
            consumed.insert((tile, direction));
        }
        elements.push(DetailedBuildingElement::ExteriorCorner { tile, turn });
    }

    for &edge in refined.edges() {
        if consumed.contains(&(edge.tile, edge.direction)) {
            continue;
        }
        if edge.feature == EdgeFeature::Door
            && let Some(total_weight) = total_door_weight
        {
            let variant =
                choose_weighted_variant(seed, edge, door_variant_weights, total_weight);
            elements.push(DetailedBuildingElement::DoorVariant { edge, variant });
        } else {
            elements.push(DetailedBuildingElement::Edge(edge));
        }
    }

    elements.sort();
    Ok(DetailedBuildingArchitecture {
        floor_tiles: refined.floor_tiles().to_vec(),
        elements,
    })
}

pub fn build_detailed_wfc_building_variation_patch(
    snapshot: &GenerationSnapshot,
    plan: &BuildingPlan,
    palette: &BuildingDetailPalette,
    variation: u32,
    config: FacadeWfcConfig,
    policy: FacadeRefinementPolicy,
) -> Result<MapPatch, BuildingDetailError> {
    let report = validate_building_plan(plan, snapshot);
    if !report.is_valid() {
        return Err(BuildingDetailError::InvalidPlan { report });
    }

    let architecture = derive_building_architecture(plan)
        .map_err(|error| BuildingDetailError::Architecture { error })?;
    let refined = refine_exterior_walls_for_variation(
        &architecture,
        plan.request.seed,
        variation,
        config,
        policy,
    )
    .map_err(|error| BuildingDetailError::Refinement { error })?;
    let weights = palette
        .door_variants
        .iter()
        .map(|variant| variant.weight)
        .collect::<Vec<_>>();
    let detail_seed = mix64(
        facade_variation_seed(plan.request.seed, variation) ^ 0x6a09_e667_f3bc_c909,
    );
    let detailed = detail_refined_building_architecture(&refined, detail_seed, &weights)?;
    realize_detailed(snapshot, plan, palette, &detailed)
}

fn realize_detailed(
    snapshot: &GenerationSnapshot,
    plan: &BuildingPlan,
    palette: &BuildingDetailPalette,
    architecture: &DetailedBuildingArchitecture,
) -> Result<MapPatch, BuildingDetailError> {
    let mut ops = Vec::new();
    for &tile in architecture.floor_tiles() {
        let Some(before) = snapshot.terrain_at(tile) else {
            return Err(BuildingDetailError::MissingSnapshotTerrain { tile });
        };
        let after = TerrainTile {
            underlay: palette.base.floor.underlay,
            overlay: palette.base.floor.overlay,
            flags: before.flags,
        };
        if before != after {
            ops.push(MapOp::SetTerrain {
                coord: MapPosition::new(tile, plan.request.base_plane),
                before,
                after,
            });
        }
    }

    let mut placements = HashSet::new();
    for &element in architecture.elements() {
        match element {
            DetailedBuildingElement::Edge(edge) => {
                if let Some(asset) = base_asset_for(palette.base, edge)? {
                    push_placement(
                        &mut ops,
                        &mut placements,
                        asset.placement(edge, plan.request.base_plane),
                    )?;
                }
            }
            DetailedBuildingElement::ExteriorCorner { tile, turn } => {
                if let Some(asset) = palette.exterior_corner {
                    push_placement(
                        &mut ops,
                        &mut placements,
                        asset.placement(tile, turn, plan.request.base_plane),
                    )?;
                } else {
                    for direction in turn.directions() {
                        let edge = SemanticEdge {
                            tile,
                            direction,
                            feature: EdgeFeature::ExteriorWall,
                        };
                        push_placement(
                            &mut ops,
                            &mut placements,
                            palette
                                .base
                                .exterior_wall
                                .placement(edge, plan.request.base_plane),
                        )?;
                    }
                }
            }
            DetailedBuildingElement::DoorVariant { edge, variant } => {
                let Some(weighted) = palette.door_variants.get(variant) else {
                    return Err(BuildingDetailError::MissingDoorVariant { edge, variant });
                };
                push_placement(
                    &mut ops,
                    &mut placements,
                    weighted.asset.placement(edge, plan.request.base_plane),
                )?;
            }
        }
    }

    Ok(MapPatch::new(ops))
}

fn base_asset_for(
    palette: BuildingPalette,
    edge: SemanticEdge,
) -> Result<Option<EdgeAsset>, BuildingDetailError> {
    match edge.feature {
        EdgeFeature::Open => Ok(None),
        EdgeFeature::ExteriorWall => Ok(Some(palette.exterior_wall)),
        EdgeFeature::InteriorWall => Ok(Some(palette.interior_wall)),
        EdgeFeature::Door => Ok(Some(palette.door)),
        EdgeFeature::Window => palette
            .window
            .map(Some)
            .ok_or(BuildingDetailError::MissingAsset { edge }),
    }
}

fn push_placement(
    ops: &mut Vec<MapOp>,
    placements: &mut HashSet<ObjectPlacement>,
    placement: ObjectPlacement,
) -> Result<(), BuildingDetailError> {
    if !placements.insert(placement) {
        return Err(BuildingDetailError::DuplicateObjectPlacement { placement });
    }
    ops.push(MapOp::AddObject { placement });
    Ok(())
}

fn validate_variant_weights(weights: &[u32]) -> Result<Option<u64>, BuildingDetailError> {
    if weights.is_empty() {
        return Ok(None);
    }
    let mut total = 0_u64;
    for (index, &weight) in weights.iter().enumerate() {
        if weight == 0 {
            return Err(BuildingDetailError::ZeroDoorVariantWeight { index });
        }
        total = total
            .checked_add(u64::from(weight))
            .ok_or(BuildingDetailError::DoorVariantWeightOverflow)?;
    }
    Ok(Some(total))
}

fn choose_weighted_variant(
    seed: u64,
    edge: SemanticEdge,
    weights: &[u32],
    total_weight: u64,
) -> usize {
    let mut ticket = detail_random(seed, edge) % total_weight;
    for (index, &weight) in weights.iter().enumerate() {
        let weight = u64::from(weight);
        if ticket < weight {
            return index;
        }
        ticket -= weight;
    }
    unreachable!("positive validated weights always select a variant")
}

fn detail_random(seed: u64, edge: SemanticEdge) -> u64 {
    let x = u64::from(u32::from_ne_bytes(edge.tile.x.to_ne_bytes()));
    let y = u64::from(u32::from_ne_bytes(edge.tile.y.to_ne_bytes()));
    let direction = u64::try_from(edge.direction.index()).unwrap_or(0);
    mix64(seed ^ x.rotate_left(13) ^ y.rotate_left(37) ^ direction.rotate_left(53))
}

fn corner_turn(directions: &[CardinalDirection]) -> Option<CornerTurn> {
    if directions.len() != 2 {
        return None;
    }
    let north = directions.contains(&CardinalDirection::North);
    let east = directions.contains(&CardinalDirection::East);
    let south = directions.contains(&CardinalDirection::South);
    let west = directions.contains(&CardinalDirection::West);
    match (north, east, south, west) {
        (true, true, false, false) => Some(CornerTurn::NorthEast),
        (false, true, true, false) => Some(CornerTurn::EastSouth),
        (false, false, true, true) => Some(CornerTurn::SouthWest),
        (true, false, false, true) => Some(CornerTurn::WestNorth),
        _ => None,
    }
}

fn mix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BuildingArchetype, BuildingConstraints, BuildingRequest, BuildingStyle, FloorMaterial,
        HeightVertexCoord, MapDocument, RectSite, capture_generation_snapshot, plan_basic_house,
        refine_exterior_walls_with_wfc,
    };
    use osrs_core::ids::FloorUnderlayId;

    fn plane() -> SourcePlane {
        SourcePlane::new(0).unwrap_or_else(|| unreachable!("plane zero is valid"))
    }

    fn request() -> BuildingRequest {
        BuildingRequest {
            seed: 42,
            site: RectSite::new(MapTile::new(3200, 3200), 12, 10)
                .unwrap_or_else(|| unreachable!("test site is valid")),
            base_plane: plane(),
            floors: 1,
            archetype: BuildingArchetype::House,
            style: BuildingStyle::Generic,
            constraints: BuildingConstraints::default(),
        }
    }

    fn heights_for(request: BuildingRequest) -> Vec<(HeightVertexCoord, i32)> {
        let mut heights = Vec::new();
        for local_y in 0..=request.site.height() {
            for local_x in 0..=request.site.width() {
                heights.push((
                    HeightVertexCoord::new(
                        MapTile::new(
                            request.site.origin().x + i32::from(local_x),
                            request.site.origin().y + i32::from(local_y),
                        ),
                        request.base_plane,
                    ),
                    -64,
                ));
            }
        }
        heights
    }

    fn orientations() -> [ObjectOrientation; 4] {
        [0, 1, 2, 3].map(|value| {
            ObjectOrientation::new(value)
                .unwrap_or_else(|| unreachable!("test orientation is valid"))
        })
    }

    fn edge_asset(object_id: u32, kind: u8) -> EdgeAsset {
        EdgeAsset {
            object_id: ObjectId::new(object_id),
            kind: LocKind::new(kind).unwrap_or_else(|| unreachable!("test loc kind is valid")),
            orientations: orientations(),
        }
    }

    fn detail_palette() -> BuildingDetailPalette {
        BuildingDetailPalette {
            base: BuildingPalette {
                floor: FloorMaterial {
                    underlay: Some(FloorUnderlayId::new(7)),
                    overlay: None,
                },
                exterior_wall: edge_asset(100, 0),
                interior_wall: edge_asset(101, 0),
                door: edge_asset(102, 0),
                window: Some(edge_asset(103, 0)),
            },
            exterior_corner: Some(CornerAsset {
                object_id: ObjectId::new(200),
                kind: LocKind::new(2).unwrap_or_else(|| unreachable!("test loc kind is valid")),
                orientations: orientations(),
            }),
            door_variants: vec![
                WeightedEdgeAsset {
                    asset: edge_asset(300, 0),
                    weight: 1,
                },
                WeightedEdgeAsset {
                    asset: edge_asset(301, 0),
                    weight: 3,
                },
            ],
        }
    }

    fn refined() -> RefinedBuildingArchitecture {
        let plan = plan_basic_house(request())
            .unwrap_or_else(|error| unreachable!("valid house plan: {error:?}"));
        let architecture = derive_building_architecture(&plan)
            .unwrap_or_else(|error| unreachable!("valid architecture: {error:?}"));
        refine_exterior_walls_with_wfc(&architecture, 77, FacadeWfcConfig::default())
            .unwrap_or_else(|error| unreachable!("valid refinement: {error:?}"))
    }

    #[test]
    fn rectangular_house_collapses_four_shell_corners() {
        let detailed = detail_refined_building_architecture(&refined(), 9, &[1, 3])
            .unwrap_or_else(|error| unreachable!("valid detail plan: {error:?}"));
        let corner_count = detailed
            .elements()
            .iter()
            .filter(|element| {
                matches!(element, DetailedBuildingElement::ExteriorCorner { .. })
            })
            .count();

        assert_eq!(corner_count, 4);
    }

    #[test]
    fn door_variant_selection_is_deterministic() {
        let refined = refined();
        let first = detail_refined_building_architecture(&refined, 91, &[1, 3])
            .unwrap_or_else(|error| unreachable!("valid detail plan: {error:?}"));
        let second = detail_refined_building_architecture(&refined, 91, &[1, 3])
            .unwrap_or_else(|error| unreachable!("valid detail plan: {error:?}"));

        assert_eq!(first, second);
        assert!(first.elements().iter().any(|element| {
            matches!(element, DetailedBuildingElement::DoorVariant { .. })
        }));
    }

    #[test]
    fn zero_weight_door_variant_is_rejected() {
        assert_eq!(
            detail_refined_building_architecture(&refined(), 1, &[1, 0, 2]),
            Err(BuildingDetailError::ZeroDoorVariantWeight { index: 1 })
        );
    }

    #[test]
    fn detailed_patch_uses_corner_and_door_assets_and_round_trips() {
        let request = request();
        let plan = plan_basic_house(request)
            .unwrap_or_else(|error| unreachable!("valid house plan: {error:?}"));
        let mut document = MapDocument::from_parts([], heights_for(request), []);
        let snapshot = capture_generation_snapshot(&document, request, []);
        let patch = build_detailed_wfc_building_variation_patch(
            &snapshot,
            &plan,
            &detail_palette(),
            2,
            FacadeWfcConfig::default(),
            FacadeRefinementPolicy::default(),
        )
        .unwrap_or_else(|error| unreachable!("valid detailed patch: {error:?}"));

        let corner_count = patch
            .ops()
            .iter()
            .filter(|op| {
                matches!(
                    op,
                    MapOp::AddObject { placement }
                        if placement.object_id == ObjectId::new(200)
                )
            })
            .count();
        assert_eq!(corner_count, 4);
        assert!(!patch.ops().iter().any(|op| {
            matches!(
                op,
                MapOp::AddObject { placement }
                    if placement.object_id == ObjectId::new(102)
            )
        }));

        let before = document.clone();
        document
            .apply_patch(&patch)
            .unwrap_or_else(|error| unreachable!("valid detailed patch: {error:?}"));
        let after = document.clone();
        assert_ne!(after, before);

        document
            .apply_patch(&patch.inverse())
            .unwrap_or_else(|error| unreachable!("valid inverse patch: {error:?}"));
        assert_eq!(document, before);

        document
            .apply_patch(&patch)
            .unwrap_or_else(|error| unreachable!("valid detailed redo: {error:?}"));
        assert_eq!(document, after);
    }
}
