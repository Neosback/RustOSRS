//! How a path is planned (`PathSettings`: the same for every path) and what a path is made of
//! (`PathProfile`: its overlay, width and the noise that makes it look natural).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ObstaclePolicy {
    /// Route round solid objects (trees, rocks, buildings); decoration is built over.
    #[default]
    AvoidSolids,
    /// Route round every object.
    AvoidAll,
    /// Build the path, but not on tiles that are blocked.
    Mask,
    /// Build it where it was drawn and warn about what it crosses.
    WarnOnly,
    /// Ignore objects.
    Ignore,
}

impl ObstaclePolicy {
    pub const ALL: [Self; 5] = [
        Self::AvoidSolids,
        Self::AvoidAll,
        Self::Mask,
        Self::WarnOnly,
        Self::Ignore,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::AvoidSolids => "Go round solid objects",
            Self::AvoidAll => "Go round every object",
            Self::Mask => "Skip blocked tiles",
            Self::WarnOnly => "Ignore, but warn",
            Self::Ignore => "Ignore objects",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AutoConnectPolicy {
    Off,
    /// Join the two ends onto a nearby path of the same overlay.
    #[default]
    Endpoints,
    /// Keep endpoint joining and also add a short interior branch to a nearby compatible path.
    Network,
}

impl AutoConnectPolicy {
    pub const ALL: [Self; 3] = [Self::Off, Self::Endpoints, Self::Network];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Endpoints => "Ends",
            Self::Network => "Network",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PathSettings {
    pub seed: i64,
    /// Distance between samples of the centre line, in tiles.
    pub sample_spacing: f64,
    /// 0 keeps the drawn corners; 1 rounds them off into a smooth curve.
    pub smoothness: f64,
    /// Sub-samples per tile edge when the path is rasterised (2..=16).
    pub supersample: u32,
    pub obstacle_policy: ObstaclePolicy,
    pub auto_connect_policy: AutoConnectPolicy,
    /// How far an end may reach to join an existing path, in tiles.
    pub snap_radius: f64,
    /// How far from the drawn line a rerouted path may stray, in tiles.
    pub route_corridor: f64,
    pub slope_penalty: f64,
    pub width_scale: f64,
    pub noise_strength: f64,
    pub noise_scale: f64,
    /// Extra distance kept from obstacles, in tiles.
    pub clearance: f64,
    pub maximum_slope: f64,
    /// Repair small raster/shape disconnects after normal reference-compatible planning. Disabled
    /// in planner defaults; the interactive editor enables it by default.
    pub repair_continuity: bool,
    /// Keep tiles that already carry an overlay.
    pub preserve_existing: bool,
}

impl Default for PathSettings {
    fn default() -> Self {
        Self {
            seed: 0,
            sample_spacing: 0.25,
            smoothness: 0.65,
            supersample: 8,
            obstacle_policy: ObstaclePolicy::AvoidSolids,
            auto_connect_policy: AutoConnectPolicy::Endpoints,
            snap_radius: 2.5,
            route_corridor: 10.0,
            slope_penalty: 2.0,
            width_scale: 1.0,
            noise_strength: 1.0,
            noise_scale: 1.0,
            clearance: 0.0,
            maximum_slope: f64::MAX,
            repair_continuity: false,
            preserve_existing: true,
        }
    }
}

fn finite_or(value: f64, fallback: f64) -> f64 {
    if value.is_finite() { value } else { fallback }
}

impl PathSettings {
    /// The settings with every value brought into the range the planner accepts.
    pub fn sanitized(mut self) -> Self {
        let d = Self::default();
        self.sample_spacing = finite_or(self.sample_spacing, d.sample_spacing).clamp(0.05, 1.0);
        self.smoothness = finite_or(self.smoothness, d.smoothness).clamp(0.0, 1.0);
        self.supersample = self.supersample.clamp(2, 16);
        self.snap_radius = finite_or(self.snap_radius, d.snap_radius).max(0.0);
        self.route_corridor = finite_or(self.route_corridor, d.route_corridor).clamp(1.0, 128.0);
        self.slope_penalty = finite_or(self.slope_penalty, d.slope_penalty).max(0.0);
        self.width_scale = finite_or(self.width_scale, 1.0).max(0.01);
        self.noise_strength = finite_or(self.noise_strength, 1.0).max(0.0);
        self.noise_scale = finite_or(self.noise_scale, 1.0).max(0.01);
        self.clearance = finite_or(self.clearance, 0.0).max(0.0);
        if self.maximum_slope.is_nan() || self.maximum_slope < 0.0 {
            self.maximum_slope = f64::MAX;
        }
        self
    }
}

/// An overlay a path may use, and how often.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MaterialVariant {
    pub overlay_id: u16,
    pub weight: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PathProfile {
    pub id: String,
    pub name: String,
    pub primary_overlay_id: u16,
    pub material_variants: Vec<MaterialVariant>,
    /// Overlays the path treats as itself when it meets them (joining, keeping).
    pub compatible_overlay_ids: Vec<u16>,
    pub base_width: f64,
    pub edge_falloff: f64,
    pub centerline_wander: f64,
    pub wander_scale: f64,
    pub width_variation: f64,
    pub width_noise_scale: f64,
    pub edge_noise: f64,
    pub edge_noise_scale: f64,
    pub material_patch_length: f64,
    pub noise_octaves: u32,
    pub obstacle_clearance: f64,
    pub maximum_slope: f64,
    pub reroute: bool,
    pub auto_connect: bool,
}

impl Default for PathProfile {
    fn default() -> Self {
        Self::new("default", "Default path", 1)
    }
}

impl PathProfile {
    pub fn new(id: &str, name: &str, primary_overlay_id: u16) -> Self {
        Self {
            id: id.to_owned(),
            name: name.to_owned(),
            primary_overlay_id: primary_overlay_id.max(1),
            material_variants: Vec::new(),
            compatible_overlay_ids: Vec::new(),
            base_width: 2.0,
            edge_falloff: 0.15,
            centerline_wander: 0.15,
            wander_scale: 6.0,
            width_variation: 0.12,
            width_noise_scale: 8.0,
            edge_noise: 0.0,
            edge_noise_scale: 2.5,
            material_patch_length: 14.0,
            noise_octaves: 3,
            obstacle_clearance: 0.25,
            maximum_slope: f64::MAX,
            reroute: true,
            auto_connect: true,
        }
    }

    /// The profile as the planner reads it: the primary overlay is among the variants and the
    /// compatible overlays, every number in range.
    pub fn sanitized(mut self) -> Self {
        let d = Self::new(&self.id, &self.name, self.primary_overlay_id);
        self.primary_overlay_id = self.primary_overlay_id.max(1);
        self.material_variants
            .retain(|v| v.overlay_id > 0 && v.weight.is_finite() && v.weight > 0.0);
        if !self
            .material_variants
            .iter()
            .any(|v| v.overlay_id == self.primary_overlay_id)
        {
            self.material_variants.insert(
                0,
                MaterialVariant {
                    overlay_id: self.primary_overlay_id,
                    weight: 1.0,
                },
            );
        }
        let mut compatible: Vec<u16> = Vec::new();
        for id in self
            .compatible_overlay_ids
            .iter()
            .copied()
            .chain([self.primary_overlay_id])
            .chain(self.material_variants.iter().map(|v| v.overlay_id))
        {
            if id > 0 && !compatible.contains(&id) {
                compatible.push(id);
            }
        }
        self.compatible_overlay_ids = compatible;
        self.base_width = finite_or(self.base_width, d.base_width).max(0.01);
        self.edge_falloff = finite_or(self.edge_falloff, d.edge_falloff).clamp(0.0, 1.0);
        self.centerline_wander = finite_or(self.centerline_wander, 0.0).max(0.0);
        self.wander_scale = finite_or(self.wander_scale, d.wander_scale).max(0.01);
        self.width_variation = finite_or(self.width_variation, 0.0).clamp(0.0, 0.95);
        self.width_noise_scale = finite_or(self.width_noise_scale, d.width_noise_scale).max(0.01);
        self.edge_noise = finite_or(self.edge_noise, 0.0).max(0.0);
        self.edge_noise_scale = finite_or(self.edge_noise_scale, d.edge_noise_scale).max(0.01);
        self.material_patch_length =
            finite_or(self.material_patch_length, d.material_patch_length).max(0.01);
        self.noise_octaves = self.noise_octaves.clamp(1, 8);
        self.obstacle_clearance = finite_or(self.obstacle_clearance, 0.0).max(0.0);
        if self.maximum_slope.is_nan() || self.maximum_slope < 0.0 {
            self.maximum_slope = f64::MAX;
        }
        self
    }

    /// A hash of everything that shapes the path's look, to vary a random choice with the profile.
    pub fn parameter_signature(&self) -> i64 {
        const PRIME: i64 = 1_099_511_628_211;
        let mut h = (-3_750_763_034_362_895_579_i64 ^ i64::from(self.primary_overlay_id))
            .wrapping_mul(PRIME);
        for variant in &self.material_variants {
            h = ((h ^ i64::from(variant.overlay_id)).wrapping_mul(PRIME)
                ^ variant.weight.to_bits() as i64)
                .wrapping_mul(PRIME);
        }
        for id in &self.compatible_overlay_ids {
            h = (h ^ i64::from(*id)).wrapping_mul(PRIME);
        }
        for value in [
            self.base_width,
            self.edge_falloff,
            self.centerline_wander,
            self.wander_scale,
            self.width_variation,
            self.width_noise_scale,
            self.edge_noise,
            self.edge_noise_scale,
            self.material_patch_length,
            self.obstacle_clearance,
            self.maximum_slope,
        ] {
            h = (h ^ value.to_bits() as i64).wrapping_mul(PRIME);
        }
        (h ^ i64::from(self.noise_octaves)).wrapping_mul(PRIME)
    }

    pub fn compatible(&self, overlay: u16) -> bool {
        self.compatible_overlay_ids.contains(&overlay)
    }
}
