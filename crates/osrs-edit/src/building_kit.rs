//! Building kits: the data a generated building is made of, as a versioned file.
//!
//! The generator plans architecture semantically (walls, doors, windows, corners, roof patterns)
//! and never knows an object id. A kit says which cache objects those semantic pieces become: one
//! asset per wall/door/window/corner, and a table of roof patterns with their sockets and the
//! pattern groups the roof solver draws from. Kits are plain JSON so they can be written by hand,
//! captured from real buildings, and kept beside prefabs.

use crate::{
    BuildingDetailPalette, BuildingGenerationConfig, BuildingGenerationError, BuildingPalette,
    BuildingPlan, CardinalDirection, CornerAsset, EdgeAsset, FacadeRefinementPolicy,
    FacadeWfcConfig, FloorMaterial, GenerationSnapshot, HipRoofAssets, LocKind, MapOp, MapPatch,
    ObjectOrientation, OverlayRotation, OverlayShape, PatternDomain, RoofAsset,
    RoofJunctionPatternKey, RoofJunctionPatternPalette, RoofJunctionPatternPaletteEntry,
    RoofJunctionPatternRequirement, RoofPalette, RoofPaletteEntry, RoofPatternSet,
    RoofSectionJunctionRepairRole, TerrainOverlay, WeightedEdgeAsset, WfcCatalog, WfcPatternId,
    WfcPatternSpec, WfcSocket, build_bsp_building_plan,
    build_detailed_wfc_building_variation_patch, building_program, derive_building_architecture,
    generate_building_plan_patch, hip_roof_placements, pack_building_program,
    route_program_connections,
};
use osrs_core::ids::{FloorOverlayId, FloorUnderlayId, ObjectId};
use serde::{Deserialize, Serialize};
use std::fmt;

pub const KIT_FORMAT: &str = "rustosrs-building-kit";
pub const KIT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KitAsset {
    pub id: u32,
    pub kind: u8,
    /// Object orientation to use for a piece on the north, east, south and west edge (or, for a
    /// corner, for each of its four turns).
    pub orientations: [u8; 4],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KitWeightedAsset {
    pub asset: KitAsset,
    pub weight: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KitOverlay {
    pub id: u32,
    pub shape: u8,
    pub rotation: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KitFloor {
    pub underlay: Option<u32>,
    pub overlay: Option<KitOverlay>,
}

/// One roof pattern: the object it becomes and how it joins its neighbours.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KitRoofPattern {
    pub id: u32,
    pub kind: u8,
    pub orientation: u8,
    pub weight: u32,
    /// Socket numbers on the north, east, south and west side; two patterns can sit side by side
    /// when the sockets facing each other are equal.
    pub sockets: [u16; 4],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KitJunction {
    /// `(role, direction to the other roof)`, e.g. `["ridge_intersection", "north"]`.
    pub requires: Vec<(String, String)>,
    /// Index into `roof.patterns`.
    pub pattern: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KitRoof {
    /// A pattern's number is its position in this list.
    pub patterns: Vec<KitRoofPattern>,
    pub ridge: Vec<u8>,
    /// Indexed by the downhill direction: north, east, south, west.
    pub slopes: [Vec<u8>; 4],
    pub eaves: [Vec<u8>; 4],
    pub junctions: Vec<KitJunction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KitFile {
    pub format: String,
    pub version: u32,
    pub name: String,
    pub notes: String,
    pub floor: KitFloor,
    pub exterior_wall: KitAsset,
    pub interior_wall: KitAsset,
    pub door: KitAsset,
    pub window: Option<KitAsset>,
    pub corner: Option<KitAsset>,
    pub door_variants: Vec<KitWeightedAsset>,
    /// How many planes above the walls the roof pieces (loc kinds 12 to 21) go. The map stores
    /// a ground-floor building's roof on the next plane up, so a kit for real objects says 1;
    /// 0 keeps the generator's own plane.
    pub roof_plane_offset: u8,
    /// A hip roof built from the footprint (the real client's edge ring, slope ring and flat top);
    /// used instead of `roof` when both are given.
    pub roof_hip: Option<KitHipRoof>,
    /// A roof solved from patterns with sockets; needed when there is no `roof_hip`.
    pub roof: Option<KitRoof>,
}

/// The two objects a hip roof uses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KitHipRoof {
    /// Slope and flat top (loc kinds 12, 15, 16, 17), e.g. `roof_redslate`.
    pub slope_object: u32,
    /// Edge trim (loc kinds 18, 19, 20), e.g. `roofedge_redslate`.
    pub edge_object: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KitError {
    Format(String),
    Version(u32),
    EmptyName,
    Value { what: &'static str, value: u32 },
    Roof(String),
}

impl fmt::Display for KitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Format(found) => write!(f, "not a building kit (format {found:?})"),
            Self::Version(version) => write!(f, "building kit version {version} is not supported"),
            Self::EmptyName => write!(f, "a building kit needs a name"),
            Self::Value { what, value } => write!(f, "{what} {value} is not valid"),
            Self::Roof(message) => write!(f, "roof: {message}"),
        }
    }
}

impl std::error::Error for KitError {}

/// A kit turned into the types the generator takes.
#[derive(Debug, Clone)]
pub struct BuildingKit {
    name: String,
    detail: BuildingDetailPalette,
    hip: Option<HipRoofAssets>,
    patterns: Option<PatternRoof>,
    roof_plane_offset: u8,
}

#[derive(Debug, Clone)]
struct PatternRoof {
    catalog: WfcCatalog,
    patterns: RoofPatternSet,
    palette: RoofPalette,
    junctions: RoofJunctionPatternPalette,
}

/// Why a kit could not make a building.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KitGenerateError {
    Generation(BuildingGenerationError),
    /// The roof would be above the top plane.
    RoofPlane {
        plane: u8,
    },
}

fn kind(value: u8) -> Result<LocKind, KitError> {
    LocKind::new(value).ok_or(KitError::Value {
        what: "object kind",
        value: u32::from(value),
    })
}

fn orientations(values: [u8; 4]) -> Result<[ObjectOrientation; 4], KitError> {
    let mut out = [ObjectOrientation::new(0).unwrap_or_else(|| unreachable!()); 4];
    for (slot, value) in out.iter_mut().zip(values) {
        *slot = ObjectOrientation::new(value).ok_or(KitError::Value {
            what: "orientation",
            value: u32::from(value),
        })?;
    }
    Ok(out)
}

fn edge(asset: &KitAsset) -> Result<EdgeAsset, KitError> {
    Ok(EdgeAsset {
        object_id: ObjectId::new(asset.id),
        kind: kind(asset.kind)?,
        orientations: orientations(asset.orientations)?,
    })
}

fn pattern_id(value: u8) -> Result<WfcPatternId, KitError> {
    WfcPatternId::new(value).ok_or(KitError::Value {
        what: "roof pattern",
        value: u32::from(value),
    })
}

fn domain(values: &[u8], count: usize) -> Result<PatternDomain, KitError> {
    let mut ids = Vec::with_capacity(values.len());
    for &value in values {
        if usize::from(value) >= count {
            return Err(KitError::Roof(format!(
                "pattern {value} is used but only {count} patterns are listed"
            )));
        }
        ids.push(pattern_id(value)?);
    }
    Ok(PatternDomain::from_patterns(ids))
}

fn role(word: &str) -> Result<RoofSectionJunctionRepairRole, KitError> {
    Ok(match word {
        "valley" => RoofSectionJunctionRepairRole::Valley,
        "crest" => RoofSectionJunctionRepairRole::Crest,
        "ridge_intersection" => RoofSectionJunctionRepairRole::RidgeIntersection,
        "cross_slope_intersection" => RoofSectionJunctionRepairRole::CrossSlopeIntersection,
        other => return Err(KitError::Roof(format!("unknown junction role {other:?}"))),
    })
}

fn direction(word: &str) -> Result<CardinalDirection, KitError> {
    Ok(match word {
        "north" => CardinalDirection::North,
        "east" => CardinalDirection::East,
        "south" => CardinalDirection::South,
        "west" => CardinalDirection::West,
        other => return Err(KitError::Roof(format!("unknown direction {other:?}"))),
    })
}

impl KitFile {
    /// Check the file and build what the generator needs.
    pub fn compile(&self) -> Result<BuildingKit, KitError> {
        if self.format != KIT_FORMAT {
            return Err(KitError::Format(self.format.clone()));
        }
        if self.version != KIT_VERSION {
            return Err(KitError::Version(self.version));
        }
        if self.name.trim().is_empty() {
            return Err(KitError::EmptyName);
        }
        let overlay = match &self.floor.overlay {
            None => None,
            Some(overlay) => Some(TerrainOverlay {
                id: FloorOverlayId::new(overlay.id),
                shape: OverlayShape::new(overlay.shape).ok_or(KitError::Value {
                    what: "overlay shape",
                    value: u32::from(overlay.shape),
                })?,
                rotation: OverlayRotation::new(overlay.rotation).ok_or(KitError::Value {
                    what: "overlay rotation",
                    value: u32::from(overlay.rotation),
                })?,
            }),
        };
        let mut door_variants = Vec::with_capacity(self.door_variants.len());
        for variant in &self.door_variants {
            door_variants.push(WeightedEdgeAsset {
                asset: edge(&variant.asset)?,
                weight: variant.weight,
            });
        }
        let detail = BuildingDetailPalette {
            base: BuildingPalette {
                floor: FloorMaterial {
                    underlay: self.floor.underlay.map(FloorUnderlayId::new),
                    overlay,
                },
                exterior_wall: edge(&self.exterior_wall)?,
                interior_wall: edge(&self.interior_wall)?,
                door: edge(&self.door)?,
                window: self.window.as_ref().map(edge).transpose()?,
            },
            exterior_corner: self
                .corner
                .as_ref()
                .map(|corner| {
                    Ok::<_, KitError>(CornerAsset {
                        object_id: ObjectId::new(corner.id),
                        kind: kind(corner.kind)?,
                        orientations: orientations(corner.orientations)?,
                    })
                })
                .transpose()?,
            door_variants,
        };

        let hip = self.roof_hip.as_ref().map(|hip| HipRoofAssets {
            slope: ObjectId::new(hip.slope_object),
            edge: ObjectId::new(hip.edge_object),
        });
        let patterns = match (&self.roof, hip) {
            (Some(roof), _) => Some(compile_pattern_roof(roof)?),
            (None, Some(_)) => None,
            (None, None) => {
                return Err(KitError::Roof(
                    "a kit needs a roof: roof_hip or roof".to_owned(),
                ));
            }
        };

        Ok(BuildingKit {
            name: self.name.clone(),
            detail,
            hip,
            patterns,
            roof_plane_offset: self.roof_plane_offset,
        })
    }
}

fn compile_pattern_roof(roof: &KitRoof) -> Result<PatternRoof, KitError> {
    let count = roof.patterns.len();
    let catalog = WfcCatalog::new(roof.patterns.iter().map(|pattern| WfcPatternSpec {
        weight: pattern.weight,
        sockets: pattern.sockets.map(WfcSocket),
    }))
    .map_err(|error| KitError::Roof(format!("{error:?}")))?;
    let mut entries = Vec::with_capacity(count);
    for (index, pattern) in roof.patterns.iter().enumerate() {
        entries.push(RoofPaletteEntry {
            pattern: pattern_id(u8::try_from(index).map_err(|_| {
                KitError::Roof("a kit holds at most 128 roof patterns".to_owned())
            })?)?,
            asset: RoofAsset {
                object_id: ObjectId::new(pattern.id),
                kind: kind(pattern.kind)?,
                orientation: ObjectOrientation::new(pattern.orientation).ok_or(
                    KitError::Value {
                        what: "orientation",
                        value: u32::from(pattern.orientation),
                    },
                )?,
            },
        });
    }
    let palette =
        RoofPalette::new(entries).map_err(|error| KitError::Roof(format!("{error:?}")))?;
    let patterns = RoofPatternSet {
        ridge: domain(&roof.ridge, count)?,
        slopes: [
            domain(&roof.slopes[0], count)?,
            domain(&roof.slopes[1], count)?,
            domain(&roof.slopes[2], count)?,
            domain(&roof.slopes[3], count)?,
        ],
        eaves: [
            domain(&roof.eaves[0], count)?,
            domain(&roof.eaves[1], count)?,
            domain(&roof.eaves[2], count)?,
            domain(&roof.eaves[3], count)?,
        ],
    };
    let mut junction_entries = Vec::with_capacity(roof.junctions.len());
    for junction in &roof.junctions {
        let mut requirements = Vec::with_capacity(junction.requires.len());
        for (word, toward) in &junction.requires {
            requirements.push(RoofJunctionPatternRequirement {
                role: role(word)?,
                direction_to_other: direction(toward)?,
            });
        }
        if usize::from(junction.pattern) >= count {
            return Err(KitError::Roof(format!(
                "junction pattern {} is not listed",
                junction.pattern
            )));
        }
        junction_entries.push(RoofJunctionPatternPaletteEntry {
            key: RoofJunctionPatternKey::new(requirements),
            pattern: pattern_id(junction.pattern)?,
        });
    }
    let junctions = RoofJunctionPatternPalette::new(junction_entries)
        .map_err(|error| KitError::Roof(format!("{error:?}")))?;

    Ok(PatternRoof {
        catalog,
        patterns,
        palette,
        junctions,
    })
}

impl BuildingKit {
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Generate one building for the request in `snapshot`: rooms sized to the site, walls, doors
    /// and windows from the kit, and its roof. `variation` re-rolls the windows and doors without
    /// moving rooms or corridors.
    pub fn generate(
        &self,
        snapshot: &GenerationSnapshot,
        variation: u32,
    ) -> Result<MapPatch, KitGenerateError> {
        let wrap = KitGenerateError::Generation;
        let request = snapshot.request();
        if request.floors != 1 {
            return Err(wrap(BuildingGenerationError::UnsupportedFloorCount {
                floors: request.floors,
            }));
        }
        let site = request.site;
        let interior =
            u32::from(site.width().saturating_sub(2)) * u32::from(site.height().saturating_sub(2));
        let program = building_program(request.archetype).fitted_to_area(interior);
        let layout = pack_building_program(&program, site, request.seed)
            .map_err(|e| wrap(BuildingGenerationError::Layout(e)))?;
        let building = build_bsp_building_plan(request, &program, &layout)
            .map_err(|e| wrap(BuildingGenerationError::Topology(e)))?;
        route_program_connections(&building)
            .map_err(|e| wrap(BuildingGenerationError::Routing(e)))?;
        let plan: &BuildingPlan = &building.plan;

        if let (Some(hip), None) = (self.hip, &self.patterns) {
            return self.with_hip_roof(snapshot, plan, hip, variation);
        }
        let Some(roof) = &self.patterns else {
            // `compile` guarantees one of the two roofs.
            return Err(wrap(BuildingGenerationError::UnsupportedFloorCount {
                floors: 0,
            }));
        };
        let patch = generate_building_plan_patch(
            snapshot,
            plan,
            BuildingGenerationConfig {
                detail_palette: &self.detail,
                facade_variation: variation,
                facade_config: FacadeWfcConfig::default(),
                facade_policy: FacadeRefinementPolicy::default(),
                roof_catalog: &roof.catalog,
                roof_patterns: roof.patterns,
                roof_palette: &roof.palette,
                roof_junction_patterns: &roof.junctions,
                roof_max_backtracks: 64,
            },
        )
        .map_err(wrap)?;
        Ok(self.lift_roof(patch))
    }

    /// The walls and floor from the facade pipeline, then a hip roof over the footprint.
    fn with_hip_roof(
        &self,
        snapshot: &GenerationSnapshot,
        plan: &BuildingPlan,
        hip: HipRoofAssets,
        variation: u32,
    ) -> Result<MapPatch, KitGenerateError> {
        let wrap = KitGenerateError::Generation;
        let architecture = derive_building_architecture(plan)
            .map_err(|e| wrap(BuildingGenerationError::Architecture(e)))?;
        let body = build_detailed_wfc_building_variation_patch(
            snapshot,
            plan,
            &self.detail,
            variation,
            FacadeWfcConfig::default(),
            FacadeRefinementPolicy::default(),
        )
        .map_err(|e| wrap(BuildingGenerationError::Detail(e)))?;
        let base = plan.request.base_plane.index().get();
        let plane = base.saturating_add(self.roof_plane_offset);
        let plane = osrs_core::coords::SourcePlane::new(plane)
            .ok_or(KitGenerateError::RoofPlane { plane })?;
        let mut ops: Vec<MapOp> = body.ops().to_vec();
        ops.extend(
            hip_roof_placements(architecture.floor_tiles(), hip, plane)
                .into_iter()
                .map(|placement| MapOp::AddObject { placement }),
        );
        Ok(MapPatch::new(ops))
    }

    /// Move roof pieces up by the kit's roof plane offset.
    fn lift_roof(&self, patch: MapPatch) -> MapPatch {
        if self.roof_plane_offset == 0 {
            return patch;
        }
        let mut ops = Vec::with_capacity(patch.ops().len());
        for op in patch.ops() {
            ops.push(match *op {
                MapOp::AddObject { mut placement } if (12..=21).contains(&placement.kind.get()) => {
                    let plane = placement
                        .position
                        .plane
                        .index()
                        .get()
                        .saturating_add(self.roof_plane_offset);
                    // Above the top plane there is nowhere to put a roof: keep it where it is.
                    if let Some(plane) = osrs_core::coords::SourcePlane::new(plane) {
                        placement.position.plane = plane;
                    }
                    MapOp::AddObject { placement }
                }
                other => other,
            });
        }
        MapPatch::new(ops)
    }
}

#[cfg(test)]
mod tests;
