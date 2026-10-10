//! Full scene assembly in the reference client's order.
//!
//! terrain streams -> empty-region fill -> loc placement (with shadow input) -> terrain colors
//! and minimum planes -> scene normal reconciliation and final lighting -> bridge link-below.

use crate::{
    LocStageOutput, SceneWindow, TerrainPresentation, WorldDefinitions, WorldError, apply_terrain,
    load_window,
    loc_stage::{LocStageStats, WorldLoc, place_window_locs},
};
use osrs_core::lighting::ReferenceLitModel;
use osrs_scene::{
    SceneGrid, SceneReferenceFinalizer,
    normal_finalization::SceneNormalMergeReport,
    terrain_build::link_bridge_tiles,
    terrain_load::{REFERENCE_SCENE_TILES, TerrainLoadGrid},
};

/// A fully assembled 104x104 scene window.
pub struct WorldScene {
    pub window: SceneWindow,
    /// Loaded height/flag/floor/shadow arrays (the builder's inputs).
    pub load: TerrainLoadGrid,
    pub scene: SceneGrid,
    /// Finalizer with every surviving `ModelData` entity lit.
    pub finalizer: SceneReferenceFinalizer,
    /// Flat-shaded lit models indexed by `LocRenderable::Lit`.
    pub lit: Vec<ReferenceLitModel>,
    pub animated: Vec<std::sync::Arc<crate::AnimatedModel>>,
    pub locs: Vec<WorldLoc>,
    pub loc_stats: LocStageStats,
    pub merge_report: SceneNormalMergeReport,
}

/// Assemble the scene for `window` from the cache.
pub fn build_world_scene(
    definitions: &mut WorldDefinitions,
    window: SceneWindow,
    presentation: TerrainPresentation,
) -> Result<WorldScene, WorldError> {
    let mut loaded = load_window(definitions, window)?;
    let mut scene = SceneGrid::new(
        REFERENCE_SCENE_TILES as u32,
        REFERENCE_SCENE_TILES as u32,
        4,
    )?;

    let LocStageOutput {
        mut finalizer,
        lit,
        animated,
        mut locs,
        stats,
    } = place_window_locs(definitions, &mut loaded, &mut scene)?;

    if presentation.flush_diagonal_decorations {
        snap_diagonal_decorations(definitions, &mut locs)?;
    }
    apply_terrain(definitions, &loaded.grid, presentation, &mut scene)?;
    finalizer.set_vertical_merge_tolerance(presentation.wall_merge_tolerance);
    let merge_report = finalizer
        .reconcile_and_light()
        .map_err(|error| WorldError::Placement(error.to_string()))?;
    link_bridge_tiles(&loaded.grid, &mut scene)?;

    Ok(WorldScene {
        window,
        load: loaded.grid,
        scene,
        finalizer,
        lit,
        animated,
        locs,
        loc_stats: stats,
        merge_report,
    })
}

/// Snap diagonal (`256`) wall decorations flush against the type-9 diagonal wall on their tile.
///
/// A type-9 wall is a thin slab (thickness 16 along the tile diagonal) whose two faces both take
/// a decoration plate. The client's displacement (`8 * (+-1, +-1)` on the first plate, none on the
/// second) only lands both plates on the slab faces when the decoration orientation equals the
/// wall orientation; other combinations leave one plate floating 11 units off a face and the
/// other buried in the slab. Here each plate is moved along the slab normal so that its base
/// plane (the end of its thickness at the origin) coincides with the slab face it faces.
fn snap_diagonal_decorations(
    definitions: &mut WorldDefinitions,
    locs: &mut [WorldLoc],
) -> Result<(), WorldError> {
    use osrs_core::definitions::LocType;
    use osrs_scene::placement::PlacementKind;
    use std::collections::HashMap;

    let mut hosts: HashMap<(u8, u32, u32), (osrs_core::ids::ObjectId, u8)> = HashMap::new();
    for loc in locs.iter() {
        if !loc.renderables.is_empty() && loc.plan.source_loc_type.get() == 9 {
            hosts.insert(
                (loc.plane.index().get(), loc.tile.x, loc.tile.y),
                (loc.object_id, loc.orientation),
            );
        }
    }

    // Vertical-axis-independent (x, z) of the vertices a face references.
    fn used_points(model: &osrs_core::model_construction::AssembledModel) -> Vec<(i32, i32)> {
        let vertices = model.vertices();
        let mut used = vec![false; vertices.len()];
        for face in model.faces() {
            for index in [face.a.get(), face.b.get(), face.c.get()] {
                if let Some(flag) = used.get_mut(index as usize) {
                    *flag = true;
                }
            }
        }
        vertices
            .iter()
            .zip(used)
            .filter(|(_, used)| *used)
            .map(|(vertex, _)| (vertex.x, vertex.z))
            .collect()
    }
    fn range(points: &[(i32, i32)], normal: (f64, f64)) -> (f64, f64) {
        points.iter().fold((f64::MAX, f64::MIN), |(lo, hi), point| {
            let projection = f64::from(point.0) * normal.0 + f64::from(point.1) * normal.1;
            (lo.min(projection), hi.max(projection))
        })
    }

    let k = std::f64::consts::FRAC_1_SQRT_2;
    for loc in locs.iter_mut() {
        let PlacementKind::WallDecoration(decor) = loc.plan.kind else {
            continue;
        };
        if decor.orientation_flag != 256 {
            continue;
        }
        let key = (loc.plane.index().get(), loc.tile.x, loc.tile.y);
        let Some(&(host_id, host_orientation)) = hosts.get(&key) else {
            continue;
        };
        let (Some(host_def), Some(decor_def)) = (
            definitions.object(host_id)?,
            definitions.object(loc.object_id)?,
        ) else {
            continue;
        };
        let Some(host_model) =
            definitions.resolve_model(&host_def, LocType::new(9), host_orientation)?
        else {
            continue;
        };
        let host_points = used_points(&host_model);
        // The slab normal is the diagonal axis along which the wall is thin.
        let Some((normal, (slab_lo, slab_hi))) = [(k, k), (k, -k)]
            .into_iter()
            .map(|normal| (normal, range(&host_points, normal)))
            .find(|(_, (lo, hi))| hi - lo < 24.0)
        else {
            continue;
        };

        let mut offsets = [(0, 0); 2];
        for (slot, request) in [Some(decor.primary), decor.secondary]
            .into_iter()
            .enumerate()
        {
            let Some(request) = request else { continue };
            let Some(plate) =
                definitions.resolve_model(&decor_def, request.loc_type, request.orientation)?
            else {
                continue;
            };
            let (plate_lo, plate_hi) = range(&used_points(&plate), normal);
            // The plate's base plane is the end of its thickness at the origin.
            let delta = if plate_lo.abs() <= 1.5 && plate_hi > 1.5 {
                slab_hi // protrudes toward +normal: sits on the +normal face
            } else if plate_hi.abs() <= 1.5 && plate_lo < -1.5 {
                slab_lo // protrudes toward -normal: sits on the -normal face
            } else {
                continue;
            };
            offsets[slot] = (
                (delta * normal.0).round() as i32,
                (delta * normal.1).round() as i32,
            );
        }
        loc.slot_offsets = Some(offsets);
    }
    Ok(())
}
