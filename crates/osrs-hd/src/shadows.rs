//! Directional shadows of the 117 HD port (phase 4): the plugin's shadow settings and the light's
//! orthographic camera that frames the shadow map around the viewer.
//!
//! The camera fit follows `ZoneRenderer.preSceneDraw`: it covers the receivers around the focal
//! point, snaps to the texel grid so shadows do not shimmer while the camera moves, and ignores
//! small size changes. Everything here is pure maths; the GPU passes live in `osrs-render`.
//! Upstream: BSD 2-Clause, see `THIRD_PARTY.md`.

use serde::{Deserialize, Serialize};

const TILE: f32 = 128.0;

/// `ShadowMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ShadowMode {
    Off,
    /// Shadows without any texture detail.
    Fast,
    /// Shadows that take the model textures' alpha into account.
    #[default]
    Detailed,
}

impl ShadowMode {
    pub const ALL: [Self; 3] = [Self::Off, Self::Fast, Self::Detailed];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Fast => "Fast",
            Self::Detailed => "Detailed",
        }
    }
}

/// `ShadowResolution` (the plugin also offers 32K, which no GPU here supports).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ShadowResolution {
    Low,
    Medium,
    #[default]
    High,
    Ultra,
    Extreme,
}

impl ShadowResolution {
    pub const ALL: [Self; 5] = [
        Self::Low,
        Self::Medium,
        Self::High,
        Self::Ultra,
        Self::Extreme,
    ];

    pub const fn texels(self) -> u32 {
        match self {
            Self::Low => 1024,
            Self::Medium => 2048,
            Self::High => 4096,
            Self::Ultra => 8192,
            Self::Extreme => 16384,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Low => "Low (1K)",
            Self::Medium => "Medium (2K)",
            Self::High => "High (4K)",
            Self::Ultra => "Ultra (8K)",
            Self::Extreme => "Extreme (16K)",
        }
    }
}

/// How shadow edges are filtered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterKind {
    Pcf,
    Dither,
    Average,
    Pcss,
}

/// `ShadowFiltering`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ShadowFiltering {
    Nearest,
    SmoothLow,
    #[default]
    SmoothHigh,
    DitheredLow,
    DitheredHigh,
    Pixelated,
    /// Percentage-closer soft shadows.
    Pcss,
}

impl ShadowFiltering {
    pub const ALL: [Self; 7] = [
        Self::Nearest,
        Self::SmoothLow,
        Self::SmoothHigh,
        Self::DitheredLow,
        Self::DitheredHigh,
        Self::Pixelated,
        Self::Pcss,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Nearest => "Nearest",
            Self::SmoothLow => "Smooth low",
            Self::SmoothHigh => "Smooth high",
            Self::DitheredLow => "Dithered low",
            Self::DitheredHigh => "Dithered high",
            Self::Pixelated => "Pixelated",
            Self::Pcss => "Soft shadows",
        }
    }

    /// The filtering technique (`ShadowFiltering.filtering`).
    pub const fn kind(self) -> FilterKind {
        match self {
            Self::Nearest | Self::SmoothLow | Self::SmoothHigh => FilterKind::Pcf,
            Self::DitheredLow | Self::DitheredHigh => FilterKind::Dither,
            Self::Pixelated => FilterKind::Average,
            Self::Pcss => FilterKind::Pcss,
        }
    }

    /// The kernel edge in texels (`ShadowFiltering.kernelSize`).
    pub const fn kernel(self) -> u32 {
        match self {
            Self::Nearest | Self::DitheredLow => 1,
            Self::SmoothLow | Self::DitheredHigh | Self::Pixelated | Self::Pcss => 2,
            Self::SmoothHigh => 3,
        }
    }
}

/// The shadow items of `HdPluginConfig` (the Shadows section).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShadowSettings {
    pub mode: ShadowMode,
    pub resolution: ShadowResolution,
    pub filtering: ShadowFiltering,
    /// Take model transparency into account (`enableShadowTransparency`).
    pub transparency: bool,
    /// Let terrain cast shadows (`terrainShadows`).
    pub terrain_shadows: bool,
    /// Cast shadows from roofs even when they are hidden (`experimentalRoofShadows`).
    pub roof_shadows: bool,
    /// How far from the camera shadows are drawn, in tiles (the legacy `shadowDistance`).
    pub distance_tiles: u32,
}

impl Default for ShadowSettings {
    fn default() -> Self {
        Self {
            mode: ShadowMode::Detailed,
            resolution: ShadowResolution::High,
            filtering: ShadowFiltering::SmoothHigh,
            transparency: true,
            terrain_shadows: true,
            roof_shadows: false,
            distance_tiles: 40,
        }
    }
}

impl ShadowSettings {
    pub const fn enabled(&self) -> bool {
        !matches!(self.mode, ShadowMode::Off)
    }
}

// ---------------------------------------------------------------------------------------------
// The light camera
// ---------------------------------------------------------------------------------------------

/// Inputs of [`ShadowCamera::fit`].
#[derive(Debug, Clone, Copy)]
pub struct ShadowFit {
    /// Unit vector from the scene towards the light (render space, `y` down).
    pub light_dir: [f32; 3],
    /// The point the camera looks at; only `x` and `z` matter.
    pub focus: [f32; 3],
    /// Radius of the square of receivers around the focus, in render units.
    pub radius: f32,
    /// Smallest and largest `y` (negative is up) of everything that can cast or receive.
    pub y_range: (f32, f32),
    /// Shadow map edge in texels.
    pub resolution: u32,
    /// Size of the previous fit: small size changes are ignored (shimmer).
    pub previous_size: Option<f32>,
}

/// The light's orthographic camera: render-space points map to clip space with `x`, `y` in
/// `-1..=1` (`y` up) and `z` in `0..=1`, `0` at the near plane towards the light.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadowCamera {
    /// Column-major `[column][row]`.
    pub view_proj: [[f32; 4]; 4],
    /// Orthonormal basis: `right` and `up` span the shadow map, `axis` points at the light.
    pub right: [f32; 3],
    pub up: [f32; 3],
    pub axis: [f32; 3],
    /// Edge of the shadow map in world units.
    pub size: f32,
    /// Length of the depth range in world units.
    pub depth_range: f32,
    /// World size of one shadow texel.
    pub texel_world: f32,
    /// Depth bias of one texel in normalised depth (`shadowBiasScale`).
    pub bias_scale: f32,
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let length = dot(v, v).sqrt();
    if length <= f32::EPSILON {
        return [0.0, -1.0, 0.0];
    }
    [v[0] / length, v[1] / length, v[2] / length]
}

impl ShadowCamera {
    /// The orthonormal light basis `(right, up, axis)` for a light direction.
    pub fn basis(light_dir: [f32; 3]) -> ([f32; 3], [f32; 3], [f32; 3]) {
        let axis = normalize(light_dir);
        // Any direction that is not parallel to the axis: straight up, or north when the light is
        // (nearly) vertical.
        let reference = if axis[1].abs() > 0.99 {
            [0.0, 0.0, 1.0]
        } else {
            [0.0, -1.0, 0.0]
        };
        let right = normalize(cross(reference, axis));
        let up = cross(axis, right);
        (right, up, axis)
    }

    /// Frame the receivers of `fit` (`ZoneRenderer.preSceneDraw`'s directional camera).
    pub fn fit(fit: &ShadowFit) -> Self {
        let (right, up, axis) = Self::basis(fit.light_dir);
        let (low, high) = fit.y_range;
        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];
        for dx in [-fit.radius, fit.radius] {
            for dz in [-fit.radius, fit.radius] {
                for y in [low, high] {
                    let point = [fit.focus[0] + dx, y, fit.focus[2] + dz];
                    let coords = [dot(point, right), dot(point, up), dot(point, axis)];
                    for axis_index in 0..3 {
                        min[axis_index] = min[axis_index].min(coords[axis_index]);
                        max[axis_index] = max[axis_index].max(coords[axis_index]);
                    }
                }
            }
        }

        // Fit the receiver footprint with room for filtering and texel snapping, rounded to half
        // tiles; ignore shrinking below 5% (it only makes shadows shimmer).
        let mut size = (((max[0] - min[0]).max(max[1] - min[1]) + 8.0 * TILE) / (TILE / 2.0))
            .ceil()
            * (TILE / 2.0);
        if let Some(previous) = fit.previous_size
            && (size - previous).abs() < previous * 0.05
            && size <= previous
        {
            size = previous;
        }
        let texel_world = size / fit.resolution.max(1) as f32;
        let snap = |value: f32| (value / texel_world).round() * texel_world;
        let center = [snap((min[0] + max[0]) * 0.5), snap((min[1] + max[1]) * 0.5)];

        // Casters above the receivers (towards the light) are inside the range too.
        let h_top = max[2] + 4.0 * TILE;
        let h_bottom = min[2] - 2.0 * TILE;
        let depth_range = h_top - h_bottom;

        let scale = 2.0 / size;
        let column = |i: usize| [right[i] * scale, up[i] * scale, -axis[i] / depth_range, 0.0];
        let view_proj = [
            column(0),
            column(1),
            column(2),
            [
                -center[0] * scale,
                -center[1] * scale,
                h_top / depth_range,
                1.0,
            ],
        ];
        Self {
            view_proj,
            right,
            up,
            axis,
            size,
            depth_range,
            texel_world,
            bias_scale: texel_world / depth_range,
        }
    }

    /// Project a render-space point: `[x, y, depth]` with `x`, `y` in `-1..=1`.
    pub fn project(&self, point: [f32; 3]) -> [f32; 3] {
        let m = &self.view_proj;
        let row = |r: usize| m[0][r] * point[0] + m[1][r] * point[1] + m[2][r] * point[2] + m[3][r];
        [row(0), row(1), row(2)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{light_direction, sun_angles};

    fn input() -> ShadowFit {
        ShadowFit {
            light_dir: light_direction(sun_angles(52.0, 235.0)),
            focus: [5000.0, 0.0, 7000.0],
            radius: 40.0 * TILE,
            y_range: (-3000.0, 800.0),
            resolution: 4096,
            previous_size: None,
        }
    }

    #[test]
    fn the_basis_is_orthonormal_even_for_a_vertical_light() {
        for dir in [
            input().light_dir,
            [0.0, -1.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 0.0, 0.0],
        ] {
            let (right, up, axis) = ShadowCamera::basis(dir);
            assert!(dot(right, up).abs() < 1e-5);
            assert!(dot(right, axis).abs() < 1e-5);
            assert!(dot(up, axis).abs() < 1e-5);
            for v in [right, up, axis] {
                assert!((dot(v, v) - 1.0).abs() < 1e-5);
            }
        }
    }

    #[test]
    fn receivers_project_into_the_unit_volume() {
        let camera = ShadowCamera::fit(&input());
        let fit = input();
        for dx in [-1.0, 0.0, 1.0] {
            for dz in [-1.0, 0.0, 1.0] {
                for y in [fit.y_range.0, fit.y_range.1] {
                    let p = [
                        fit.focus[0] + dx * fit.radius,
                        y,
                        fit.focus[2] + dz * fit.radius,
                    ];
                    let [x, yy, z] = camera.project(p);
                    assert!((-1.0..=1.0).contains(&x), "x {x}");
                    assert!((-1.0..=1.0).contains(&yy), "y {yy}");
                    assert!((0.0..=1.0).contains(&z), "z {z}");
                }
            }
        }
    }

    #[test]
    fn points_closer_to_the_light_have_smaller_depth() {
        let camera = ShadowCamera::fit(&input());
        let ground = [5200.0, 0.0, 6900.0];
        let lifted = [
            ground[0] + camera.axis[0] * 600.0,
            ground[1] + camera.axis[1] * 600.0,
            ground[2] + camera.axis[2] * 600.0,
        ];
        let (a, b) = (camera.project(ground), camera.project(lifted));
        // Same position on the map, nearer the light.
        assert!((a[0] - b[0]).abs() < 1e-5 && (a[1] - b[1]).abs() < 1e-5);
        assert!(b[2] < a[2]);
        assert!((a[2] - b[2] - 600.0 / camera.depth_range).abs() < 1e-4);
    }

    #[test]
    fn tall_casters_above_the_receivers_stay_in_range() {
        let fit = input();
        let camera = ShadowCamera::fit(&fit);
        // A 1500-unit tower on the edge of the receiver square, seen from the light.
        let top = [
            fit.focus[0] + fit.radius,
            -1500.0,
            fit.focus[2] + fit.radius,
        ];
        let z = camera.project(top)[2];
        assert!((0.0..=1.0).contains(&z), "{z}");
    }

    #[test]
    fn the_map_is_snapped_to_the_texel_grid() {
        let first = ShadowCamera::fit(&input());
        // Move the focus by a fraction of a texel along the light's right axis: the map stays on
        // the texel grid, so it either does not move or moves by whole texels (no shimmering).
        let mut moved = input();
        moved.previous_size = Some(first.size);
        let nudge = first.texel_world * 0.2;
        moved.focus[0] += first.right[0] * nudge;
        moved.focus[2] += first.right[2] * nudge;
        let second = ShadowCamera::fit(&moved);
        assert_eq!(first.size, second.size);
        let probe = [5000.0, -100.0, 7000.0];
        let shift = (first.project(probe)[0] - second.project(probe)[0]) * first.size / 2.0;
        let texels = shift / first.texel_world;
        assert!(
            (texels - texels.round()).abs() < 0.05 && texels.abs() <= 1.05,
            "{texels}"
        );

        // A full texel step moves the map by exactly one texel.
        let mut stepped = input();
        stepped.previous_size = Some(first.size);
        stepped.focus[0] += first.right[0] * first.texel_world * 3.0;
        stepped.focus[2] += first.right[2] * first.texel_world * 3.0;
        let third = ShadowCamera::fit(&stepped);
        let (a, b) = (first.project(probe), third.project(probe));
        let shift = (a[0] - b[0]) * first.size / 2.0;
        let texels = shift / first.texel_world;
        assert!((texels - texels.round()).abs() < 0.05, "{texels}");
    }

    #[test]
    fn small_size_reductions_are_ignored() {
        let first = ShadowCamera::fit(&input());
        let mut smaller = input();
        smaller.radius -= 40.0;
        smaller.previous_size = Some(first.size);
        assert_eq!(ShadowCamera::fit(&smaller).size, first.size);
        // A large change is followed.
        let mut much = input();
        much.radius = 10.0 * TILE;
        much.previous_size = Some(first.size);
        assert!(ShadowCamera::fit(&much).size < first.size);
    }

    #[test]
    fn filtering_options_map_to_the_plugin_kernels() {
        assert_eq!(ShadowFiltering::SmoothHigh.kernel(), 3);
        assert_eq!(ShadowFiltering::SmoothHigh.kind(), FilterKind::Pcf);
        assert_eq!(ShadowFiltering::DitheredHigh.kernel(), 2);
        assert_eq!(ShadowFiltering::Pcss.kind(), FilterKind::Pcss);
        assert_eq!(ShadowFiltering::Pixelated.kind(), FilterKind::Average);
        assert_eq!(ShadowSettings::default().resolution.texels(), 4096);
        assert!(ShadowSettings::default().enabled());
    }
}
