//! Viewport picking and copyable diagnostics for tiles and objects.

use crate::camera::FlyCamera;
use osrs_world::{LocInfo, RegionInfo, SurfaceKind, TileInfo};
use std::{collections::HashMap, fmt::Write as _, sync::Arc};

pub type Infos = HashMap<(i32, i32), Arc<RegionInfo>>;

const REGION_UNITS: f64 = 64.0 * 128.0;

/// A picking ray in world local units (128 per tile).
#[derive(Debug, Clone, Copy)]
pub struct Ray {
    pub origin: [f64; 3],
    pub dir: [f64; 3],
}

/// Ray through pixel `(px, py)` of a `width x height` viewport.
///
/// Inverts `scale * projection * rotateX(pitch) * rotateY(yaw) * translate(-camera)`:
/// view-space direction `(ndc_x * w / (2 scale), -ndc_y * h / (2 scale), 1)` rotated back.
pub fn ray_through_pixel(
    camera: &FlyCamera,
    render_origin: (i32, i32),
    width: f32,
    height: f32,
    px: f32,
    py: f32,
) -> Ray {
    let reference = camera.reference(width);
    let ndc_x = f64::from(2.0 * px / width - 1.0);
    let ndc_y = f64::from(1.0 - 2.0 * py / height);
    let scale = f64::from(reference.scale);
    let view = [
        ndc_x * f64::from(width) / (2.0 * scale),
        -ndc_y * f64::from(height) / (2.0 * scale),
        1.0,
    ];
    let (sp, cp) = f64::from(camera.pitch).sin_cos();
    let (sy, cy) = f64::from(camera.yaw).sin_cos();
    // Rx^T
    let after_x = [
        view[0],
        cp * view[1] + sp * view[2],
        -sp * view[1] + cp * view[2],
    ];
    // Ry^T
    let dir = [
        cy * after_x[0] - sy * after_x[2],
        after_x[1],
        sy * after_x[0] + cy * after_x[2],
    ];
    let length = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
    Ray {
        origin: [
            f64::from(render_origin.0) + f64::from(camera.x),
            f64::from(camera.y),
            f64::from(render_origin.1) + f64::from(camera.z),
        ],
        dir: [dir[0] / length, dir[1] / length, dir[2] / length],
    }
}

fn ray_box(ray: &Ray, min: [i32; 3], max: [i32; 3]) -> Option<f64> {
    let mut near = 0.0_f64;
    let mut far = f64::MAX;
    for axis in 0..3 {
        // Pad degenerate boxes so zero-thickness slabs can still be hit.
        let lo = f64::from(min[axis]) - 4.0;
        let hi = f64::from(max[axis]) + 4.0;
        if ray.dir[axis].abs() < 1e-12 {
            if ray.origin[axis] < lo || ray.origin[axis] > hi {
                return None;
            }
            continue;
        }
        let t0 = (lo - ray.origin[axis]) / ray.dir[axis];
        let t1 = (hi - ray.origin[axis]) / ray.dir[axis];
        near = near.max(t0.min(t1));
        far = far.min(t0.max(t1));
        if near > far {
            return None;
        }
    }
    Some(near)
}

fn region_key(x: f64, z: f64) -> (i32, i32) {
    (
        (x / REGION_UNITS).floor() as i32,
        (z / REGION_UNITS).floor() as i32,
    )
}

/// Distance along the ray to the terrain of `plane`, if it is hit.
fn terrain_hit(infos: &Infos, ray: &Ray, plane: usize) -> Option<f64> {
    let height = |t: f64| {
        let x = ray.origin[0] + ray.dir[0] * t;
        let z = ray.origin[2] + ray.dir[2] * t;
        infos
            .get(&region_key(x, z))
            .and_then(|info| info.height_at(plane, x as f32, z as f32))
            .map(f64::from)
    };
    let below = |t: f64| height(t).is_some_and(|ground| ray.origin[1] + ray.dir[1] * t >= ground);
    let mut previous = 0.0;
    let mut t = 8.0;
    while t < 120_000.0 {
        if below(t) {
            let (mut lo, mut hi) = (previous, t);
            for _ in 0..24 {
                let mid = 0.5 * (lo + hi);
                if below(mid) {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            return Some(hi);
        }
        previous = t;
        t += 24.0 + t * 0.01;
    }
    None
}

/// An object under the cursor.
#[derive(Clone)]
pub struct ObjectPick {
    pub region: (i32, i32),
    pub index: usize,
}

/// A terrain tile under the cursor.
#[derive(Clone, Copy)]
pub struct TilePick {
    pub tile: (i32, i32),
}

#[derive(Clone, Default)]
pub struct Pick {
    pub object: Option<ObjectPick>,
    pub tile: Option<TilePick>,
}

/// Find the nearest visible object and the terrain tile along `ray`.
pub fn pick(infos: &Infos, ray: &Ray, view_plane: u8) -> Pick {
    let plane = usize::from(view_plane.min(3));
    let terrain = terrain_hit(infos, ray, plane);
    let tile = terrain.map(|distance| {
        let x = ray.origin[0] + ray.dir[0] * distance;
        let z = ray.origin[2] + ray.dir[2] * distance;
        TilePick {
            tile: ((x / 128.0).floor() as i32, (z / 128.0).floor() as i32),
        }
    });

    let mut best: Option<(f64, f64, ObjectPick)> = None;
    for (key, info) in infos {
        for (index, loc) in info.locs.iter().enumerate() {
            if loc.min_plane > view_plane {
                continue;
            }
            for slot in &loc.slots {
                let Some(distance) = ray_box(ray, slot.min, slot.max) else {
                    continue;
                };
                // Ignore boxes that are only reached behind the terrain.
                if terrain.is_some_and(|ground| distance > ground + 16.0) {
                    continue;
                }
                let volume = f64::from((slot.max[0] - slot.min[0]).max(1))
                    * f64::from((slot.max[1] - slot.min[1]).max(1))
                    * f64::from((slot.max[2] - slot.min[2]).max(1));
                let better = best.as_ref().is_none_or(|(d, v, _)| {
                    distance < *d - 1.0 || ((distance - *d).abs() <= 1.0 && volume < *v)
                });
                if better {
                    best = Some((
                        distance,
                        volume,
                        ObjectPick {
                            region: *key,
                            index,
                        },
                    ));
                }
            }
        }
    }
    Pick {
        object: best.map(|(_, _, pick)| pick),
        tile,
    }
}

/// Project a world point to viewport pixels (`None` behind the camera).
pub fn project(
    camera: &FlyCamera,
    render_origin: (i32, i32),
    width: f32,
    height: f32,
    point: [f64; 3],
) -> Option<[f32; 2]> {
    let reference = camera.reference(width);
    let relative = [
        point[0] - f64::from(render_origin.0) - f64::from(camera.x),
        point[1] - f64::from(camera.y),
        point[2] - f64::from(render_origin.1) - f64::from(camera.z),
    ];
    let (sp, cp) = f64::from(camera.pitch).sin_cos();
    let (sy, cy) = f64::from(camera.yaw).sin_cos();
    // Ry
    let after_y = [
        cy * relative[0] + sy * relative[2],
        relative[1],
        -sy * relative[0] + cy * relative[2],
    ];
    // Rx
    let view = [
        after_y[0],
        cp * after_y[1] - sp * after_y[2],
        sp * after_y[1] + cp * after_y[2],
    ];
    if view[2] < 1.0 {
        return None;
    }
    let scale = f64::from(reference.scale);
    let x = view[0] * scale / view[2] + f64::from(width) / 2.0;
    let y = view[1] * scale / view[2] + f64::from(height) / 2.0;
    Some([x as f32, y as f32])
}

fn tile_name(info: &TileInfo) -> &'static str {
    match info.surface {
        SurfaceKind::None => "none",
        SurfaceKind::Flat => "flat paint",
        SurfaceKind::Shaped => "shaped model",
    }
}

fn write_tile(out: &mut String, tile: (i32, i32), plane: usize, info: &TileInfo) {
    let _ = writeln!(
        out,
        "  tile ({}, {}) plane {plane}: surface={} faces={} skipped_faces={} underlay={} overlay_raw={} shape={} rotation={} settings={:#04x} (bridge={} vis_below={}) shadow={} min_plane={} linked_below={} heights[sw,se,ne,nw]={:?}",
        tile.0,
        tile.1,
        tile_name(info),
        info.faces,
        info.skipped_faces,
        info.underlay,
        info.overlay_raw,
        info.shape,
        info.rotation,
        info.settings,
        info.settings & 2 != 0,
        info.settings & 8 != 0,
        info.shadow,
        info.min_plane,
        info.has_linked_below,
        info.heights,
    );
}

fn write_loc(out: &mut String, loc: &LocInfo) {
    let def = &loc.definition;
    let _ = writeln!(
        out,
        "  OBJECT {} name={:?} type={} orientation={} anchor=({}, {}) source_plane={} draw_level={} min_plane={}",
        def.identity.id.get(),
        def.name,
        loc.loc_type,
        loc.orientation,
        loc.tile.0,
        loc.tile.1,
        loc.plane,
        loc.level,
        loc.min_plane,
    );
    let _ = writeln!(out, "    plan: {:?}", loc.plan);
    for (index, slot) in loc.slots.iter().enumerate() {
        let _ = writeln!(
            out,
            "    slot {index}: {} origin={:?} rotation={} bounds={:?}..{:?} vertices={} faces={} biased={} (max {}) alpha={} textured={} hidden={}",
            slot.kind,
            slot.origin,
            slot.rotation,
            slot.min,
            slot.max,
            slot.vertices,
            slot.faces,
            slot.biased_faces,
            slot.max_bias,
            slot.alpha_faces,
            slot.textured_faces,
            slot.hidden_faces,
        );
    }
    let _ = writeln!(
        out,
        "    def: size={}x{} decoration_displacement={} is_rotated={} non_flat_shading={} contour_clip={:?} ambient={} contrast={} animation={:?} morphs={}",
        def.size_x,
        def.size_y,
        def.decoration_displacement,
        def.is_rotated,
        def.non_flat_shading,
        def.contour_clip,
        def.ambient,
        def.contrast,
        def.animation,
        def.morphs.is_some(),
    );
    let _ = writeln!(
        out,
        "    def: scale={:?} translation={:?} recolors={:?} retextures={:?} ground_raise={} full_recolor={:?} placement={:?}",
        def.scale,
        def.translation,
        def.recolors,
        def.retextures,
        def.ground_raise,
        def.full_recolor,
        def.placement,
    );
    let _ = writeln!(out, "    def: models={:?}", def.models);
}

fn info_for(infos: &Infos, tile: (i32, i32)) -> Option<&Arc<RegionInfo>> {
    infos.get(&(tile.0.div_euclid(64), tile.1.div_euclid(64)))
}

/// Text for one object (`Copy object`).
pub fn describe_object(infos: &Infos, pick: &ObjectPick) -> String {
    let mut out = String::new();
    if let Some(loc) = infos
        .get(&pick.region)
        .and_then(|info| info.locs.get(pick.index))
    {
        let _ = writeln!(out, "=== object ===");
        write_loc(&mut out, loc);
        let _ = writeln!(out, "=== tile under the anchor ===");
        for plane in 0..4 {
            if let Some(info) =
                info_for(infos, loc.tile).and_then(|region| region.tile(plane, loc.tile))
            {
                write_tile(&mut out, loc.tile, plane, info);
            }
        }
    }
    out
}

/// Text for one tile: terrain on every plane and every object anchored on it.
pub fn describe_tile(infos: &Infos, tile: (i32, i32)) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "=== tile ({}, {}) ===", tile.0, tile.1);
    if let Some(region) = info_for(infos, tile) {
        for plane in 0..4 {
            if let Some(info) = region.tile(plane, tile) {
                write_tile(&mut out, tile, plane, info);
            }
        }
        for loc in region.locs_at(tile) {
            write_loc(&mut out, loc);
        }
    }
    out
}

/// Text for the tile and everything within `radius` tiles (objects whose footprint reaches the
/// area are included): the data needed to reason about normal merging and wall joins.
pub fn describe_neighbourhood(
    infos: &Infos,
    center: (i32, i32),
    plane: usize,
    radius: i32,
) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "=== neighbourhood of ({}, {}) radius {radius}, terrain plane {plane} ===",
        center.0, center.1
    );
    for dy in (-radius..=radius).rev() {
        for dx in -radius..=radius {
            let tile = (center.0 + dx, center.1 + dy);
            if let Some(info) = info_for(infos, tile).and_then(|region| region.tile(plane, tile)) {
                write_tile(&mut out, tile, plane, info);
            }
        }
    }
    let _ = writeln!(
        out,
        "--- objects (anchored within radius + 3, footprint overlapping) ---"
    );
    let reach = radius + 3;
    for dy in -reach..=reach {
        for dx in -reach..=reach {
            let anchor = (center.0 + dx, center.1 + dy);
            let Some(region) = info_for(infos, anchor) else {
                continue;
            };
            for loc in region.locs_at(anchor) {
                let footprint = loc.plan.rotated_definition_footprint;
                let (width, depth) = (i32::from(footprint.width), i32::from(footprint.depth));
                let overlaps = loc.tile.0 + width > center.0 - radius
                    && loc.tile.0 <= center.0 + radius
                    && loc.tile.1 + depth > center.1 - radius
                    && loc.tile.1 <= center.1 + radius;
                if overlaps {
                    write_loc(&mut out, loc);
                }
            }
        }
    }
    out
}

/// One-line summary for the hover readout.
pub fn summary(infos: &Infos, pick: &Pick) -> String {
    let mut parts = Vec::new();
    if let Some(object) = &pick.object
        && let Some(loc) = infos
            .get(&object.region)
            .and_then(|info| info.locs.get(object.index))
    {
        parts.push(format!(
            "object {} type {} rot {} at ({}, {}) plane {}",
            loc.definition.identity.id.get(),
            loc.loc_type,
            loc.orientation,
            loc.tile.0,
            loc.tile.1,
            loc.plane,
        ));
    }
    if let Some(tile) = &pick.tile {
        parts.push(format!("tile ({}, {})", tile.tile.0, tile.tile.1));
    }
    parts.join("  |  ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ray_points_project_back_to_their_pixel() {
        let mut camera = FlyCamera::new(3000.0, -2500.0, 1800.0);
        camera.yaw = 0.7;
        camera.pitch = 0.9;
        camera.fov_degrees = 70.0;
        let origin = (128 * 200, 128 * 300);
        let (width, height) = (1280.0_f32, 720.0_f32);
        for (px, py) in [
            (640.0, 360.0),
            (10.0, 10.0),
            (1200.0, 700.0),
            (300.0, 500.0),
        ] {
            let ray = ray_through_pixel(&camera, origin, width, height, px, py);
            for distance in [500.0, 4000.0] {
                let point = [
                    ray.origin[0] + ray.dir[0] * distance,
                    ray.origin[1] + ray.dir[1] * distance,
                    ray.origin[2] + ray.dir[2] * distance,
                ];
                let Some(projected) = project(&camera, origin, width, height, point) else {
                    panic!("point is in front of the camera");
                };
                assert!(
                    (projected[0] - px).abs() < 0.05,
                    "{projected:?} vs {px},{py}"
                );
                assert!(
                    (projected[1] - py).abs() < 0.05,
                    "{projected:?} vs {px},{py}"
                );
            }
        }
    }

    #[test]
    fn centre_pixel_ray_is_the_camera_forward_vector() {
        let mut camera = FlyCamera::new(0.0, -2000.0, 0.0);
        camera.yaw = -0.4;
        camera.pitch = 0.6;
        let ray = ray_through_pixel(&camera, (0, 0), 1000.0, 800.0, 500.0, 400.0);
        let forward = camera.forward();
        for (component, expected) in ray.dir.iter().zip(forward) {
            assert!((component - f64::from(expected)).abs() < 1e-5);
        }
    }

    /// Needs the pinned cache (`RUSTOSRS_TARGET_CACHE_DIR`).
    #[test]
    #[ignore = "requires the pinned build-241 cache"]
    fn picking_straight_down_finds_the_object_under_the_ray()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::var("RUSTOSRS_TARGET_CACHE_DIR")?;
        let mut definitions = osrs_world::WorldDefinitions::open(dir)?;
        let region = osrs_core::coords::RegionCoord::new(50, 50);
        let built = osrs_world::build_region_geometry(
            &mut definitions,
            region,
            osrs_world::TerrainPresentation::default(),
        )?
        .ok_or("region 50,50 missing")?;
        let mut infos = Infos::new();
        infos.insert((50, 50), built.info.clone());
        let mut checked = 0;
        for loc in built
            .info
            .locs
            .iter()
            .filter(|loc| loc.min_plane == 0)
            .step_by(37)
        {
            let Some(slot) = loc.slots.first() else {
                continue;
            };
            let center = [
                f64::from(slot.min[0] + slot.max[0]) / 2.0,
                f64::from(slot.min[2] + slot.max[2]) / 2.0,
            ];
            let ray = Ray {
                origin: [center[0], -20_000.0, center[1]],
                dir: [0.0, 1.0, 0.0],
            };
            let pick = pick(&infos, &ray, 0);
            let Some(object) = pick.object else { continue };
            let hit = &infos[&object.region].locs[object.index];
            // The hit object's footprint must contain the ray column.
            let contained = hit.slots.iter().any(|s| {
                f64::from(s.min[0]) - 4.0 <= center[0]
                    && center[0] <= f64::from(s.max[0]) + 4.0
                    && f64::from(s.min[2]) - 4.0 <= center[1]
                    && center[1] <= f64::from(s.max[2]) + 4.0
            });
            assert!(contained, "picked object does not contain the ray column");
            checked += 1;
        }
        assert!(checked > 20, "only {checked} locs were checked");
        Ok(())
    }

    /// Needs the pinned cache. Every sampled object, viewed from a camera in front of it, must
    /// be pickable at the pixel its centre projects to (or something nearer must be).
    #[test]
    #[ignore = "requires the pinned build-241 cache"]
    fn objects_are_pickable_at_their_projected_centre() -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::var("RUSTOSRS_TARGET_CACHE_DIR")?;
        let mut definitions = osrs_world::WorldDefinitions::open(dir)?;
        let built = osrs_world::build_region_geometry(
            &mut definitions,
            osrs_core::coords::RegionCoord::new(40, 48),
            osrs_world::TerrainPresentation::default(),
        )?
        .ok_or("region 40,48 missing")?;
        let mut infos = Infos::new();
        infos.insert((40, 48), built.info.clone());
        let (mut total, mut missed) = (0, 0);
        for (index, loc) in built.info.locs.iter().enumerate() {
            if loc.min_plane != 0 || loc.plane != 0 {
                continue;
            }
            let Some(slot) = loc.slots.first() else {
                continue;
            };
            let center = [
                f64::from(slot.min[0] + slot.max[0]) / 2.0,
                f64::from(slot.min[1] + slot.max[1]) / 2.0,
                f64::from(slot.min[2] + slot.max[2]) / 2.0,
            ];
            let mut camera = FlyCamera::new(0.0, 0.0, 0.0);
            camera.pitch = 0.9;
            // 700 units south and 900 up, looking north at the centre.
            camera.x = center[0] as f32;
            camera.y = (center[1] - 900.0) as f32;
            camera.z = (center[2] - 700.0) as f32;
            let (width, height) = (1280.0_f32, 720.0_f32);
            let Some(pixel) = project(&camera, (0, 0), width, height, center) else {
                continue;
            };
            if !(0.0..width).contains(&pixel[0]) || !(0.0..height).contains(&pixel[1]) {
                continue;
            }
            let ray = ray_through_pixel(&camera, (0, 0), width, height, pixel[0], pixel[1]);
            let pick = pick(&infos, &ray, 0);
            total += 1;
            let ok = pick.object.as_ref().is_some_and(|object| {
                let hit = &infos[&object.region].locs[object.index];
                hit.slots
                    .iter()
                    .any(|s| f64::from(s.min[0]) - 4.0 <= center[0] + 700.0)
                    && (object.index == index || ray_box_any(&ray, hit))
            });
            if !ok {
                missed += 1;
                if missed <= 8 {
                    println!(
                        "missed: object {} at {:?} (picked {:?})",
                        loc.definition.identity.id.get(),
                        loc.tile,
                        pick.object.as_ref().map(|o| infos[&o.region].locs[o.index]
                            .definition
                            .identity
                            .id
                            .get())
                    );
                }
            }
        }
        println!("{missed} of {total} sampled objects not pickable");
        assert!(
            missed * 20 <= total,
            "too many unpickable objects: {missed}/{total}"
        );
        Ok(())
    }

    fn ray_box_any(ray: &Ray, loc: &osrs_world::LocInfo) -> bool {
        loc.slots
            .iter()
            .any(|slot| ray_box(ray, slot.min, slot.max).is_some())
    }
}
