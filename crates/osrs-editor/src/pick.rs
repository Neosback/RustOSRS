//! Viewport picking and copyable diagnostics for tiles and objects.

mod describe;

pub use describe::{describe_neighbourhood, describe_object, describe_tile, summary, tile_values};

use crate::camera::FlyCamera;
use osrs_world::{LocInfo, RegionInfo, SurfaceKind, TileInfo};
use std::{collections::HashMap, sync::Arc};

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
#[derive(Debug, Clone)]
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
                let Some(box_distance) = ray_box(ray, slot.min, slot.max) else {
                    continue;
                };
                // The box is only a coarse filter: thin plates on diagonal walls fill most of
                // their box. Hit the real triangles when the slot has them.
                let distance = if slot.triangles.is_empty() {
                    box_distance
                } else {
                    match ray_triangles(ray, slot.origin, &slot.triangles) {
                        Some(distance) => distance,
                        None => continue,
                    }
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

/// Nearest hit of `ray` with the (double-sided) triangles of a slot placed at `origin`.
fn ray_triangles(ray: &Ray, origin: [i32; 3], triangles: &[[[i16; 3]; 3]]) -> Option<f64> {
    let origin = [
        f64::from(origin[0]),
        f64::from(origin[1]),
        f64::from(origin[2]),
    ];
    let point = |p: [i16; 3]| {
        [
            origin[0] + f64::from(p[0]),
            origin[1] + f64::from(p[1]),
            origin[2] + f64::from(p[2]),
        ]
    };
    let sub = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let cross = |a: [f64; 3], b: [f64; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let mut nearest: Option<f64> = None;
    for triangle in triangles {
        let (a, b, c) = (point(triangle[0]), point(triangle[1]), point(triangle[2]));
        let edge1 = sub(b, a);
        let edge2 = sub(c, a);
        let p = cross(ray.dir, edge2);
        let determinant = dot(edge1, p);
        if determinant.abs() < 1e-9 {
            continue;
        }
        let inverse = 1.0 / determinant;
        let t_vec = sub(ray.origin, a);
        let u = dot(t_vec, p) * inverse;
        if !(0.0..=1.0).contains(&u) {
            continue;
        }
        let q = cross(t_vec, edge1);
        let v = dot(ray.dir, q) * inverse;
        if v < 0.0 || u + v > 1.0 {
            continue;
        }
        let t = dot(edge2, q) * inverse;
        if t > 0.0 && nearest.is_none_or(|best| t < best) {
            nearest = Some(t);
        }
    }
    nearest
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

#[cfg(test)]
mod tests;
