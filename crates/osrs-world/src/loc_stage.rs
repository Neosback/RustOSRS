//! Initial loc placement stage: a port of `DynamicObject.method2054` and `FriendSystem.addObjects`.
//!
//! For every decoded location (in the client's region and stream order) this stage samples the
//! terrain height, plans the placement, builds the object's entities (selection, mirroring,
//! transforms, exact loc lighting or retained `ModelData`, ground contouring), inserts the
//! placement into the scene, and writes the `clipped` shadow grid the terrain builder consumes.
//!
//! Collision maps, minimap/occlusion flags, and sound registration are deliberately not modeled:
//! no renderer consumes them.

use crate::{
    LoadedWindow, WorldDefinitions, WorldError,
    animation::{AnimatedContour, AnimatedModel},
};
use osrs_core::{
    contour::{ContourGroundInput, contour_ground_copy, contour_model_data_copy, model_xz_radius},
    coords::{SceneTile, StoragePlane},
    definitions::{LocType, ObjectDefinition},
    dynamic_model::{DynamicModelCache, DynamicModelKey, DynamicModelTerrain},
    ids::ObjectId,
    lighting::ReferenceLitModel,
    model::WorkingModel,
    morph::select_morph_target,
    static_entity::{InitialStaticEntity, InitialStaticEntityCache, SceneLocalModelDataEntity},
};
use osrs_scene::{
    SceneGrid, SceneModelDataId, SceneReferenceFinalizer,
    placement::{ModelRequest, PlacementInput, PlacementKind, PlacementPlan, plan_placement},
    placement_height::{PlacementHeightInput, sample_placement_height},
    terrain_load::{REFERENCE_SCENE_TILES, TerrainLoadGrid},
};
use std::{collections::HashMap, sync::Arc};

/// Value a clipped wall writes into the shadow grid.
const WALL_SHADOW: u8 = 50;
/// Default shadow value of a clipped game object whose entity is not a lit `Model`.
const DEFAULT_OBJECT_SHADOW: u8 = 15;
const MAX_OBJECT_SHADOW: i32 = 30;

/// What one model request of a placed loc resolved to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocRenderable {
    /// Index into [`LocStageOutput::lit`]: a final reference-lit model.
    Lit(usize),
    /// Scene `ModelData` registered with the finalizer; lit after normal reconciliation.
    ModelData(SceneModelDataId),
    /// Index into [`LocStageOutput::animated`]: an animated `DynamicObject` posed per frame.
    Animated(usize),
    /// A placed object that draws nothing (null morph target, dropped duplicate).
    Omitted,
}

/// One placed location.
#[derive(Debug, Clone)]
pub struct WorldLoc {
    pub object_id: ObjectId,
    pub plane: StoragePlane,
    pub tile: SceneTile,
    pub loc_type: u8,
    pub orientation: u8,
    pub plan: PlacementPlan,
    /// One entry per model request in the plan (two for type 2/8), in plan order. Empty when the
    /// object has no model for the requested type (nothing is placed in that case).
    pub renderables: Vec<LocRenderable>,
}

/// Counters describing a loc stage run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LocStageStats {
    pub decoded: usize,
    pub outside_scene: usize,
    pub missing_definition: usize,
    pub no_model: usize,
    pub rejected_by_capacity: usize,
    pub placed: usize,
    pub dynamic: usize,
    pub replaced: usize,
}

/// Everything the loc stage produced.
pub struct LocStageOutput {
    /// Finalizer holding the surviving `ModelData` entities; call `reconcile_and_light` after the
    /// terrain stage.
    pub finalizer: SceneReferenceFinalizer,
    /// Final lit models for flat-shaded entities, indexed by [`LocRenderable::Lit`].
    pub lit: Vec<ReferenceLitModel>,
    /// Animated models indexed by [`LocRenderable::Animated`].
    pub animated: Vec<Arc<AnimatedModel>>,
    pub locs: Vec<WorldLoc>,
    pub stats: LocStageStats,
}

/// What a `DynamicObject` loc currently draws.
enum DynamicEntity {
    /// No playable legacy animation: the (contoured) lit model, drawn statically.
    Static(Box<ReferenceLitModel>),
    Animated(Arc<AnimatedModel>),
    Hidden,
}

enum BuiltEntity {
    Lit(Box<ReferenceLitModel>),
    Data(SceneLocalModelDataEntity),
    Dynamic(DynamicEntity),
}

/// Per-window dynamic-object state: lit-base cache and shared base models.
#[derive(Default)]
struct DynamicState {
    cache: DynamicModelCache,
    bases: HashMap<DynamicModelKey, Arc<ReferenceLitModel>>,
}

struct PendingLoc {
    object_id: ObjectId,
    plane: StoragePlane,
    tile: SceneTile,
    loc_type: u8,
    orientation: u8,
    plan: PlacementPlan,
    entities: Vec<BuiltEntity>,
    inserted: bool,
}

/// Place every location of `loaded` into `scene`, writing shadow input into `loaded.grid`.
pub fn place_window_locs(
    definitions: &mut WorldDefinitions,
    loaded: &mut LoadedWindow,
    scene: &mut SceneGrid,
) -> Result<LocStageOutput, WorldError> {
    let window = loaded.window;
    let plane_heights: Vec<Arc<Vec<Vec<i32>>>> = (0..4)
        .map(|plane| Arc::new(loaded.grid.plane_heights(plane)))
        .collect();
    let mut dynamic = DynamicState::default();
    let mut cache = InitialStaticEntityCache::new();
    let mut stats = LocStageStats::default();
    let mut pending: Vec<PendingLoc> = Vec::new();

    let locations = std::mem::take(&mut loaded.locations);
    for (region, locs) in &locations {
        let origin = window.region_scene_origin(*region);
        for loc in locs.locations() {
            stats.decoded += 1;
            let tile_x = origin.0 + i32::from(loc.tile.x());
            let tile_y = origin.1 + i32::from(loc.tile.y());
            // `x > 0 && y > 0 && x < tileHeights.length - 2 && y < ...`
            let limit = REFERENCE_SCENE_TILES as i32 - 1;
            if tile_x <= 0 || tile_y <= 0 || tile_x >= limit || tile_y >= limit {
                stats.outside_scene += 1;
                continue;
            }
            let plane = StoragePlane::new(loc.source_plane.index().get())
                .ok_or_else(|| WorldError::Placement("source plane out of range".into()))?;
            let tile = SceneTile::new(tile_x as u32, tile_y as u32);

            let Some(definition) = definitions.object(loc.object_id)? else {
                stats.missing_definition += 1;
                continue;
            };

            let heights = &plane_heights[usize::from(plane.index().get())];
            let sampled_height = sample_placement_height(
                heights,
                REFERENCE_SCENE_TILES as u32,
                REFERENCE_SCENE_TILES as u32,
                PlacementHeightInput {
                    tile,
                    size_x: definition.size_x,
                    size_y: definition.size_y,
                    orientation: loc.orientation,
                },
            )
            .map_err(|error| WorldError::Placement(error.to_string()))?;

            let existing_wall_displacement = match scene.boundary(plane, tile) {
                Some(existing) => definitions
                    .object(existing.object_id())?
                    .map(|existing_definition| existing_definition.decoration_displacement)
                    .or(Some(16)),
                None => None,
            };

            let plan = plan_placement(PlacementInput {
                loc_type: LocType::new(loc.loc_type),
                orientation: loc.orientation,
                tile,
                size_x: definition.size_x,
                size_y: definition.size_y,
                sampled_height,
                existing_wall_displacement,
            })
            .map_err(|error| WorldError::Placement(error.to_string()))?;

            let requests = model_requests(&plan.kind);
            let mut entities = Vec::with_capacity(requests.len());
            let is_dynamic = definition.animation.is_some() || definition.morphs.is_some();
            for request in &requests {
                let entity = if is_dynamic {
                    Some(BuiltEntity::Dynamic(build_dynamic(
                        definitions,
                        &mut dynamic,
                        &definition,
                        *request,
                        heights,
                        &plan,
                        sampled_height,
                    )?))
                } else {
                    build_entity(
                        definitions,
                        &mut cache,
                        &definition,
                        *request,
                        heights.as_slice(),
                        &plan,
                        sampled_height,
                    )?
                };
                if let Some(entity) = entity {
                    entities.push(entity);
                }
            }
            if entities.is_empty() {
                stats.no_model += 1;
            }
            if is_dynamic {
                stats.dynamic += 1;
            }

            // The client only creates scene objects when the primary renderable exists.
            let creates_object = match plan.kind {
                PlacementKind::Boundary(_) => !entities.is_empty(),
                PlacementKind::FloorDecoration(_)
                | PlacementKind::WallDecoration(_)
                | PlacementKind::GameObject(_) => !entities.is_empty(),
            };
            let inserted = if creates_object {
                let inserted = scene
                    .insert_placement(plane, loc.object_id, plan)
                    .map_err(WorldError::Scene)?;
                if !inserted {
                    stats.rejected_by_capacity += 1;
                }
                inserted
            } else {
                false
            };

            write_shadow(
                &mut loaded.grid,
                &plan,
                plane,
                tile,
                &definition,
                inserted,
                entities.first(),
            );

            pending.push(PendingLoc {
                object_id: loc.object_id,
                plane,
                tile,
                loc_type: loc.loc_type,
                orientation: loc.orientation,
                plan,
                entities,
                inserted,
            });
        }
    }
    loaded.locations = locations;

    finish(pending, scene, &loaded.grid, stats)
}

fn model_requests(kind: &PlacementKind) -> Vec<ModelRequest> {
    match kind {
        PlacementKind::FloorDecoration(plan) => vec![plan.model],
        PlacementKind::Boundary(plan) => [Some(plan.primary), plan.secondary]
            .into_iter()
            .flatten()
            .collect(),
        PlacementKind::WallDecoration(plan) => [Some(plan.primary), plan.secondary]
            .into_iter()
            .flatten()
            .collect(),
        PlacementKind::GameObject(plan) => vec![plan.model],
    }
}

/// Default-state object for a morphing definition: every variable reads zero.
fn effective_definition(
    definitions: &mut WorldDefinitions,
    original: &Arc<ObjectDefinition>,
) -> Result<Option<Arc<ObjectDefinition>>, WorldError> {
    let Some(morphs) = &original.morphs else {
        return Ok(Some(original.clone()));
    };
    let selector = if morphs.transform_varbit.is_some() || morphs.transform_varp.is_some() {
        0
    } else {
        -1
    };
    match select_morph_target(morphs, selector) {
        Some(id) => definitions.object(id),
        None => Ok(None),
    }
}

/// `DynamicObject.getModel`: the transformed definition's lit model, posed per frame when the
/// loc's own sequence has legacy frames.
fn build_dynamic(
    definitions: &mut WorldDefinitions,
    state: &mut DynamicState,
    original: &Arc<ObjectDefinition>,
    request: ModelRequest,
    heights: &Arc<Vec<Vec<i32>>>,
    plan: &PlacementPlan,
    sampled_height: i32,
) -> Result<DynamicEntity, WorldError> {
    let Some(effective) = effective_definition(definitions, original)? else {
        return Ok(DynamicEntity::Hidden);
    };
    let sequence = match original.animation {
        Some(id) => definitions.sequence(id)?,
        None => None,
    };
    let frames = match &sequence {
        Some(sequence) => definitions.legacy_frames(sequence)?,
        None => None,
    };
    let terrain = DynamicModelTerrain {
        heights: heights.as_slice(),
        origin_x: plan.model_center.x.units(),
        base_height: sampled_height,
        origin_z: plan.model_center.z.units(),
    };

    let mut resolve_error: Option<WorldError> = None;
    let mut builder = |definitions: &mut WorldDefinitions| match definitions.resolve_model(
        &effective,
        request.loc_type,
        request.orientation,
    ) {
        Ok(Some(assembled)) => match WorkingModel::from_assembled(&assembled) {
            Ok(model) => Some(model),
            Err(error) => {
                resolve_error = Some(WorldError::Placement(error.to_string()));
                None
            }
        },
        Ok(None) => None,
        Err(error) => {
            resolve_error = Some(error);
            None
        }
    };

    let (Some(sequence), Some(frames)) = (sequence, frames) else {
        let built = state
            .cache
            .get_or_build_legacy(
                &effective,
                request.loc_type,
                request.orientation,
                None,
                Some(terrain),
                || builder(definitions),
            )
            .map_err(|error| WorldError::Placement(error.to_string()))?
            .map(std::borrow::Cow::into_owned);
        if let Some(error) = resolve_error {
            return Err(error);
        }
        return Ok(match built {
            Some(model) => DynamicEntity::Static(Box::new(model)),
            None => DynamicEntity::Hidden,
        });
    };

    // Animated: cache the lit base only; posing and contouring happen per frame at runtime.
    let present = state
        .cache
        .get_or_build_legacy(
            &effective,
            request.loc_type,
            request.orientation,
            None,
            None,
            || builder(definitions),
        )
        .map_err(|error| WorldError::Placement(error.to_string()))?
        .is_some();
    if let Some(error) = resolve_error {
        return Err(error);
    }
    if !present {
        return Ok(DynamicEntity::Hidden);
    }
    let key = DynamicModelKey::for_object(&effective, request.loc_type, request.orientation);
    let base = match state.bases.get(&key) {
        Some(base) => base.clone(),
        None => {
            let Some(base) = state.cache.cached_base(key) else {
                return Ok(DynamicEntity::Hidden);
            };
            let base = Arc::new(base.clone());
            state.bases.insert(key, base.clone());
            base
        }
    };
    let contour = effective
        .contour_clip
        .and_then(|clip| i32::try_from(clip).ok())
        .map(|clip| AnimatedContour {
            clip,
            heights: heights.clone(),
            origin_x: plan.model_center.x.units(),
            base_height: sampled_height,
            origin_z: plan.model_center.z.units(),
        });
    Ok(DynamicEntity::Animated(Arc::new(AnimatedModel {
        base,
        sequence,
        frames,
        orientation: request.orientation,
        contour,
    })))
}

/// `ObjectComposition.getEntity`: cached lit model or scene-local `ModelData`, then contouring.
fn build_entity(
    definitions: &mut WorldDefinitions,
    cache: &mut InitialStaticEntityCache,
    definition: &ObjectDefinition,
    request: ModelRequest,
    heights: &[Vec<i32>],
    plan: &PlacementPlan,
    sampled_height: i32,
) -> Result<Option<BuiltEntity>, WorldError> {
    let mut resolve_error: Option<WorldError> = None;
    let entity = cache
        .get_or_build(
            definition,
            request.loc_type,
            request.orientation,
            || match definitions.resolve_model(definition, request.loc_type, request.orientation) {
                Ok(Some(assembled)) => match WorkingModel::from_assembled(&assembled) {
                    Ok(model) => Some(model),
                    Err(error) => {
                        resolve_error = Some(WorldError::Placement(error.to_string()));
                        None
                    }
                },
                Ok(None) => None,
                Err(error) => {
                    resolve_error = Some(error);
                    None
                }
            },
        )
        .map_err(WorldError::Lighting)?;
    if let Some(error) = resolve_error {
        return Err(error);
    }
    let Some(entity) = entity else {
        return Ok(None);
    };

    let contour = definition.contour_clip.map(|clip| ContourGroundInput {
        heights,
        origin_x: plan.model_center.x.units(),
        base_height: sampled_height,
        origin_z: plan.model_center.z.units(),
        clip: clip as i32,
    });

    Ok(Some(match entity {
        InitialStaticEntity::Lit(lit) => match contour {
            Some(input) => BuiltEntity::Lit(Box::new(
                contour_ground_copy(&lit, input)
                    .map_err(WorldError::Contour)?
                    .into_owned(),
            )),
            None => BuiltEntity::Lit(lit),
        },
        InitialStaticEntity::ModelData(data) => match contour {
            Some(input) => {
                let lighting = data.lighting();
                let contoured = contour_model_data_copy(data.model(), input)
                    .map_err(WorldError::Contour)?
                    .into_owned();
                BuiltEntity::Data(SceneLocalModelDataEntity::from_parts(contoured, lighting))
            }
            None => BuiltEntity::Data(data),
        },
    }))
}

/// `Tiles.Tiles_underlays2` writes performed by `addObjects` for `clipped` definitions.
fn write_shadow(
    grid: &mut TerrainLoadGrid,
    plan: &PlacementPlan,
    plane: StoragePlane,
    tile: SceneTile,
    definition: &ObjectDefinition,
    inserted: bool,
    first_entity: Option<&BuiltEntity>,
) {
    if !definition.placement.clipped {
        return;
    }
    let plane = usize::from(plane.index().get());
    let (x, y) = (tile.x as usize, tile.y as usize);
    let orientation = plan.source_orientation;
    match plan.kind {
        PlacementKind::Boundary(_) => match plan.source_loc_type.get() {
            0 => {
                let cells: [(usize, usize); 2] = match orientation {
                    0 => [(x, y), (x, y + 1)],
                    1 => [(x, y + 1), (x + 1, y + 1)],
                    2 => [(x + 1, y), (x + 1, y + 1)],
                    _ => [(x, y), (x + 1, y)],
                };
                for (cx, cy) in cells {
                    grid.set_shadow(plane, cx, cy, WALL_SHADOW);
                }
            }
            1 | 3 => {
                let (cx, cy) = match orientation {
                    0 => (x, y + 1),
                    1 => (x + 1, y + 1),
                    2 => (x + 1, y),
                    _ => (x, y),
                };
                grid.set_shadow(plane, cx, cy, WALL_SHADOW);
            }
            _ => {}
        },
        PlacementKind::GameObject(game)
            if plan.source_loc_type.get() == 10 || plan.source_loc_type.get() == 11 =>
        {
            if !inserted {
                return;
            }
            let value = match first_entity {
                Some(BuiltEntity::Lit(lit)) => {
                    let radius = (model_xz_radius(lit) / 4).min(MAX_OBJECT_SHADOW);
                    radius.clamp(0, MAX_OBJECT_SHADOW) as u8
                }
                _ => DEFAULT_OBJECT_SHADOW,
            };
            let footprint = game.storage_footprint;
            for dx in 0..=usize::from(footprint.width) {
                for dy in 0..=usize::from(footprint.depth) {
                    grid.raise_shadow(plane, x + dx, y + dy, value);
                }
            }
        }
        _ => {}
    }
}

/// Resolve slot replacement, then register the surviving `ModelData` entities with the finalizer.
fn finish(
    pending: Vec<PendingLoc>,
    scene: &SceneGrid,
    grid: &TerrainLoadGrid,
    mut stats: LocStageStats,
) -> Result<LocStageOutput, WorldError> {
    // Boundary and floor-decoration slots keep only their last occupant (the client overwrites).
    let mut boundary_owner: HashMap<(u8, u32, u32), usize> = HashMap::new();
    let mut floor_owner: HashMap<(u8, u32, u32), usize> = HashMap::new();
    for (index, loc) in pending.iter().enumerate() {
        if !loc.inserted {
            continue;
        }
        let key = (loc.plane.index().get(), loc.tile.x, loc.tile.y);
        let previous = match loc.plan.kind {
            PlacementKind::Boundary(_) => boundary_owner.insert(key, index),
            PlacementKind::FloorDecoration(_) => floor_owner.insert(key, index),
            _ => None,
        };
        if previous.is_some() {
            stats.replaced += 1;
        }
    }

    let mut finalizer =
        SceneReferenceFinalizer::new(scene.width(), scene.height(), scene.plane_count())
            .map_err(|error| WorldError::Placement(error.to_string()))?;
    for plane in 0..4_usize {
        let storage =
            StoragePlane::new(plane as u8).ok_or_else(|| WorldError::Placement("plane".into()))?;
        for x in 0..=grid.size_x() {
            for y in 0..=grid.size_y() {
                finalizer
                    .set_height_corner(storage, x as u32, y as u32, grid.height(plane, x, y))
                    .map_err(|error| WorldError::Placement(error.to_string()))?;
            }
        }
    }

    let mut lit = Vec::new();
    let mut animated: Vec<Arc<AnimatedModel>> = Vec::new();
    let mut locs = Vec::with_capacity(pending.len());
    for (index, loc) in pending.into_iter().enumerate() {
        let key = (loc.plane.index().get(), loc.tile.x, loc.tile.y);
        let survives = loc.inserted
            && match loc.plan.kind {
                PlacementKind::Boundary(_) => boundary_owner.get(&key) == Some(&index),
                PlacementKind::FloorDecoration(_) => floor_owner.get(&key) == Some(&index),
                _ => true,
            };

        let mut renderables = Vec::with_capacity(loc.entities.len());
        for entity in loc.entities {
            renderables.push(match entity {
                BuiltEntity::Dynamic(DynamicEntity::Static(model)) => {
                    lit.push(*model);
                    LocRenderable::Lit(lit.len() - 1)
                }
                BuiltEntity::Dynamic(DynamicEntity::Animated(model)) => {
                    animated.push(model);
                    LocRenderable::Animated(animated.len() - 1)
                }
                BuiltEntity::Dynamic(DynamicEntity::Hidden) => LocRenderable::Omitted,
                BuiltEntity::Lit(model) => {
                    lit.push(*model);
                    LocRenderable::Lit(lit.len() - 1)
                }
                BuiltEntity::Data(data) => {
                    if survives {
                        LocRenderable::ModelData(finalizer.add_initial_model_data(data))
                    } else {
                        // A replaced or rejected ModelData entity never reaches finalization.
                        LocRenderable::Omitted
                    }
                }
            });
        }
        if !survives {
            renderables.clear();
        } else {
            register_with_finalizer(&mut finalizer, &loc.plan, loc.plane, loc.tile, &renderables)?;
            stats.placed += 1;
        }

        locs.push(WorldLoc {
            object_id: loc.object_id,
            plane: loc.plane,
            tile: loc.tile,
            loc_type: loc.loc_type,
            orientation: loc.orientation,
            plan: loc.plan,
            renderables,
        });
    }

    Ok(LocStageOutput {
        finalizer,
        lit,
        animated,
        locs,
        stats,
    })
}

fn register_with_finalizer(
    finalizer: &mut SceneReferenceFinalizer,
    plan: &PlacementPlan,
    plane: StoragePlane,
    tile: SceneTile,
    renderables: &[LocRenderable],
) -> Result<(), WorldError> {
    let data_id = |renderable: Option<&LocRenderable>| match renderable {
        Some(LocRenderable::ModelData(id)) => Some(*id),
        _ => None,
    };
    let map = |error: osrs_scene::SceneNormalError| WorldError::Placement(error.to_string());
    match plan.kind {
        PlacementKind::Boundary(_) => {
            if let Some(primary) = data_id(renderables.first()) {
                finalizer
                    .set_boundary(plane, tile, primary, data_id(renderables.get(1)))
                    .map_err(map)?;
            }
        }
        PlacementKind::FloorDecoration(_) => {
            if let Some(model) = data_id(renderables.first()) {
                finalizer
                    .set_floor_decoration(plane, tile, model)
                    .map_err(map)?;
            }
        }
        PlacementKind::GameObject(game) => {
            if let Some(model) = data_id(renderables.first()) {
                finalizer
                    .insert_game_object(plane, tile, game.storage_footprint, model)
                    .map_err(map)?;
            }
        }
        PlacementKind::WallDecoration(_) => {}
    }
    Ok(())
}
