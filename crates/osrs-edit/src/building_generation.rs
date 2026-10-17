//! End-to-end headless building generation into one reversible map patch.
//!
//! This layer composes the existing macro-program, BSP topology, routing, facade/detail,
//! vertical, and unified roof stages without introducing an alternate realization path.
//! Generation remains single-floor at this checkpoint because `GenerationSnapshot` and detailed
//! shell realization currently describe terrain only on the request base plane.

use crate::{
    BspLayoutError, BspTopologyError, BuildingArchitectureError, BuildingDetailError,
    BuildingDetailPalette, BuildingPlan, BuildingVerticalError, FacadeRefinementPolicy,
    FacadeWfcConfig, GenerationSnapshot, MapOp, MapPatch, ObjectPlacement, ProgramRoutingError,
    RoofGenerationError, RoofJunctionPatternPalette, RoofPalette, RoofPatternSet, WfcCatalog,
    build_bsp_building_plan, build_detailed_wfc_building_variation_patch, building_program,
    derive_building_architecture, generate_roof_patch, pack_building_program,
    plan_vertical_connectivity, route_program_connections,
};
use std::collections::HashSet;

/// Inputs that control local facade/detail and roof realization while the snapshot request owns
/// macro geometry, archetype, style, floor count, and the authoritative building seed.
#[derive(Clone, Copy)]
pub struct BuildingGenerationConfig<'a> {
    pub detail_palette: &'a BuildingDetailPalette,
    pub facade_variation: u32,
    pub facade_config: FacadeWfcConfig,
    pub facade_policy: FacadeRefinementPolicy,
    pub roof_catalog: &'a WfcCatalog,
    pub roof_patterns: RoofPatternSet,
    pub roof_palette: &'a RoofPalette,
    pub roof_junction_patterns: &'a RoofJunctionPatternPalette,
    pub roof_max_backtracks: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildingGenerationError {
    UnsupportedFloorCount { floors: u8 },
    Layout(BspLayoutError),
    Topology(BspTopologyError),
    Routing(ProgramRoutingError),
    Architecture(BuildingArchitectureError),
    Detail(BuildingDetailError),
    Vertical(BuildingVerticalError),
    Roof(RoofGenerationError),
    DuplicateObjectPlacement { placement: ObjectPlacement },
}

/// Generate a complete single-floor building from the request captured by `snapshot`.
///
/// The macro path is:
/// archetype program -> BSP packing -> semantic topology -> program routing -> detailed facade
/// realization -> vertical semantics -> unified roof generation -> one `MapPatch`.
///
/// Multi-floor orchestration is rejected explicitly until snapshots and detailed shell realization
/// can represent every generated plane. The standalone vertical planner remains multi-floor capable.
pub fn generate_building_patch(
    snapshot: &GenerationSnapshot,
    config: BuildingGenerationConfig<'_>,
) -> Result<MapPatch, BuildingGenerationError> {
    let request = snapshot.request();
    ensure_single_floor(request.floors)?;

    let program = building_program(request.archetype);
    let layout = pack_building_program(&program, request.site, request.seed)
        .map_err(BuildingGenerationError::Layout)?;
    let building = build_bsp_building_plan(request, &program, &layout)
        .map_err(BuildingGenerationError::Topology)?;
    route_program_connections(&building).map_err(BuildingGenerationError::Routing)?;

    generate_building_plan_patch(snapshot, &building.plan, config)
}

/// Compile an already-planned single-floor building through detail, vertical, and unified roof
/// generation. This lower-level entry point keeps non-rectangular semantic plans on the same final
/// realization path, including complex sectioned roofs and junction repair.
pub fn generate_building_plan_patch(
    snapshot: &GenerationSnapshot,
    plan: &BuildingPlan,
    config: BuildingGenerationConfig<'_>,
) -> Result<MapPatch, BuildingGenerationError> {
    ensure_single_floor(plan.request.floors)?;

    let architecture =
        derive_building_architecture(plan).map_err(BuildingGenerationError::Architecture)?;
    let floor_tiles = vec![architecture.floor_tiles().to_vec()];
    let vertical = plan_vertical_connectivity(plan.request, &floor_tiles)
        .map_err(BuildingGenerationError::Vertical)?;

    let body = build_detailed_wfc_building_variation_patch(
        snapshot,
        plan,
        config.detail_palette,
        config.facade_variation,
        config.facade_config,
        config.facade_policy,
    )
    .map_err(BuildingGenerationError::Detail)?;

    let roof = generate_roof_patch(
        &vertical,
        config.roof_catalog,
        config.roof_patterns,
        config.roof_palette,
        config.roof_junction_patterns,
        plan.request.seed,
        config.roof_max_backtracks,
    )
    .map_err(BuildingGenerationError::Roof)?;

    compose_generation_patches([body, roof])
}

fn ensure_single_floor(floors: u8) -> Result<(), BuildingGenerationError> {
    if floors == 1 {
        Ok(())
    } else {
        Err(BuildingGenerationError::UnsupportedFloorCount { floors })
    }
}

fn compose_generation_patches(
    patches: impl IntoIterator<Item = MapPatch>,
) -> Result<MapPatch, BuildingGenerationError> {
    let mut ops = Vec::new();
    let mut added_objects = HashSet::new();

    for patch in patches {
        for &op in patch.ops() {
            if let MapOp::AddObject { placement } = op
                && !added_objects.insert(placement)
            {
                return Err(BuildingGenerationError::DuplicateObjectPlacement { placement });
            }
            ops.push(op);
        }
    }

    Ok(MapPatch::new(ops))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BuildingArchetype, BuildingConstraints, BuildingPalette, BuildingRequest, BuildingStyle,
        CardinalDirection, CornerAsset, EdgeAsset, FloorMaterial, HeightVertexCoord, LocKind,
        MapDocument, PatternDomain, PortalKind, RectSite, RoofAsset, RoofJunctionPatternKey,
        RoofJunctionPatternPaletteEntry, RoofJunctionPatternRequirement, RoofPaletteEntry,
        RoofSectionJunctionRepairRole, RoomConnection, RoomId, RoomKind, RoomRect, RoomSpec,
        WeightedEdgeAsset, WfcPatternId, WfcPatternSpec, WfcSocket, capture_generation_snapshot,
    };
    use osrs_core::coords::{MapTile, SourcePlane};
    use osrs_core::ids::{FloorUnderlayId, ObjectId};

    fn plane(value: u8) -> SourcePlane {
        SourcePlane::new(value).unwrap_or_else(|| unreachable!("test plane is valid"))
    }

    fn request(
        archetype: BuildingArchetype,
        floors: u8,
        width: u16,
        height: u16,
    ) -> BuildingRequest {
        BuildingRequest {
            seed: 77,
            site: RectSite::new(MapTile::new(3200, 3200), width, height)
                .unwrap_or_else(|| unreachable!("test site is valid")),
            base_plane: plane(1),
            floors,
            archetype,
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

    fn orientations() -> [crate::ObjectOrientation; 4] {
        [0, 1, 2, 3].map(|value| {
            crate::ObjectOrientation::new(value)
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

    fn pattern(value: u8) -> WfcPatternId {
        WfcPatternId::new(value).unwrap_or_else(|| unreachable!("test pattern id is valid"))
    }

    fn domain(values: &[u8]) -> PatternDomain {
        PatternDomain::from_patterns(values.iter().copied().map(pattern))
    }

    fn roof_patterns() -> RoofPatternSet {
        RoofPatternSet {
            ridge: domain(&[0, 1]),
            slopes: [domain(&[2]), domain(&[3]), domain(&[4]), domain(&[5])],
            eaves: [domain(&[6]), domain(&[7]), domain(&[8]), domain(&[9])],
        }
    }

    fn roof_catalog() -> WfcCatalog {
        WfcCatalog::new((0..10).map(|index| WfcPatternSpec {
            weight: u32::try_from(index + 1).unwrap_or_else(|_| unreachable!("small test catalog")),
            sockets: [WfcSocket(1); 4],
        }))
        .unwrap_or_else(|error| unreachable!("valid test catalog: {error:?}"))
    }

    fn roof_palette() -> RoofPalette {
        RoofPalette::new((0_u8..12).map(|value| {
            RoofPaletteEntry {
                pattern: pattern(value),
                asset: RoofAsset {
                    object_id: ObjectId::new(20_000 + u32::from(value)),
                    kind: LocKind::new(10)
                        .unwrap_or_else(|| unreachable!("test loc kind is valid")),
                    orientation: crate::ObjectOrientation::new(value % 4)
                        .unwrap_or_else(|| unreachable!("test orientation is valid")),
                },
            }
        }))
        .unwrap_or_else(|error| unreachable!("valid roof palette: {error:?}"))
    }

    fn junction_key(direction_to_other: CardinalDirection) -> RoofJunctionPatternKey {
        RoofJunctionPatternKey::new([RoofJunctionPatternRequirement {
            role: RoofSectionJunctionRepairRole::RidgeIntersection,
            direction_to_other,
        }])
    }

    fn junction_patterns() -> RoofJunctionPatternPalette {
        RoofJunctionPatternPalette::new([
            RoofJunctionPatternPaletteEntry {
                key: junction_key(CardinalDirection::North),
                pattern: pattern(10),
            },
            RoofJunctionPatternPaletteEntry {
                key: junction_key(CardinalDirection::South),
                pattern: pattern(11),
            },
        ])
        .unwrap_or_else(|error| unreachable!("valid junction palette: {error:?}"))
    }

    fn complex_plan(request: BuildingRequest) -> BuildingPlan {
        let bottom = RoomId::new(0);
        let stem = RoomId::new(1);
        BuildingPlan {
            request,
            rooms: vec![
                RoomSpec {
                    id: bottom,
                    kind: RoomKind::Hallway,
                    rect: RoomRect {
                        origin: MapTile::new(3200, 3200),
                        width: 5,
                        height: 1,
                    },
                },
                RoomSpec {
                    id: stem,
                    kind: RoomKind::Living,
                    rect: RoomRect {
                        origin: MapTile::new(3201, 3201),
                        width: 3,
                        height: 3,
                    },
                },
            ],
            connections: vec![
                RoomConnection {
                    from: None,
                    to: stem,
                    doorway: MapTile::new(3201, 3201),
                    kind: PortalKind::Door,
                },
                RoomConnection {
                    from: Some(bottom),
                    to: stem,
                    doorway: MapTile::new(3201, 3200),
                    kind: PortalKind::Open,
                },
            ],
            entrance: MapTile::new(3200, 3201),
        }
    }

    #[test]
    fn archetype_generation_produces_one_deterministic_body_and_roof_patch() {
        let request = request(BuildingArchetype::Bank, 1, 28, 22);
        let document = MapDocument::from_parts([], heights_for(request), []);
        let snapshot = capture_generation_snapshot(&document, request, []);
        let detail_palette = detail_palette();
        let roof_catalog = roof_catalog();
        let roof_palette = roof_palette();
        let junction_patterns = junction_patterns();
        let config = BuildingGenerationConfig {
            detail_palette: &detail_palette,
            facade_variation: 2,
            facade_config: FacadeWfcConfig::default(),
            facade_policy: FacadeRefinementPolicy::default(),
            roof_catalog: &roof_catalog,
            roof_patterns: roof_patterns(),
            roof_palette: &roof_palette,
            roof_junction_patterns: &junction_patterns,
            roof_max_backtracks: 32,
        };

        let first = generate_building_patch(&snapshot, config)
            .unwrap_or_else(|error| unreachable!("valid building generation: {error:?}"));
        let second = generate_building_patch(&snapshot, config)
            .unwrap_or_else(|error| unreachable!("repeat generation succeeds: {error:?}"));

        assert_eq!(first, second);
        assert!(
            first
                .ops()
                .iter()
                .any(|op| matches!(op, MapOp::SetTerrain { .. }))
        );
        assert!(first.ops().iter().any(|op| {
            matches!(
                op,
                MapOp::AddObject { placement }
                    if placement.object_id.get() >= 20_000
            )
        }));
    }

    #[test]
    fn complex_semantic_building_uses_repaired_roof_and_round_trips_exactly() {
        let request = request(BuildingArchetype::House, 1, 8, 8);
        let plan = complex_plan(request);
        let mut document = MapDocument::from_parts([], heights_for(request), []);
        let original = document.clone();
        let snapshot = capture_generation_snapshot(&document, request, []);
        let detail_palette = detail_palette();
        let roof_catalog = roof_catalog();
        let roof_palette = roof_palette();
        let junction_patterns = junction_patterns();
        let config = BuildingGenerationConfig {
            detail_palette: &detail_palette,
            facade_variation: 3,
            facade_config: FacadeWfcConfig::default(),
            facade_policy: FacadeRefinementPolicy::default(),
            roof_catalog: &roof_catalog,
            roof_patterns: roof_patterns(),
            roof_palette: &roof_palette,
            roof_junction_patterns: &junction_patterns,
            roof_max_backtracks: 32,
        };

        let first = generate_building_plan_patch(&snapshot, &plan, config)
            .unwrap_or_else(|error| unreachable!("valid complex generation: {error:?}"));
        let second = generate_building_plan_patch(&snapshot, &plan, config)
            .unwrap_or_else(|error| unreachable!("repeat complex generation: {error:?}"));
        assert_eq!(first, second);

        let roof_object_count = first
            .ops()
            .iter()
            .filter(|op| {
                matches!(
                    op,
                    MapOp::AddObject { placement }
                        if placement.object_id.get() >= 20_000
                )
            })
            .count();
        assert_eq!(roof_object_count, 14);
        assert_eq!(
            first
                .ops()
                .iter()
                .filter(|op| {
                    matches!(
                        op,
                        MapOp::AddObject { placement }
                            if [ObjectId::new(20_010), ObjectId::new(20_011)]
                                .contains(&placement.object_id)
                    )
                })
                .count(),
            6
        );

        document
            .apply_patch(&first)
            .unwrap_or_else(|error| unreachable!("complete building patch applies: {error:?}"));
        assert_ne!(document, original);
        document
            .apply_patch(&first.inverse())
            .unwrap_or_else(|error| unreachable!("complete building inverse applies: {error:?}"));
        assert_eq!(document, original);
    }

    #[test]
    fn multi_floor_generation_is_rejected_until_all_planes_can_be_realized() {
        let request = request(BuildingArchetype::House, 2, 12, 10);
        let document = MapDocument::from_parts([], heights_for(request), []);
        let snapshot = capture_generation_snapshot(&document, request, []);
        let detail_palette = detail_palette();
        let roof_catalog = roof_catalog();
        let roof_palette = roof_palette();
        let junction_patterns = junction_patterns();
        let config = BuildingGenerationConfig {
            detail_palette: &detail_palette,
            facade_variation: 0,
            facade_config: FacadeWfcConfig::default(),
            facade_policy: FacadeRefinementPolicy::default(),
            roof_catalog: &roof_catalog,
            roof_patterns: roof_patterns(),
            roof_palette: &roof_palette,
            roof_junction_patterns: &junction_patterns,
            roof_max_backtracks: 32,
        };

        assert_eq!(
            generate_building_patch(&snapshot, config),
            Err(BuildingGenerationError::UnsupportedFloorCount { floors: 2 })
        );
    }
}
