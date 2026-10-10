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
    if presentation.smooth_terrain {
        smooth_flat_terrain(&mut scene);
    }

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
            // No type-9 wall here (diagonal boundary hosts, or none): keep the client's offsets
            // but still lift each plate off the surface it sits on.
            let Some(decor_def) = definitions.object(loc.object_id)? else {
                continue;
            };
            let mut offsets = [(decor.offset_x, decor.offset_z), (0, 0)];
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
                let points = used_points(&plate);
                for normal in [(k, k), (k, -k), (-k, -k), (-k, k)] {
                    let (lo, hi) = range(&points, normal);
                    if lo.abs() <= 1.5 && hi > 1.5 {
                        offsets[slot].0 += (normal.0 * 1.0).round() as i32;
                        offsets[slot].1 += (normal.1 * 1.0).round() as i32;
                        break;
                    }
                }
            }
            loc.slot_offsets = Some(offsets);
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
        // The host's flat face passes through the tile centre: along one diagonal axis its
        // vertices end at 0. Orient the axis so the outward side is positive.
        let Some((normal, slab_lo)) =
            [(k, k), (k, -k), (-k, -k), (-k, k)]
                .into_iter()
                .find_map(|normal| {
                    let (lo, hi) = range(&host_points, normal);
                    (hi.abs() < 1.0 && lo < -2.0).then_some((normal, lo))
                })
        else {
            continue;
        };
        // Thin slab (a 16-unit wall) has a second, inner face; a wedge's inside is solid.
        let thin = -slab_lo < 24.0;
        // Plates must clear the face they sit on by a hair: the integer 45-degree rotation
        // leaves some plates up to 0.7 units inside the wall, where the depth buffer hides
        // them (the software client paints decorations over their wall regardless of depth).
        const LIFT: f64 = 1.0;

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
                LIFT // protrudes outward: sits on the outer face
            } else if plate_hi.abs() <= 1.5 && plate_lo < -1.5 {
                if thin {
                    slab_lo - LIFT // protrudes inward: sits on the inner face
                } else {
                    // Buried in a solid wedge: keep the client's placement.
                    offsets[slot] = if slot == 0 {
                        (decor.offset_x, decor.offset_z)
                    } else {
                        (0, 0)
                    };
                    continue;
                }
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

/// Average the colour at every vertex shared by untextured flat terrain tiles.
///
/// The client gives each tile its own blended hue/saturation and per-corner lightness, so the
/// colours of the two tiles meeting at a vertex generally differ (about a third of shared
/// vertices differ in hue, over a quarter in lightness). Hue is averaged on the 64-step circle.
fn smooth_flat_terrain(scene: &mut SceneGrid) {
    use osrs_core::coords::{SceneTile, StoragePlane};
    use osrs_scene::terrain::TerrainSurface;

    const SKIPPED: i32 = 12_345_678;
    let (width, height) = (scene.width() as usize, scene.height() as usize);
    for plane_index in 0..4_u8 {
        let Some(plane) = StoragePlane::new(plane_index) else {
            continue;
        };
        // (sum cos, sum sin, sum saturation, sum lightness, count) per vertex.
        let stride = height + 1;
        let mut sums = vec![(0.0_f64, 0.0_f64, 0_i32, 0_i32, 0_i32); (width + 1) * stride];
        let eligible = |scene: &SceneGrid, x: usize, y: usize| match scene
            .tile(plane, SceneTile::new(x as u32, y as u32))
            .and_then(|tile| tile.terrain.as_ref())
        {
            Some(TerrainSurface::Flat(flat))
                if flat.texture_id.is_none() && flat.colors.northeast != SKIPPED =>
            {
                Some(flat.colors)
            }
            _ => None,
        };
        for x in 0..width {
            for y in 0..height {
                let Some(colors) = eligible(scene, x, y) else {
                    continue;
                };
                for (color, vx, vy) in [
                    (colors.southwest, x, y),
                    (colors.southeast, x + 1, y),
                    (colors.northeast, x + 1, y + 1),
                    (colors.northwest, x, y + 1),
                ] {
                    let entry = &mut sums[vx * stride + vy];
                    let angle = f64::from((color >> 10) & 63) / 64.0 * std::f64::consts::TAU;
                    entry.0 += angle.cos();
                    entry.1 += angle.sin();
                    entry.2 += (color >> 7) & 7;
                    entry.3 += color & 127;
                    entry.4 += 1;
                }
            }
        }
        let averaged = |vx: usize, vy: usize, original: i32| {
            let (cos, sin, sat, light, count) = sums[vx * stride + vy];
            if count == 0 {
                return original;
            }
            let hue = if cos.abs() + sin.abs() < 1e-9 {
                (original >> 10) & 63
            } else {
                let turns = sin.atan2(cos) / std::f64::consts::TAU;
                (((turns * 64.0).round() as i32) + 64) & 63
            };
            let saturation = (sat + count / 2) / count;
            let lightness = (light + count / 2) / count;
            (original & !0xFFFF) | (hue << 10) | (saturation << 7) | lightness
        };
        for x in 0..width {
            for y in 0..height {
                if eligible(scene, x, y).is_none() {
                    continue;
                }
                let Some(tile) = scene.tile_mut(plane, SceneTile::new(x as u32, y as u32)) else {
                    continue;
                };
                if let Some(TerrainSurface::Flat(flat)) = tile.terrain.as_mut() {
                    flat.colors.southwest = averaged(x, y, flat.colors.southwest);
                    flat.colors.southeast = averaged(x + 1, y, flat.colors.southeast);
                    flat.colors.northeast = averaged(x + 1, y + 1, flat.colors.northeast);
                    flat.colors.northwest = averaged(x, y + 1, flat.colors.northwest);
                }
            }
        }
    }
}
