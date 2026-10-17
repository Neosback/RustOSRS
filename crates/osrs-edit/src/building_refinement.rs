//! WFC-based local architectural refinement with global topology held fixed.
//!
//! The first refinement pass operates only on straight exterior-wall runs. Door/open portals,
//! corners, room geometry, corridor geometry, and reachability are immutable inputs. A compact
//! WFC automaton chooses isolated windows among otherwise solid wall edges.

use crate::{
    BuildingArchitecture, BuildingArchitectureError, BuildingPalette, BuildingPlan,
    BuildingValidationReport, EdgeFeature, GenerationSnapshot, MapOp, MapPatch, MapPosition,
    ObjectPlacement, PatternDomain, SemanticEdge, TerrainTile, WfcCatalog, WfcCatalogError,
    WfcConstraint, WfcCoord, WfcPatternId, WfcPatternSpec, WfcSocket, WfcSolveError,
    derive_building_architecture, solve_wfc, validate_building_plan,
};
use osrs_core::coords::MapTile;
use std::collections::{BTreeMap, HashSet};

const STABLE_WALL_PATTERN: u8 = 0;
const BEFORE_WINDOW_PATTERN: u8 = 1;
const WINDOW_PATTERN: u8 = 2;
const AFTER_WINDOW_PATTERN: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FacadeWfcConfig {
    pub wall_weight: u32,
    pub window_weight: u32,
    pub min_run_length: u16,
    pub max_backtracks: u32,
}

impl Default for FacadeWfcConfig {
    fn default() -> Self {
        Self {
            wall_weight: 8,
            window_weight: 3,
            min_run_length: 3,
            max_backtracks: 64,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefinedBuildingArchitecture {
    floor_tiles: Vec<MapTile>,
    edges: Vec<SemanticEdge>,
}

impl RefinedBuildingArchitecture {
    pub fn floor_tiles(&self) -> &[MapTile] {
        &self.floor_tiles
    }

    pub fn edges(&self) -> &[SemanticEdge] {
        &self.edges
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FacadeRefinementError {
    InvalidConfig,
    InvalidPlan { report: BuildingValidationReport },
    Architecture { error: BuildingArchitectureError },
    Catalog { error: WfcCatalogError },
    Solve { error: WfcSolveError },
    MissingSnapshotTerrain { tile: MapTile },
    MissingWindowAsset { edge: SemanticEdge },
    DuplicateObjectPlacement { placement: ObjectPlacement },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct RunKey {
    direction: crate::CardinalDirection,
    fixed: i32,
}

pub fn refine_exterior_walls_with_wfc(
    architecture: &BuildingArchitecture,
    seed: u64,
    config: FacadeWfcConfig,
) -> Result<RefinedBuildingArchitecture, FacadeRefinementError> {
    if config.wall_weight == 0 || config.window_weight == 0 || config.min_run_length < 3 {
        return Err(FacadeRefinementError::InvalidConfig);
    }

    let catalog = facade_catalog(config)?;
    let mut exterior_count_by_tile = BTreeMap::<MapTile, usize>::new();
    for edge in architecture.edges() {
        if edge.feature == EdgeFeature::ExteriorWall {
            *exterior_count_by_tile.entry(edge.tile).or_default() += 1;
        }
    }

    let mut groups = BTreeMap::<RunKey, Vec<SemanticEdge>>::new();
    for &edge in architecture.edges() {
        if edge.feature != EdgeFeature::ExteriorWall
            || exterior_count_by_tile.get(&edge.tile).copied().unwrap_or(0) != 1
        {
            continue;
        }
        let key = match edge.direction {
            crate::CardinalDirection::North | crate::CardinalDirection::South => RunKey {
                direction: edge.direction,
                fixed: edge.tile.y,
            },
            crate::CardinalDirection::East | crate::CardinalDirection::West => RunKey {
                direction: edge.direction,
                fixed: edge.tile.x,
            },
        };
        groups.entry(key).or_default().push(edge);
    }

    let mut features = architecture
        .edges()
        .iter()
        .map(|edge| ((edge.tile, edge.direction), edge.feature))
        .collect::<BTreeMap<_, _>>();

    for (key, mut edges) in groups {
        edges.sort_by_key(|edge| tangent(edge.tile, key.direction));
        for run in contiguous_runs(&edges, key.direction) {
            if run.len() < usize::from(config.min_run_length) {
                continue;
            }
            refine_run(&catalog, run, seed, key, config, &mut features)?;
        }
    }

    let edges = features
        .into_iter()
        .map(|((tile, direction), feature)| SemanticEdge {
            tile,
            direction,
            feature,
        })
        .collect();

    Ok(RefinedBuildingArchitecture {
        floor_tiles: architecture.floor_tiles().to_vec(),
        edges,
    })
}

pub fn build_wfc_refined_building_patch(
    snapshot: &GenerationSnapshot,
    plan: &BuildingPlan,
    palette: BuildingPalette,
    refinement_seed: u64,
    config: FacadeWfcConfig,
) -> Result<MapPatch, FacadeRefinementError> {
    let report = validate_building_plan(plan, snapshot);
    if !report.is_valid() {
        return Err(FacadeRefinementError::InvalidPlan { report });
    }
    let architecture = derive_building_architecture(plan)
        .map_err(|error| FacadeRefinementError::Architecture { error })?;
    let refined = refine_exterior_walls_with_wfc(&architecture, refinement_seed, config)?;
    realize_refined(snapshot, plan, palette, &refined)
}

fn realize_refined(
    snapshot: &GenerationSnapshot,
    plan: &BuildingPlan,
    palette: BuildingPalette,
    architecture: &RefinedBuildingArchitecture,
) -> Result<MapPatch, FacadeRefinementError> {
    let mut ops = Vec::new();
    for &tile in architecture.floor_tiles() {
        let Some(before) = snapshot.terrain_at(tile) else {
            return Err(FacadeRefinementError::MissingSnapshotTerrain { tile });
        };
        let after = TerrainTile {
            underlay: palette.floor.underlay,
            overlay: palette.floor.overlay,
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
    for &edge in architecture.edges() {
        let asset = match edge.feature {
            EdgeFeature::Open => continue,
            EdgeFeature::ExteriorWall => palette.exterior_wall,
            EdgeFeature::InteriorWall => palette.interior_wall,
            EdgeFeature::Door => palette.door,
            EdgeFeature::Window => palette
                .window
                .ok_or(FacadeRefinementError::MissingWindowAsset { edge })?,
        };
        let placement = asset.placement(edge, plan.request.base_plane);
        if !placements.insert(placement) {
            return Err(FacadeRefinementError::DuplicateObjectPlacement { placement });
        }
        ops.push(MapOp::AddObject { placement });
    }

    Ok(MapPatch::new(ops))
}

fn facade_catalog(config: FacadeWfcConfig) -> Result<WfcCatalog, FacadeRefinementError> {
    let zero = WfcSocket(0);
    let enter_window = WfcSocket(1);
    let exit_window = WfcSocket(2);
    WfcCatalog::new([
        WfcPatternSpec {
            weight: config.wall_weight,
            sockets: [zero, zero, zero, zero],
        },
        WfcPatternSpec {
            weight: config.window_weight,
            sockets: [zero, enter_window, zero, zero],
        },
        WfcPatternSpec {
            weight: config.window_weight,
            sockets: [zero, exit_window, zero, enter_window],
        },
        WfcPatternSpec {
            weight: config.window_weight,
            sockets: [zero, zero, zero, exit_window],
        },
    ])
    .map_err(|error| FacadeRefinementError::Catalog { error })
}

fn refine_run(
    catalog: &WfcCatalog,
    run: &[SemanticEdge],
    base_seed: u64,
    key: RunKey,
    config: FacadeWfcConfig,
    features: &mut BTreeMap<(MapTile, crate::CardinalDirection), EdgeFeature>,
) -> Result<(), FacadeRefinementError> {
    let width = u16::try_from(run.len()).map_err(|_| FacadeRefinementError::InvalidConfig)?;
    let stable = pattern(STABLE_WALL_PATTERN);
    let before = pattern(BEFORE_WINDOW_PATTERN);
    let after = pattern(AFTER_WINDOW_PATTERN);
    let constraints = [
        WfcConstraint {
            coord: WfcCoord::new(0, 0),
            allowed: PatternDomain::from_patterns([stable, before]),
        },
        WfcConstraint {
            coord: WfcCoord::new(width - 1, 0),
            allowed: PatternDomain::from_patterns([stable, after]),
        },
    ];
    let start = tangent(run[0].tile, key.direction);
    let seed = run_seed(base_seed, key, start);
    let solution = solve_wfc(
        catalog,
        width,
        1,
        seed,
        &constraints,
        config.max_backtracks,
    )
    .map_err(|error| FacadeRefinementError::Solve { error })?;

    for (index, edge) in run.iter().enumerate() {
        let pattern = solution
            .at(WfcCoord::new(
                u16::try_from(index).unwrap_or_else(|_| unreachable!("run width is u16")),
                0,
            ))
            .unwrap_or_else(|| unreachable!("solved facade cell exists"));
        if pattern.get() == WINDOW_PATTERN {
            features.insert((edge.tile, edge.direction), EdgeFeature::Window);
        }
    }
    Ok(())
}

fn contiguous_runs(
    edges: &[SemanticEdge],
    direction: crate::CardinalDirection,
) -> Vec<&[SemanticEdge]> {
    if edges.is_empty() {
        return Vec::new();
    }
    let mut runs = Vec::new();
    let mut start = 0;
    for index in 1..edges.len() {
        if tangent(edges[index].tile, direction) != tangent(edges[index - 1].tile, direction) + 1 {
            runs.push(&edges[start..index]);
            start = index;
        }
    }
    runs.push(&edges[start..]);
    runs
}

const fn tangent(tile: MapTile, direction: crate::CardinalDirection) -> i32 {
    match direction {
        crate::CardinalDirection::North | crate::CardinalDirection::South => tile.x,
        crate::CardinalDirection::East | crate::CardinalDirection::West => tile.y,
    }
}

fn run_seed(base: u64, key: RunKey, start: i32) -> u64 {
    let direction = u64::try_from(key.direction.index()).unwrap_or(0);
    let fixed = u64::from(u32::from_ne_bytes(key.fixed.to_ne_bytes()));
    let start = u64::from(u32::from_ne_bytes(start.to_ne_bytes()));
    mix64(base ^ direction.rotate_left(7) ^ fixed.rotate_left(21) ^ start.rotate_left(39))
}

fn mix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn pattern(value: u8) -> WfcPatternId {
    WfcPatternId::new(value).unwrap_or_else(|| unreachable!("facade pattern id is valid"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BuildingArchetype, BuildingConstraints, BuildingRequest, BuildingStyle, EdgeAsset,
        FloorMaterial, HeightVertexCoord, LocKind, MapDocument, ObjectOrientation, RectSite,
        build_bsp_building_plan, building_program, capture_generation_snapshot,
        pack_building_program,
    };
    use osrs_core::coords::SourcePlane;
    use osrs_core::ids::{FloorUnderlayId, ObjectId};

    fn plane() -> SourcePlane {
        SourcePlane::new(0).unwrap_or_else(|| unreachable!("plane zero is valid"))
    }

    fn request(seed: u64) -> BuildingRequest {
        BuildingRequest {
            seed,
            site: RectSite::new(MapTile::new(3200, 3200), 28, 22)
                .unwrap_or_else(|| unreachable!("test site is valid")),
            base_plane: plane(),
            floors: 1,
            archetype: BuildingArchetype::Tavern,
            style: BuildingStyle::Generic,
            constraints: BuildingConstraints::default(),
        }
    }

    fn architecture(seed: u64) -> (BuildingRequest, BuildingPlan, BuildingArchitecture) {
        let request = request(seed);
        let program = building_program(request.archetype);
        let layout = pack_building_program(&program, request.site, request.seed)
            .unwrap_or_else(|error| unreachable!("valid BSP layout: {error:?}"));
        let generated = build_bsp_building_plan(request, &program, &layout)
            .unwrap_or_else(|error| unreachable!("valid topology: {error:?}"));
        let architecture = derive_building_architecture(&generated.plan)
            .unwrap_or_else(|error| unreachable!("valid architecture: {error:?}"));
        (request, generated.plan, architecture)
    }

    fn heights_for(request: BuildingRequest) -> Vec<(HeightVertexCoord, i32)> {
        let mut heights = Vec::new();
        for y in 0..=request.site.height() {
            for x in 0..=request.site.width() {
                heights.push((
                    HeightVertexCoord::new(
                        MapTile::new(
                            request.site.origin().x + i32::from(x),
                            request.site.origin().y + i32::from(y),
                        ),
                        request.base_plane,
                    ),
                    -64,
                ));
            }
        }
        heights
    }

    fn asset(id: u32) -> EdgeAsset {
        EdgeAsset {
            object_id: ObjectId::new(id),
            kind: LocKind::new(0).unwrap_or_else(|| unreachable!("valid loc kind")),
            orientations: [0, 1, 2, 3].map(|orientation| {
                ObjectOrientation::new(orientation)
                    .unwrap_or_else(|| unreachable!("valid orientation"))
            }),
        }
    }

    fn palette() -> BuildingPalette {
        BuildingPalette {
            floor: FloorMaterial {
                underlay: Some(FloorUnderlayId::new(7)),
                overlay: None,
            },
            exterior_wall: asset(100),
            interior_wall: asset(101),
            door: asset(102),
            window: Some(asset(103)),
        }
    }

    #[test]
    fn same_architecture_seed_and_config_produce_identical_refinement() {
        let (_, _, architecture) = architecture(42);
        let config = FacadeWfcConfig::default();
        let first = refine_exterior_walls_with_wfc(&architecture, 991, config)
            .unwrap_or_else(|error| unreachable!("valid refinement: {error:?}"));
        let second = refine_exterior_walls_with_wfc(&architecture, 991, config)
            .unwrap_or_else(|error| unreachable!("valid refinement: {error:?}"));
        assert_eq!(first, second);
    }

    #[test]
    fn refinement_never_changes_non_exterior_edges_or_corner_edges() {
        let (_, _, architecture) = architecture(83);
        let refined = refine_exterior_walls_with_wfc(
            &architecture,
            7,
            FacadeWfcConfig {
                wall_weight: 1,
                window_weight: 100,
                ..FacadeWfcConfig::default()
            },
        )
        .unwrap_or_else(|error| unreachable!("valid refinement: {error:?}"));
        let original = architecture
            .edges()
            .iter()
            .map(|edge| ((edge.tile, edge.direction), edge.feature))
            .collect::<BTreeMap<_, _>>();
        let corner_counts = architecture
            .edges()
            .iter()
            .filter(|edge| edge.feature == EdgeFeature::ExteriorWall)
            .fold(BTreeMap::<MapTile, usize>::new(), |mut counts, edge| {
                *counts.entry(edge.tile).or_default() += 1;
                counts
            });

        for edge in refined.edges() {
            let before = original[&(edge.tile, edge.direction)];
            if before != EdgeFeature::ExteriorWall
                || corner_counts.get(&edge.tile).copied().unwrap_or(0) > 1
            {
                assert_eq!(edge.feature, before);
            }
        }
    }

    #[test]
    fn generated_windows_are_isolated_and_not_run_endpoints() {
        let (_, _, architecture) = architecture(101);
        let config = FacadeWfcConfig {
            wall_weight: 1,
            window_weight: 100,
            ..FacadeWfcConfig::default()
        };
        let mut found_window = false;
        for seed in 0..64 {
            let refined = refine_exterior_walls_with_wfc(&architecture, seed, config)
                .unwrap_or_else(|error| unreachable!("valid refinement: {error:?}"));
            let window_keys = refined
                .edges()
                .iter()
                .filter(|edge| edge.feature == EdgeFeature::Window)
                .map(|edge| (edge.tile, edge.direction))
                .collect::<HashSet<_>>();
            if window_keys.is_empty() {
                continue;
            }
            found_window = true;
            for &(tile, direction) in &window_keys {
                let previous = match direction {
                    crate::CardinalDirection::North | crate::CardinalDirection::South => {
                        MapTile::new(tile.x - 1, tile.y)
                    }
                    crate::CardinalDirection::East | crate::CardinalDirection::West => {
                        MapTile::new(tile.x, tile.y - 1)
                    }
                };
                let next = match direction {
                    crate::CardinalDirection::North | crate::CardinalDirection::South => {
                        MapTile::new(tile.x + 1, tile.y)
                    }
                    crate::CardinalDirection::East | crate::CardinalDirection::West => {
                        MapTile::new(tile.x, tile.y + 1)
                    }
                };
                assert!(!window_keys.contains(&(previous, direction)));
                assert!(!window_keys.contains(&(next, direction)));
            }
            break;
        }
        assert!(found_window);
    }

    #[test]
    fn refined_patch_round_trips_through_normal_map_document_path() {
        let (request, plan, _) = architecture(72);
        let mut document = MapDocument::from_parts([], heights_for(request), []);
        let snapshot = capture_generation_snapshot(&document, request, []);
        let patch = build_wfc_refined_building_patch(
            &snapshot,
            &plan,
            palette(),
            991,
            FacadeWfcConfig::default(),
        )
        .unwrap_or_else(|error| unreachable!("valid refined patch: {error:?}"));
        let before = document.clone();

        document
            .apply_patch(&patch)
            .unwrap_or_else(|error| unreachable!("valid generated patch: {error:?}"));
        let after = document.clone();
        assert_ne!(after, before);

        document
            .apply_patch(&patch.inverse())
            .unwrap_or_else(|error| unreachable!("valid inverse patch: {error:?}"));
        assert_eq!(document, before);

        document
            .apply_patch(&patch)
            .unwrap_or_else(|error| unreachable!("valid generated redo: {error:?}"));
        assert_eq!(document, after);
    }
}
