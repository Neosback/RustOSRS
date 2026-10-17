//! Light definitions, spatial indexing, animation waveforms, and selection.
//!
//! Placed lights, object-attached lights, and NPC lights defined in `lights.json`
//! are evaluated with distance attenuation matching the 117HD specification:
//! `attenuation = (1 - sqrt(distance^2 / radius^2))^2`.

use crate::{Color, Gamevals, HdData, Light};
use std::{collections::HashMap, f32::consts::PI};

/// Evaluated point light in world coordinates ready for GPU upload.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EvaluatedLight {
    /// World position X (world units, 1 tile = 128 units).
    pub world_x: f32,
    /// World height Y.
    pub world_y: f32,
    /// World position Z (world units, 1 tile = 128 units).
    pub world_z: f32,
    /// Effective radius in world units.
    pub radius: f32,
    /// Linear color components (r, g, b) in `0.0..=1.0`.
    pub color: [f32; 3],
    /// Effective light intensity / strength.
    pub strength: f32,
}

impl EvaluatedLight {
    /// Squared radius for GPU culling and attenuation calculation.
    #[inline]
    pub fn radius_squared(&self) -> f32 {
        self.radius * self.radius
    }

    /// Evaluates attenuation at the given distance squared: `(1 - sqrt(d^2 / r^2))^2`.
    #[inline]
    pub fn attenuation(&self, distance_squared: f32) -> f32 {
        let r_sq = self.radius_squared();
        if distance_squared >= r_sq || r_sq <= 0.0 {
            0.0
        } else {
            let att = 1.0 - (distance_squared / r_sq).sqrt();
            att * att
        }
    }
}

/// Waveform generator for animated lights (PULSE, FLICKER, or STATIC).
pub fn evaluate_light_animation(
    kind: Option<&str>,
    base_strength: f32,
    base_radius: f32,
    duration_ms: Option<f32>,
    range_percent: Option<f32>,
    elapsed_seconds: f64,
    random_seed: f32,
) -> (f32, f32) {
    let range = range_percent.unwrap_or(0.0).max(0.0);
    match kind.map(str::to_ascii_uppercase).as_deref() {
        Some("FLICKER") => {
            // RLHD flicker waveform: sum of high-order cosine harmonics normalized by 4.335.
            let t = (2.0 * PI as f64) * ((elapsed_seconds % 60.0) / 60.0 + f64::from(random_seed));
            let c11 = (11.0 * t).cos().powi(3) as f32;
            let c17 = (17.0 * t).cos().powi(6) as f32;
            let c23 = (23.0 * t).cos().powi(2) as f32;
            let c31 = (31.0 * t).cos().powi(6) as f32;
            let c71 = (71.0 * t).cos().powi(4) as f32;
            let c151 = (151.0 * t).cos().powi(6) as f32 / 2.0;
            let raw_flicker = (c11 + c17 + c23 + c31 + c71 + c151) / 4.335;

            let max_flicker = 1.0 + range / 100.0;
            let min_flicker = 1.0 - range / 100.0;
            let flicker = min_flicker + (max_flicker - min_flicker) * raw_flicker.clamp(0.0, 1.0);

            (base_strength * flicker, base_radius * 1.5)
        }
        Some("PULSE") => {
            let duration = duration_ms.unwrap_or(1000.0).max(1.0) / 1000.0;
            let anim = (elapsed_seconds as f32 / duration).fract();
            let wave = 1.0 - 2.0 * (anim - 0.5).abs();
            let multiplier = 1.0 + (2.0 * wave - 1.0) * (range / 100.0);
            (base_strength * multiplier, base_radius * multiplier)
        }
        _ => (base_strength, base_radius),
    }
}

/// Spatially indexed light set supporting fast region and entity queries.
#[derive(Debug, Clone, Default)]
pub struct LightSet {
    /// Placed lights indexed by `(plane, tile_x, tile_y)`.
    placed: HashMap<(i32, i32, i32), Vec<Light>>,
    /// Object lights indexed by resolved loc/object ID.
    object_lights: HashMap<u32, Vec<Light>>,
    /// NPC lights indexed by resolved NPC ID.
    npc_lights: HashMap<u32, Vec<Light>>,
    /// All raw definitions.
    all: Vec<Light>,
}

impl LightSet {
    /// Build a new light set from HD scene data and game values.
    pub fn new(data: &HdData) -> Self {
        let mut placed: HashMap<(i32, i32, i32), Vec<Light>> = HashMap::new();
        let mut object_lights: HashMap<u32, Vec<Light>> = HashMap::new();
        let mut npc_lights: HashMap<u32, Vec<Light>> = HashMap::new();

        for light in &data.lights {
            if let (Some(x), Some(y)) = (light.world_x, light.world_y) {
                let plane = light.plane.unwrap_or(0);
                placed.entry((plane, x, y)).or_default().push(light.clone());
            }

            for id_ref in &light.object_ids {
                if let Some(id) = Gamevals::resolve(&data.gamevals.objects, id_ref)
                    && id >= 0
                {
                    object_lights
                        .entry(id as u32)
                        .or_default()
                        .push(light.clone());
                }
            }

            for id_ref in &light.npc_ids {
                if let Some(id) = Gamevals::resolve(&data.gamevals.npcs, id_ref)
                    && id >= 0
                {
                    npc_lights.entry(id as u32).or_default().push(light.clone());
                }
            }
        }

        Self {
            placed,
            object_lights,
            npc_lights,
            all: data.lights.clone(),
        }
    }

    /// Total count of all loaded light definitions.
    #[inline]
    pub fn len(&self) -> usize {
        self.all.len()
    }

    /// Whether the light set is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.all.is_empty()
    }

    /// Query lights attached to an object / loc ID.
    pub fn for_object(&self, object_id: u32) -> &[Light] {
        self.object_lights
            .get(&object_id)
            .map_or(&[], Vec::as_slice)
    }

    /// Query lights attached to an NPC ID.
    pub fn for_npc(&self, npc_id: u32) -> &[Light] {
        self.npc_lights.get(&npc_id).map_or(&[], Vec::as_slice)
    }

    /// Collect and evaluate visible point lights around a camera focal point.
    ///
    /// Lights are culled within `draw_distance_tiles` (plus their own radius)
    /// and sorted by distance to `(focal_world_x, focal_world_z)` up to `max_lights`.
    pub fn collect_visible(
        &self,
        plane: i32,
        focal_world_x: f32,
        focal_world_z: f32,
        draw_distance_tiles: f32,
        elapsed_seconds: f64,
        max_lights: usize,
    ) -> Vec<EvaluatedLight> {
        let max_distance = draw_distance_tiles * 128.0;
        let mut candidates = Vec::new();

        let min_tile_x = ((focal_world_x - max_distance) / 128.0).floor() as i32 - 10;
        let max_tile_x = ((focal_world_x + max_distance) / 128.0).ceil() as i32 + 10;
        let min_tile_y = ((focal_world_z - max_distance) / 128.0).floor() as i32 - 10;
        let max_tile_y = ((focal_world_z + max_distance) / 128.0).ceil() as i32 + 10;

        for (&(p, tx, ty), lights) in &self.placed {
            if p != plane
                || tx < min_tile_x
                || tx > max_tile_x
                || ty < min_tile_y
                || ty > max_tile_y
            {
                continue;
            }

            for light in lights {
                let base_radius = light.radius.unwrap_or(400) as f32;
                let base_strength = light.strength.unwrap_or(12.0);
                let (strength, radius) = evaluate_light_animation(
                    light.kind.as_deref(),
                    base_strength,
                    base_radius,
                    light.duration,
                    light.range,
                    elapsed_seconds,
                    ((tx * 31 + ty * 17) & 0xff) as f32 / 255.0,
                );

                if strength <= 0.0 || radius <= 0.0 {
                    continue;
                }

                let (offset_x, offset_z) = if let Some(offset) = &light.offset {
                    (
                        offset.first().copied().unwrap_or(0.0),
                        offset.get(2).copied().unwrap_or(0.0),
                    )
                } else {
                    (0.0, 0.0)
                };

                let world_x = (tx as f32) * 128.0 + 64.0 + offset_x;
                let world_z = (ty as f32) * 128.0 + 64.0 + offset_z;
                let height = light.height.unwrap_or(0) as f32;

                let dx = world_x - focal_world_x;
                let dz = world_z - focal_world_z;
                let dist_sq = dx * dx + dz * dz;

                let cutoff = max_distance + radius;
                if dist_sq <= cutoff * cutoff {
                    let color = light
                        .color
                        .as_ref()
                        .and_then(Color::rgb)
                        .unwrap_or([1.0, 0.9, 0.7]);

                    candidates.push((
                        dist_sq,
                        EvaluatedLight {
                            world_x,
                            world_y: -height, // OSRS height convention
                            world_z,
                            radius,
                            color,
                            strength,
                        },
                    ));
                }
            }
        }

        // Sort by distance to focal point, closest lights first.
        candidates.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        candidates
            .into_iter()
            .take(max_lights)
            .map(|(_, light)| light)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_light_preserves_values() {
        let (strength, radius) = evaluate_light_animation(None, 15.0, 300.0, None, None, 10.0, 0.0);
        assert_eq!(strength, 15.0);
        assert_eq!(radius, 300.0);
    }

    #[test]
    fn pulse_animation_oscillates_within_range() {
        let (s_mid, r_mid) = evaluate_light_animation(
            Some("PULSE"),
            10.0,
            100.0,
            Some(2000.0),
            Some(20.0),
            0.5,
            0.0,
        );
        assert!((8.0..=12.0).contains(&s_mid));
        assert!((80.0..=120.0).contains(&r_mid));
    }

    #[test]
    fn attenuation_drops_to_zero_at_radius() {
        let light = EvaluatedLight {
            world_x: 0.0,
            world_y: 0.0,
            world_z: 0.0,
            radius: 100.0,
            color: [1.0, 1.0, 1.0],
            strength: 10.0,
        };
        assert_eq!(light.attenuation(10000.0), 0.0);
        assert_eq!(light.attenuation(20000.0), 0.0);
        assert!((light.attenuation(0.0) - 1.0).abs() < 1e-5);
        assert!((light.attenuation(2500.0) - 0.25).abs() < 1e-3);
    }
}
