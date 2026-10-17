//! Per-area lighting environments of the 117 HD port (phase 3): normalisation of the
//! `environments.json` entries, area selection, smooth transitions and the per-frame lighting
//! values (ambient, directional light, fog, sky and water colours) the HD shaders consume.
//!
//! Ported from `rs117.hd.scene.EnvironmentManager`, `rs117.hd.scene.environments.Environment`
//! and the sky-less path of `rs117.hd.renderer.SkyRenderer`. The day/night cycle and sky shading
//! (phase 7) are not part of this module: environments behave as if the plugin's "daylight cycle"
//! were off. Upstream: BSD 2-Clause, see `THIRD_PARTY.md`.

use serde::{Deserialize, Serialize};

use crate::{
    Color, HdData,
    expr::{Expression, NoVariables, Variables},
    settings::SeasonalTheme,
    tiles::{Aabb, Areas},
};

/// Tiles of camera movement between two frames that skips the transition (teleporting, entering a
/// dungeon). Walking over a loading line moves the focal point 40-41 tiles.
pub const SKIP_TRANSITION_DISTANCE: i32 = 41;
/// Seconds an environment transition takes.
pub const TRANSITION_DURATION: f32 = 3.0;
/// `HdPlugin.MAX_FOG_DEPTH`.
pub const MAX_FOG_DEPTH: f32 = 100.0;

const LUMINANCE: [f32; 3] = [0.2126, 0.7152, 0.0722];

/// `ColorUtils.ATMOSPHERIC_OPTICAL_DEPTH`: Rayleigh extinction plus neutral haze.
fn atmospheric_optical_depth() -> [f32; 3] {
    const HAZE: f32 = 0.08;
    [
        (1.0_f32 + 2.0 * 0.026).ln() + HAZE,
        (1.0_f32 + 2.0 * 0.053).ln() + HAZE,
        (1.0_f32 + 2.0 * 0.136).ln() + HAZE,
    ]
}

// ---------------------------------------------------------------------------------------------
// Colour helpers
// ---------------------------------------------------------------------------------------------

pub fn srgb_to_linear(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

pub fn linear_to_srgb(value: f32) -> f32 {
    if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

pub fn srgb_to_linear3(color: [f32; 3]) -> [f32; 3] {
    color.map(srgb_to_linear)
}

pub fn linear_to_srgb3(color: [f32; 3]) -> [f32; 3] {
    color.map(linear_to_srgb)
}

pub fn luminance(color: [f32; 3]) -> f32 {
    color[0] * LUMINANCE[0] + color[1] * LUMINANCE[1] + color[2] * LUMINANCE[2]
}

/// `ColorUtils.srgbToHsv`, all components in `0..=1`.
pub fn srgb_to_hsv(rgb: [f32; 3]) -> [f32; 3] {
    let [r, g, b] = rgb;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let hue = if delta <= f32::EPSILON {
        0.0
    } else if max == r {
        ((g - b) / delta).rem_euclid(6.0) / 6.0
    } else if max == g {
        ((b - r) / delta + 2.0) / 6.0
    } else {
        ((r - g) / delta + 4.0) / 6.0
    };
    let saturation = if max <= 0.0 { 0.0 } else { delta / max };
    [hue, saturation, max]
}

/// `ColorUtils.hsvToSrgb`.
pub fn hsv_to_srgb(hsv: [f32; 3]) -> [f32; 3] {
    let [h, s, v] = hsv;
    let c = v * s;
    let h6 = h.rem_euclid(1.0) * 6.0;
    let x = c * (1.0 - (h6.rem_euclid(2.0) - 1.0).abs());
    let (r, g, b) = match h6 as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    [r + m, g + m, b + m]
}

fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [mix(a[0], b[0], t), mix(a[1], b[1], t), mix(a[2], b[2], t)]
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A colour as the plugin reads it (`SrgbToLinearAdapter`): hex or 0-255 channels, converted to
/// linear sRGB.
pub fn color_to_linear(color: &Color) -> Option<[f32; 3]> {
    let srgb = match color {
        Color::Hex(_) => color.rgb()?,
        Color::Rgb(parts) if parts.len() >= 3 => {
            [parts[0] / 255.0, parts[1] / 255.0, parts[2] / 255.0]
        }
        Color::Rgb(_) => return None,
    };
    Some(srgb_to_linear3(srgb))
}

fn hex(value: &str) -> [f32; 3] {
    Color::Hex(value.to_owned())
        .rgb()
        .map_or([0.0; 3], srgb_to_linear3)
}

// ---------------------------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------------------------

/// `DefaultSkyColor`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DefaultSky {
    /// "117 HD Blue".
    #[default]
    Default,
    /// The host's own sky colour (RuneLite's Skybox plugin).
    Host,
    /// "Old School Black".
    Osrs,
    /// "2008 HD Tan".
    Hd2008,
}

impl DefaultSky {
    pub const ALL: [Self; 4] = [Self::Default, Self::Host, Self::Osrs, Self::Hd2008];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Default => "117 HD Blue",
            Self::Host => "Editor sky colour",
            Self::Osrs => "Old School Black",
            Self::Hd2008 => "2008 HD Tan",
        }
    }
}

/// `FogDepthMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FogDepthMode {
    /// The environment's own fog depth.
    #[default]
    Dynamic,
    /// A fixed fog depth for every area.
    Static,
}

impl FogDepthMode {
    pub const ALL: [Self; 2] = [Self::Dynamic, Self::Static];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Dynamic => "Dynamic",
            Self::Static => "Static",
        }
    }
}

/// The `HdPluginConfig` items that shape environments (Lighting and Environment sections).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EnvironmentSettings {
    /// Change environmental lighting based on the area (`environmentalLighting`).
    pub atmospheric_lighting: bool,
    pub fog_depth_mode: FogDepthMode,
    /// Distance fog and the haze at the draw-distance border (off: no fog at all).
    pub distance_fog: bool,
    /// Fog depth when [`FogDepthMode::Static`], in tiles.
    pub static_fog_depth: f32,
    pub ground_fog: bool,
    pub default_sky: DefaultSky,
    /// The host sky colour used by [`DefaultSky::Host`] (sRGB, `0..=1`); set by the host each
    /// frame, not persisted.
    #[serde(skip)]
    pub host_sky: [f32; 3],
    /// Force the default sky colour in every environment.
    pub override_sky: bool,
    pub decouple_sky_and_water: bool,
    pub season: SeasonalTheme,
    pub poh_themes: bool,
    pub legacy_tob: bool,
    /// Lightning flashes in storm areas (`flashingEffects`).
    pub flashing_effects: bool,
}

impl Default for EnvironmentSettings {
    fn default() -> Self {
        Self {
            atmospheric_lighting: true,
            fog_depth_mode: FogDepthMode::Dynamic,
            distance_fog: true,
            static_fog_depth: 5.0,
            ground_fog: true,
            default_sky: DefaultSky::Default,
            host_sky: [200.0 / 255.0, 184.0 / 255.0, 168.0 / 255.0],
            override_sky: false,
            decouple_sky_and_water: false,
            season: SeasonalTheme::Summer,
            poh_themes: true,
            legacy_tob: false,
            flashing_effects: false,
        }
    }
}

impl EnvironmentSettings {
    /// `DefaultSkyColor.getRgb`, linear.
    pub fn default_sky_rgb(&self) -> [f32; 3] {
        let srgb = match self.default_sky {
            DefaultSky::Default => [185.0 / 255.0, 214.0 / 255.0, 1.0],
            DefaultSky::Host => self.host_sky,
            DefaultSky::Osrs => [0.0; 3],
            DefaultSky::Hd2008 => [200.0 / 255.0, 192.0 / 255.0, 169.0 / 255.0],
        };
        srgb_to_linear3(srgb)
    }

    /// The sky colour used for the water of a default-sky area (`OSRS` black keeps the water blue).
    fn default_sky_for_water(&self) -> [f32; 3] {
        if self.default_sky == DefaultSky::Osrs {
            srgb_to_linear3([185.0 / 255.0, 214.0 / 255.0, 1.0])
        } else {
            self.default_sky_rgb()
        }
    }

    fn resolved_season(&self) -> SeasonalTheme {
        match self.season {
            SeasonalTheme::Automatic => SeasonalTheme::Summer,
            other => other,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Environment
// ---------------------------------------------------------------------------------------------

/// The blendable part of an environment (everything `Environment.interpolate` mixes).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnvValues {
    pub fog_color: [f32; 3],
    pub water_color: [f32; 3],
    pub ambient_color: [f32; 3],
    pub directional_color: [f32; 3],
    pub underglow_color: [f32; 3],
    pub water_caustics_color: [f32; 3],
    /// `[altitude, azimuth]` of the light in radians.
    pub shadow_angles: [f32; 2],
    pub fog_depth: f32,
    pub night_exposure: f32,
    pub ambient_strength: f32,
    pub directional_strength: f32,
    pub underglow_strength: f32,
    pub ground_fog_start: f32,
    pub ground_fog_end: f32,
    pub ground_fog_opacity: f32,
    pub water_caustics_strength: f32,
    pub wind_angle: f32,
    pub wind_speed: f32,
    pub wind_strength: f32,
    pub wind_ceiling: f32,
}

impl EnvValues {
    /// `Environment.interpolate`.
    pub fn interpolate(from: &Self, to: &Self, t: f32) -> Self {
        Self {
            fog_color: mix3(from.fog_color, to.fog_color, t),
            water_color: mix3(from.water_color, to.water_color, t),
            ambient_color: mix3(from.ambient_color, to.ambient_color, t),
            directional_color: mix3(from.directional_color, to.directional_color, t),
            underglow_color: mix3(from.underglow_color, to.underglow_color, t),
            water_caustics_color: mix3(from.water_caustics_color, to.water_caustics_color, t),
            shadow_angles: [
                mix(from.shadow_angles[0], to.shadow_angles[0], t),
                mix(from.shadow_angles[1], to.shadow_angles[1], t),
            ],
            fog_depth: mix(from.fog_depth, to.fog_depth, t),
            night_exposure: mix(from.night_exposure, to.night_exposure, t),
            ambient_strength: mix(from.ambient_strength, to.ambient_strength, t),
            directional_strength: mix(from.directional_strength, to.directional_strength, t),
            underglow_strength: mix(from.underglow_strength, to.underglow_strength, t),
            ground_fog_start: mix(from.ground_fog_start, to.ground_fog_start, t),
            ground_fog_end: mix(from.ground_fog_end, to.ground_fog_end, t),
            ground_fog_opacity: mix(from.ground_fog_opacity, to.ground_fog_opacity, t),
            water_caustics_strength: mix(
                from.water_caustics_strength,
                to.water_caustics_strength,
                t,
            ),
            wind_angle: mix(from.wind_angle, to.wind_angle, t),
            wind_speed: mix(from.wind_speed, to.wind_speed, t),
            wind_strength: mix(from.wind_strength, to.wind_strength, t),
            wind_ceiling: mix(from.wind_ceiling, to.wind_ceiling, t),
        }
    }
}

/// One normalised environment (`Environment` after `normalize()`).
#[derive(Debug, Clone)]
pub struct Environment {
    pub key: Option<String>,
    /// Name of the area it applies to (`ALL`, `NONE` or an `areas.json` name).
    pub area: String,
    pub description: Option<String>,
    pub is_overworld: bool,
    pub is_poh_theme: bool,
    pub is_legacy_tob: bool,
    pub is_underwater: bool,
    pub force: bool,
    pub allow_sky_override: bool,
    pub allow_roof_shadows: bool,
    pub lightning_effects: bool,
    pub instant_transition: bool,
    pub hide_vanilla_skyboxes: bool,
    pub has_water_color_override: bool,
    pub has_fog_color_override: bool,
    varbit_condition: Option<Expression>,
    varp_condition: Option<Expression>,
    pub values: EnvValues,
}

impl Environment {
    /// Names used in the debug output.
    pub fn label(&self) -> &str {
        self.key.as_deref().unwrap_or(&self.area)
    }

    /// Does the environment's varbit/varp condition hold for `vars`?
    pub fn condition_satisfied(&self, vars: &dyn Variables) -> bool {
        self.varbit_condition.as_ref().is_none_or(|c| c.test(vars))
            && self.varp_condition.as_ref().is_none_or(|c| c.test(vars))
    }

    pub const fn has_condition(&self) -> bool {
        self.varbit_condition.is_some() || self.varp_condition.is_some()
    }

    /// The sky colour fog fades into (`EnvironmentManager.getFogColor`).
    pub fn sky_fog_color(&self, settings: &EnvironmentSettings) -> [f32; 3] {
        if self.uses_default_sky(settings) {
            settings.default_sky_rgb()
        } else {
            self.values.fog_color
        }
    }

    /// `usesDefaultSkyColor`.
    pub fn uses_default_sky(&self, settings: &EnvironmentSettings) -> bool {
        !self.has_fog_color_override || self.allow_sky_override && settings.override_sky
    }

    /// The water colour the environment resolves to (`updateTargetSkyColor`).
    pub fn sky_water_color(&self, settings: &EnvironmentSettings) -> [f32; 3] {
        let mut water = if self.uses_default_sky(settings) {
            settings.default_sky_for_water()
        } else {
            self.values.fog_color
        };
        if self.has_water_color_override {
            water = self.values.water_color;
        } else if settings.decouple_sky_and_water {
            water = srgb_to_linear3([185.0 / 255.0, 214.0 / 255.0, 1.0]);
        }
        water
    }
}

/// `HDUtils.sunAngles(altitude, azimuth)` in degrees to radians.
pub fn sun_angles(altitude_degrees: f32, azimuth_degrees: f32) -> [f32; 2] {
    [altitude_degrees.to_radians(), azimuth_degrees.to_radians()]
}

/// The unit vector from a surface towards the light for `[altitude, azimuth]` (radians). Render
/// space is RuneLite's local space: `x` east, `z` north, `y` down, azimuth measured clockwise from
/// north (`180` = south). This is `Camera.getForwardDirection` of the plugin's directional camera
/// with yaw `PI - azimuth` and pitch `altitude`.
pub fn light_direction(angles: [f32; 2]) -> [f32; 3] {
    let [altitude, azimuth] = angles;
    [
        altitude.cos() * azimuth.sin(),
        -altitude.sin(),
        altitude.cos() * azimuth.cos(),
    ]
}

const fn default_values() -> EnvValues {
    EnvValues {
        fog_color: [0.0; 3],
        water_color: [0.0; 3],
        ambient_color: [1.0; 3],
        directional_color: [1.0; 3],
        underglow_color: [0.0; 3],
        water_caustics_color: [1.0; 3],
        shadow_angles: [0.0; 2],
        fog_depth: 25.0,
        night_exposure: 1.0,
        ambient_strength: 1.0,
        directional_strength: 0.25,
        underglow_strength: 0.0,
        ground_fog_start: -200.0,
        ground_fog_end: -500.0,
        ground_fog_opacity: 0.0,
        water_caustics_strength: 0.25,
        wind_angle: 0.0,
        wind_speed: 15.0,
        wind_strength: 0.0,
        wind_ceiling: 1280.0,
    }
}

impl Environment {
    /// `Environment.DEFAULT`: used wherever no area environment applies.
    pub fn fallback() -> Self {
        let mut values = default_values();
        values.fog_color = hex("#000000");
        values.water_color = hex("#66eaff");
        values.ambient_color = hex("#ffffff");
        values.shadow_angles = sun_angles(52.0, 235.0);
        values.water_caustics_color = values.directional_color;
        Self {
            key: Some("DEFAULT".to_owned()),
            area: "ALL".to_owned(),
            description: None,
            is_overworld: false,
            is_poh_theme: false,
            is_legacy_tob: false,
            is_underwater: false,
            force: false,
            allow_sky_override: true,
            allow_roof_shadows: true,
            lightning_effects: false,
            instant_transition: false,
            hide_vanilla_skyboxes: false,
            has_water_color_override: false,
            has_fog_color_override: false,
            varbit_condition: None,
            varp_condition: None,
            values,
        }
    }
}

/// Normalise the JSON entry (`Environment.normalize`). `default` is `Environment.DEFAULT`.
fn normalize(
    json: &crate::Environment,
    areas: &Areas,
    overworld: &[Aabb],
    default: &Environment,
) -> Environment {
    let area = json.area.clone().unwrap_or_else(|| "NONE".to_owned());
    let color = |value: &Option<Color>| value.as_ref().and_then(color_to_linear);
    let angle = |value: Option<f32>| value.map(f32::to_radians);

    let directional_color = color(&json.directional_color).unwrap_or([1.0; 3]);
    let directional_strength = json.directional_strength.unwrap_or(0.25);
    let has_fog_color = color(&json.fog_color).is_some();
    let has_water_color = color(&json.water_color).is_some();
    let mut fog_color = color(&json.fog_color).unwrap_or(default.values.fog_color);
    let mut water_color = color(&json.water_color).unwrap_or(default.values.water_color);
    let mut has_fog_color_override = has_fog_color;
    let mut has_water_color_override = has_water_color;

    let mut is_overworld = false;
    if area != "ALL" && area != "NONE" {
        is_overworld = areas.boxes(&area).is_some_and(|boxes| {
            boxes
                .iter()
                .any(|a| overworld.iter().any(|b| boxes_intersect(a, b)))
        });
        if !is_overworld {
            if !has_fog_color_override {
                fog_color = default.values.fog_color;
                has_fog_color_override = true;
            }
            if !has_water_color_override {
                water_color = default.values.water_color;
                has_water_color_override = true;
            }
        }
    }

    let ambient_color = color(&json.ambient_color).unwrap_or_else(|| {
        if is_overworld {
            let depth = atmospheric_optical_depth();
            std::array::from_fn(|i| directional_color[i] * 0.5 * (depth[i].exp() - 1.0))
        } else {
            default.values.ambient_color
        }
    });
    let ambient_strength = match json.ambient_strength {
        Some(strength) if strength >= 0.0 => strength,
        _ if is_overworld => directional_strength,
        _ => default.values.ambient_strength,
    };
    let shadow_angles = match json.shadow_angles.as_deref() {
        Some(&[altitude, azimuth, ..]) => sun_angles(altitude, azimuth),
        _ => default.values.shadow_angles,
    };
    let water_caustics_strength = match json.water_caustics_strength {
        Some(strength) if strength != -1.0 => strength,
        _ => directional_strength,
    };

    let condition = |name: &str| {
        json.extra
            .get(name)
            .and_then(|value| Expression::from_json(value).ok())
    };
    Environment {
        key: json.key.clone(),
        area,
        description: json.description.clone(),
        is_overworld,
        is_poh_theme: json.is_poh_theme == Some(true),
        is_legacy_tob: json
            .extra
            .get("isLegacyTob")
            .and_then(serde_json::Value::as_bool)
            == Some(true),
        is_underwater: json.is_underwater == Some(true),
        force: json.force == Some(true),
        allow_sky_override: json.allow_sky_override != Some(false),
        allow_roof_shadows: json.allow_roof_shadows != Some(false),
        lightning_effects: json
            .extra
            .get("lightningEffects")
            .and_then(serde_json::Value::as_bool)
            == Some(true),
        instant_transition: json.instant_transition == Some(true),
        hide_vanilla_skyboxes: json.hide_vanilla_skyboxes == Some(true),
        has_water_color_override,
        has_fog_color_override,
        varbit_condition: condition("varbitCondition"),
        varp_condition: condition("varpCondition"),
        values: EnvValues {
            fog_color,
            water_color,
            ambient_color,
            directional_color,
            underglow_color: color(&json.underglow_color).unwrap_or([0.0; 3]),
            water_caustics_color: color(&json.water_caustics_color).unwrap_or(directional_color),
            shadow_angles,
            fog_depth: json.fog_depth.unwrap_or(25.0).max(0.0),
            night_exposure: json.night_exposure.unwrap_or(1.0),
            ambient_strength,
            directional_strength,
            underglow_strength: json.underglow_strength.unwrap_or(0.0),
            ground_fog_start: json.ground_fog_start.unwrap_or(-200.0),
            ground_fog_end: json.ground_fog_end.unwrap_or(-500.0),
            ground_fog_opacity: json.ground_fog_opacity.unwrap_or(0.0),
            water_caustics_strength,
            wind_angle: angle(json.wind_angle).unwrap_or(0.0),
            wind_speed: json.wind_speed.unwrap_or(15.0),
            wind_strength: json.wind_strength.unwrap_or(0.0),
            wind_ceiling: json.wind_ceiling.unwrap_or(1280.0),
        },
    }
}

fn boxes_intersect(a: &Aabb, b: &Aabb) -> bool {
    (0..3).all(|axis| a.min[axis] <= b.max[axis] && a.max[axis] >= b.min[axis])
}

// ---------------------------------------------------------------------------------------------
// The environment table
// ---------------------------------------------------------------------------------------------

/// All environments of a settings combination, in the file's order (specific areas first).
#[derive(Debug, Clone)]
pub struct EnvironmentSet {
    environments: Vec<Environment>,
    default: Environment,
    overworld: Option<usize>,
    autumn: Option<usize>,
    winter: Option<usize>,
    areas: Areas,
}

impl EnvironmentSet {
    /// Normalise `data.environments`. Legacy Theatre of Blood and POH-theme environments stay in
    /// the table; [`Self::index_at`] skips them according to the settings
    /// (`EnvironmentManager.loadEnvironments`).
    pub fn new(data: &HdData, areas: &Areas) -> Self {
        let default = Environment::fallback();
        let overworld_boxes = areas
            .boxes("OVERWORLD")
            .map(<[Aabb]>::to_vec)
            .unwrap_or_default();
        let mut environments = Vec::new();
        let (mut overworld, mut autumn, mut winter) = (None, None, None);
        for json in &data.environments {
            let environment = normalize(json, areas, &overworld_boxes, &default);
            match environment.key.as_deref() {
                Some("OVERWORLD") => overworld = Some(environments.len()),
                Some("AUTUMN") => autumn = Some(environments.len()),
                Some("WINTER") => winter = Some(environments.len()),
                _ => {}
            }
            environments.push(environment);
        }
        Self {
            environments,
            default,
            overworld,
            autumn,
            winter,
            areas: areas.clone(),
        }
    }

    pub fn len(&self) -> usize {
        self.environments.len()
    }

    pub fn is_empty(&self) -> bool {
        self.environments.is_empty()
    }

    /// `Environment.DEFAULT`.
    pub const fn default_environment(&self) -> &Environment {
        &self.default
    }

    /// The environment at table position `index` (`usize::MAX` is [`Self::default_environment`]).
    pub fn get(&self, index: usize) -> &Environment {
        self.environments.get(index).unwrap_or(&self.default)
    }

    /// Index of the first environment whose area contains the tile and whose condition holds, or
    /// `usize::MAX` for the default environment (`EnvironmentManager.update` over the scene list
    /// with `Environment.DEFAULT` appended).
    pub fn index_at(
        &self,
        x: i32,
        y: i32,
        plane: i32,
        settings: &EnvironmentSettings,
        vars: &dyn Variables,
    ) -> usize {
        self.environments
            .iter()
            .position(|environment| {
                !(environment.is_legacy_tob && !settings.legacy_tob
                    || environment.is_poh_theme && !settings.poh_themes)
                    && self.areas.contains(&environment.area, x, y, plane)
                    && environment.condition_satisfied(vars)
            })
            .unwrap_or(usize::MAX)
    }

    /// The environment at a world tile under the default settings, with all game variables zero
    /// (a fresh account).
    pub fn environment_at(&self, x: i32, y: i32, plane: i32) -> &Environment {
        self.get(self.index_at(x, y, plane, &EnvironmentSettings::default(), &NoVariables))
    }

    /// `getOverworldEnvironment`.
    pub fn overworld_environment(&self, season: SeasonalTheme) -> &Environment {
        let index = match season {
            SeasonalTheme::Autumn => self.autumn,
            SeasonalTheme::Winter => self.winter,
            _ => self.overworld,
        };
        index.map_or(&self.default, |index| &self.environments[index])
    }

    fn overworld_index(&self, season: SeasonalTheme) -> usize {
        match season {
            SeasonalTheme::Autumn => self.autumn,
            SeasonalTheme::Winter => self.winter,
            _ => self.overworld,
        }
        .unwrap_or(usize::MAX)
    }
}

// ---------------------------------------------------------------------------------------------
// Transitions
// ---------------------------------------------------------------------------------------------

/// Drives the current environment: picks the environment around the camera's focal point and
/// blends to it over [`TRANSITION_DURATION`] seconds (`EnvironmentManager.update`).
#[derive(Debug, Clone)]
pub struct EnvironmentState {
    /// Table index of the area-selected environment (`usize::MAX` = default); `None` before the
    /// first update (`Environment.NONE`).
    target: Option<usize>,
    from: EnvValues,
    to: EnvValues,
    current: EnvValues,
    transition_complete: bool,
    transition_progress: f32,
    transition_start: f64,
    transition_id: u64,
    previous_position: [i32; 3],
    lightning: Lightning,
    /// The settings the target was last evaluated with: lighting-relevant changes re-apply it.
    applied: Option<(bool, SeasonalTheme)>,
}

#[derive(Debug, Clone)]
struct Lightning {
    enabled: bool,
    brightness: f32,
    next_time: Option<f64>,
    rng: u64,
}

impl Lightning {
    fn random(&mut self) -> f32 {
        // xorshift64*
        self.rng ^= self.rng >> 12;
        self.rng ^= self.rng << 25;
        self.rng ^= self.rng >> 27;
        ((self.rng.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40) as f32) / (1u64 << 24) as f32
    }

    fn next_time(&mut self, now: f64) -> f64 {
        const QUICK_CHANCE: f32 = 0.5;
        if self.random() <= QUICK_CHANCE {
            now + f64::from(mix(0.04, 0.15, self.random()))
        } else {
            now + f64::from(mix(5.5, 17.0, self.random()))
        }
    }

    /// `updateLightning`: the flash brightness for this frame.
    fn update(&mut self, now: f64, delta: f32) {
        const FADE_SPEED: f32 = 80.0;
        const NEW_BRIGHTNESS: f32 = 7.0;
        if self.brightness > 0.0 {
            self.brightness = (self.brightness - delta * FADE_SPEED).max(0.0);
        }
        match self.next_time {
            None => self.next_time = Some(self.next_time(now)),
            Some(at) if now > at => {
                self.brightness = NEW_BRIGHTNESS;
                self.next_time = Some(self.next_time(now));
            }
            Some(_) => {}
        }
    }
}

impl Default for EnvironmentState {
    fn default() -> Self {
        let default = Environment::fallback().values;
        Self {
            target: None,
            from: default,
            to: default,
            current: default,
            transition_complete: true,
            transition_progress: 1.0,
            transition_start: 0.0,
            transition_id: 0,
            previous_position: [0; 3],
            applied: None,
            lightning: Lightning {
                enabled: false,
                brightness: 0.0,
                next_time: None,
                rng: 0x9E37_79B9_7F4A_7C15,
            },
        }
    }
}

impl EnvironmentState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Forget the target so the next update snaps to its environment (a reload).
    pub fn reset(&mut self) {
        self.target = None;
    }

    /// The blended values for this frame (lightning not applied).
    pub const fn current(&self) -> &EnvValues {
        &self.current
    }

    pub const fn transition_progress(&self) -> f32 {
        self.transition_progress
    }

    pub const fn transition_id(&self) -> u64 {
        self.transition_id
    }

    /// Table index of the area-selected environment, if one was picked yet.
    pub const fn target_index(&self) -> Option<usize> {
        self.target
    }

    /// The area-selected environment (`getTargetEnvironment`).
    pub fn target<'a>(&self, set: &'a EnvironmentSet) -> &'a Environment {
        self.target.map_or(&set.default, |index| set.get(index))
    }

    pub const fn lightning_brightness(&self) -> f32 {
        self.lightning.brightness
    }

    /// The environment whose lighting the target uses: itself, or the overworld one when
    /// atmospheric lighting is off and the target does not force its own.
    fn lighting_environment<'a>(
        &self,
        set: &'a EnvironmentSet,
        settings: &EnvironmentSettings,
        area_environment: &'a Environment,
    ) -> &'a Environment {
        if !settings.atmospheric_lighting && !area_environment.force {
            set.overworld_environment(settings.resolved_season())
        } else {
            area_environment
        }
    }

    /// Resolve an `OVERWORLD`-keyed environment to the seasonal one (`getResolvedTargetEnvironment`).
    fn resolved_index(&self, set: &EnvironmentSet, settings: &EnvironmentSettings) -> usize {
        match self.target {
            Some(index) if set.get(index).key.as_deref() == Some("OVERWORLD") => {
                set.overworld_index(settings.resolved_season())
            }
            Some(index) => index,
            None => usize::MAX,
        }
    }

    /// Advance to time `now` (seconds) with the camera's focal point at world tile `focal`
    /// `(x, y, plane)`. `delta` is the frame time in seconds. `vars` supplies the varbits and
    /// varps environments can be conditioned on.
    pub fn update(
        &mut self,
        set: &EnvironmentSet,
        settings: &EnvironmentSettings,
        focal: [i32; 3],
        vars: &dyn Variables,
        now: f64,
        delta: f32,
    ) {
        let tile_change = (0..3)
            .map(|axis| (focal[axis] - self.previous_position[axis]).abs())
            .max()
            .unwrap_or(0);
        self.previous_position = focal;
        let skip_transition = tile_change >= SKIP_TRANSITION_DISTANCE;

        let index = set.index_at(focal[0], focal[1], focal[2], settings, vars);
        // Atmospheric lighting and the seasonal theme change what the same area resolves to: apply it again, blending like any other change.
        let applied = (settings.atmospheric_lighting, settings.season);
        let force = self.applied.is_some_and(|previous| previous != applied);
        self.applied = Some(applied);
        self.change_environment(set, settings, index, skip_transition, force, now);

        // Updated every frame, since the sky settings may change under a running transition.
        let resolved = set.get(self.resolved_index(set, settings));
        self.to.fog_color = resolved.sky_fog_color(settings);
        self.to.water_color = resolved.sky_water_color(settings);

        if self.transition_complete {
            // Fog and water colour are always written, since lightning affects them.
            self.current.fog_color = self.to.fog_color;
            self.current.water_color = self.to.water_color;
        } else {
            self.transition_progress = smoothstep(
                0.0,
                1.0,
                ((now - self.transition_start) / f64::from(TRANSITION_DURATION)) as f32,
            );
            self.current = EnvValues::interpolate(&self.from, &self.to, self.transition_progress);
            if self.transition_progress >= 1.0 {
                self.transition_complete = true;
            }
        }
        self.update_lightning(now, delta, settings);
    }

    fn update_lightning(&mut self, now: f64, delta: f32, settings: &EnvironmentSettings) {
        self.lightning.update(now, delta);
        if self.lightning.enabled && settings.flashing_effects {
            let t = self.lightning.brightness.clamp(0.0, 1.0);
            const LIGHTNING: [f32; 3] = [0.25; 3];
            self.current.fog_color = mix3(self.current.fog_color, LIGHTNING, t);
            self.current.water_color = mix3(self.current.water_color, LIGHTNING, t);
        } else {
            self.lightning.brightness = 0.0;
        }
    }

    fn change_environment(
        &mut self,
        set: &EnvironmentSet,
        settings: &EnvironmentSettings,
        index: usize,
        mut skip_transition: bool,
        force: bool,
        now: f64,
    ) {
        if self.target == Some(index) && !force {
            return;
        }
        let previous = self.target.map(|previous| set.get(previous));
        let new = set.get(index);
        if previous.is_none() {
            skip_transition = true;
        }
        if previous.is_some_and(|previous| previous.instant_transition) || new.instant_transition {
            skip_transition = true;
        }
        self.target = Some(index);
        self.transition_id += 1;
        self.transition_complete = false;
        self.transition_progress = 0.0;
        self.transition_start = now
            - if skip_transition {
                f64::from(TRANSITION_DURATION)
            } else {
                0.0
            };

        self.from = self.current;
        self.from.shadow_angles[0] =
            self.current.shadow_angles[0].rem_euclid(std::f32::consts::TAU);
        self.from.shadow_angles[1] =
            self.current.shadow_angles[1].rem_euclid(std::f32::consts::TAU);

        let area = set.get(self.resolved_index(set, settings));
        let lighting = self.lighting_environment(set, settings, area);
        self.to = lighting.values;
        self.to.fog_depth = area.values.fog_depth;
        self.to.ground_fog_start = area.values.ground_fog_start;
        self.to.ground_fog_end = area.values.ground_fog_end;
        self.to.ground_fog_opacity = area.values.ground_fog_opacity;
        self.to.shadow_angles = area.values.shadow_angles;
        self.lightning.enabled = area.lightning_effects;
        self.to.fog_color = area.sky_fog_color(settings);
        self.to.water_color = area.sky_water_color(settings);

        // Prevent transitions from taking the long way around.
        for axis in 0..2 {
            let diff = self.from.shadow_angles[axis] - self.to.shadow_angles[axis];
            if diff.abs() > std::f32::consts::PI {
                self.to.shadow_angles[axis] += std::f32::consts::TAU * diff.signum();
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Frame lighting
// ---------------------------------------------------------------------------------------------

/// Everything the HD shaders read from the environment for one frame (`UBOGlobal`'s lighting,
/// fog and water fields), in linear colour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lighting {
    /// Unit vector from the scene towards the light.
    pub light_dir: [f32; 3],
    pub light_angles: [f32; 2],
    /// Luminance-normalised ambient colour and its strength (the luminance).
    pub ambient_color: [f32; 3],
    pub ambient_strength: f32,
    pub light_color: [f32; 3],
    pub light_strength: f32,
    pub underglow_color: [f32; 3],
    pub underglow_strength: f32,
    pub fog_color: [f32; 3],
    /// Fog depth after scaling by the draw distance (`0` = no fog).
    pub fog_depth: f32,
    pub ground_fog_start: f32,
    pub ground_fog_end: f32,
    pub ground_fog_opacity: f32,
    pub water_color: [f32; 3],
    /// Water surface colours by brightness, as the plugin derives them from the water colour.
    pub water_color_light: [f32; 3],
    pub water_color_mid: [f32; 3],
    pub water_color_dark: [f32; 3],
    /// The area is underwater (`uboGlobal.underwaterEnvironment`).
    pub underwater_environment: bool,
    pub caustics_color: [f32; 3],
    pub caustics_strength: f32,
    pub lightning_brightness: f32,
    /// Whether the directional light casts shadows.
    pub casts_shadows: bool,
    /// The area allows roofs to cast shadows while hidden.
    pub allow_roof_shadows: bool,
    pub wind_angle: f32,
    pub wind_speed: f32,
    pub wind_strength: f32,
    pub wind_ceiling: f32,
    pub sun_dir: [f32; 3],
    pub sun_enabled: bool,
    pub sun_color: [f32; 3],
    pub sun_intensity: f32,
    pub moon_dir: [f32; 3],
    pub moon_phase: f32,
    pub moon_color: [f32; 3],
    pub moon_intensity: f32,
    pub sky_zenith_color: [f32; 3],
    pub horizon_width: f32,
}

impl Default for Lighting {
    fn default() -> Self {
        let settings = EnvironmentSettings::default();
        let environment = Environment::fallback();
        Self::from_values(&environment.values, &settings, 0.0, false, true, 0.0)
    }
}

impl Lighting {
    /// Evaluate `values` as the plugin's sky-less path does: authored light colours times
    /// strengths, split into a unit colour and a luminance strength (`SkyRenderer.prepareFrame`
    /// and `updateGlobalUbo`). `draw_distance_tiles` scales the fog depth.
    pub fn from_values(
        values: &EnvValues,
        settings: &EnvironmentSettings,
        draw_distance_tiles: f32,
        underwater: bool,
        allow_roof_shadows: bool,
        lightning_brightness: f32,
    ) -> Self {
        let scale = |color: [f32; 3], strength: f32| color.map(|c| c * strength);
        let ambient = scale(values.ambient_color, values.ambient_strength);
        let directional = scale(values.directional_color, values.directional_strength);
        let ambient_strength = luminance(ambient);
        let light_strength = luminance(directional);
        let normalise = |color: [f32; 3], strength: f32| {
            if strength > 0.0 {
                color.map(|c| c / strength)
            } else {
                color
            }
        };
        let fog_depth = if settings.distance_fog {
            match settings.fog_depth_mode {
                FogDepthMode::Dynamic => values.fog_depth,
                FogDepthMode::Static => settings.static_fog_depth,
            }
        } else {
            0.0
        } * draw_distance_tiles.min(90.0)
            / 10.0;

        let water_hsv = srgb_to_hsv(values.water_color);
        let water_shade = |factor: f32| {
            linear_to_srgb3(hsv_to_srgb([
                water_hsv[0],
                water_hsv[1],
                water_hsv[2] * factor,
            ]))
        };
        Self {
            light_dir: light_direction(values.shadow_angles),
            light_angles: values.shadow_angles,
            ambient_color: normalise(ambient, ambient_strength),
            ambient_strength,
            light_color: normalise(directional, light_strength),
            light_strength,
            underglow_color: values.underglow_color,
            underglow_strength: values.underglow_strength,
            fog_color: values.fog_color,
            fog_depth: fog_depth.min(MAX_FOG_DEPTH * 9.0),
            ground_fog_start: values.ground_fog_start,
            ground_fog_end: values.ground_fog_end,
            ground_fog_opacity: if settings.ground_fog {
                values.ground_fog_opacity
            } else {
                0.0
            },
            water_color: values.water_color,
            water_color_light: water_shade(0.8),
            water_color_mid: water_shade(0.45),
            water_color_dark: water_shade(0.05),
            underwater_environment: underwater,
            caustics_color: values.water_caustics_color,
            caustics_strength: values.water_caustics_strength,
            lightning_brightness,
            casts_shadows: light_strength > 0.0,
            allow_roof_shadows,
            wind_angle: values.wind_angle,
            wind_speed: values.wind_speed,
            wind_strength: values.wind_strength,
            wind_ceiling: values.wind_ceiling,
            sun_dir: light_direction(values.shadow_angles),
            sun_enabled: light_strength > 0.0,
            sun_color: normalise(directional, light_strength),
            sun_intensity: 1.0,
            moon_dir: {
                let s = light_direction(values.shadow_angles);
                [-s[0], -s[1], -s[2]]
            },
            moon_phase: 0.5,
            moon_color: [0.75, 0.82, 1.0],
            moon_intensity: 0.5,
            sky_zenith_color: [
                values.fog_color[0] * 0.4,
                values.fog_color[1] * 0.55,
                values.fog_color[2] * 0.85,
            ],
            horizon_width: 0.35,
        }
    }
}

impl EnvironmentState {
    /// The lighting of the current frame.
    pub fn lighting(
        &self,
        set: &EnvironmentSet,
        settings: &EnvironmentSettings,
        draw_distance_tiles: f32,
    ) -> Lighting {
        let target = self.target(set);
        Lighting::from_values(
            &self.current,
            settings,
            draw_distance_tiles,
            target.is_underwater,
            target.allow_roof_shadows,
            self.lightning.brightness,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hand_made() -> (EnvironmentSet, Areas) {
        let data = HdData {
            areas: serde_json::from_str(
                r#"[
                  { "name": "OVERWORLD", "aabbs": [[1000, 1000, 4000, 4000]] },
                  { "name": "TOWN", "aabbs": [[3000, 3000, 3100, 3100]] },
                  { "name": "CAVE", "aabbs": [[2000, 9000, 2100, 9100]] },
                  { "name": "QUEST", "aabbs": [[3000, 3000, 3100, 3100]] }
                ]"#,
            )
            .unwrap_or_default(),
            environments: serde_json::from_str(
                r##"[
                  { "area": "QUEST", "varbitCondition": "MYQ >= 5", "fogDepth": 99 },
                  { "area": "TOWN", "ambientColor": "#ffffff", "ambientStrength": 2,
                    "directionalColor": "#ffffff", "directionalStrength": 1,
                    "fogColor": "#102030", "fogDepth": 40, "shadowAngles": [40, 20] },
                  { "area": "CAVE", "ambientStrength": 0.5, "directionalStrength": 0 },
                  { "key": "OVERWORLD", "area": "OVERWORLD", "ambientColor": "#97baff",
                    "ambientStrength": 1, "directionalStrength": 4, "shadowAngles": [52, 235],
                    "fogDepth": 6.5 },
                  { "key": "WINTER", "ambientColor": "#8fcaff", "ambientStrength": 3.5,
                    "directionalStrength": 1.5, "fogColor": "#b8c5db", "fogDepth": 18 },
                  { "area": "TOWN", "instantTransition": true, "fogDepth": 1 }
                ]"##,
            )
            .unwrap_or_default(),
            ..HdData::default()
        };
        let areas = Areas::new(&data);
        (EnvironmentSet::new(&data, &areas), areas)
    }

    #[test]
    fn the_first_matching_environment_wins_and_default_is_the_fallback() {
        let (set, _) = hand_made();
        // TOWN is listed before the overworld; the QUEST entry needs a varbit that is zero.
        assert_eq!(set.environment_at(3050, 3050, 0).values.fog_depth, 40.0);
        // Outside every area: the default environment.
        assert_eq!(
            set.environment_at(10, 10, 0).key.as_deref(),
            Some("DEFAULT")
        );
        // The overworld environment covers the rest of the mainland.
        assert_eq!(
            set.environment_at(2500, 2500, 0).key.as_deref(),
            Some("OVERWORLD")
        );
        assert_eq!(
            set.environment_at(2050, 9050, 0)
                .values
                .directional_strength,
            0.0
        );
    }

    #[test]
    fn conditions_use_the_supplied_variables() {
        let (set, _) = hand_made();
        let mut vars = std::collections::HashMap::new();
        vars.insert("MYQ".to_owned(), crate::expr::Value::Int(7));
        let index = set.index_at(3050, 3050, 0, &EnvironmentSettings::default(), &vars);
        assert_eq!(set.get(index).values.fog_depth, 99.0);
        vars.insert("MYQ".to_owned(), crate::expr::Value::Int(2));
        let index = set.index_at(3050, 3050, 0, &EnvironmentSettings::default(), &vars);
        assert_eq!(set.get(index).values.fog_depth, 40.0);
    }

    #[test]
    fn normalisation_follows_the_plugin() {
        let (set, _) = hand_made();
        let cave = set.environment_at(2050, 9050, 0);
        // A non-overworld environment inherits the default fog and water colour, as overrides.
        assert!(cave.has_fog_color_override && cave.has_water_color_override);
        assert_eq!(
            cave.values.fog_color,
            set.default_environment().values.fog_color
        );
        // Missing ambient colour falls back to the default's (white).
        assert_eq!(cave.values.ambient_color, [1.0; 3]);

        let overworld = set.environment_at(2500, 2500, 0);
        assert!(overworld.is_overworld);
        // The overworld defines no fog colour: it follows the configured sky.
        assert!(!overworld.has_fog_color_override);
        let settings = EnvironmentSettings::default();
        assert_eq!(
            overworld.sky_fog_color(&settings),
            settings.default_sky_rgb()
        );
        // Without an explicit ambient strength the overworld uses the directional strength.
        let derived = &set.environment_at(2500, 2500, 0).values;
        assert!((derived.ambient_color[2] - srgb_to_linear(1.0)).abs() < 1e-3);

        // The shadow angles are stored in radians.
        let town = set.environment_at(3050, 3050, 0);
        assert!((town.values.shadow_angles[0] - 40.0_f32.to_radians()).abs() < 1e-6);
    }

    #[test]
    fn lighting_splits_colour_and_luminance_strength() {
        let (set, _) = hand_made();
        let town = set.environment_at(3050, 3050, 0);
        let lighting = Lighting::from_values(
            &town.values,
            &EnvironmentSettings::default(),
            50.0,
            false,
            true,
            0.0,
        );
        // White light: strength equals the authored strength, colour is white.
        assert!((lighting.ambient_strength - 2.0).abs() < 1e-4);
        assert!((lighting.light_strength - 1.0).abs() < 1e-4);
        assert!(
            lighting
                .ambient_color
                .iter()
                .all(|c| (c - 1.0).abs() < 1e-4)
        );
        assert!(lighting.casts_shadows);
        // Fog depth scales with the draw distance: 40 * 50 / 10.
        assert!((lighting.fog_depth - 200.0).abs() < 1e-3);

        let cave = set.environment_at(2050, 9050, 0);
        let dark = Lighting::from_values(
            &cave.values,
            &EnvironmentSettings::default(),
            50.0,
            false,
            true,
            0.0,
        );
        assert!(!dark.casts_shadows);
    }

    #[test]
    fn the_light_points_at_the_default_sun() {
        let dir = light_direction(sun_angles(52.0, 235.0));
        let length = dir.iter().map(|c| c * c).sum::<f32>().sqrt();
        assert!((length - 1.0).abs() < 1e-5);
        // Above the horizon (y is down), towards the south-west (azimuth 235).
        assert!(dir[1] < 0.0 && dir[0] < 0.0 && dir[2] < 0.0);
    }

    #[test]
    fn hsv_round_trips() {
        for color in [
            [0.2, 0.4, 0.9],
            [1.0, 0.0, 0.0],
            [0.5, 0.5, 0.5],
            [0.0, 0.8, 0.1],
        ] {
            let back = hsv_to_srgb(srgb_to_hsv(color));
            for (a, b) in color.iter().zip(back) {
                assert!((a - b).abs() < 1e-5, "{color:?} -> {back:?}");
            }
        }
    }

    fn state_at(state: &mut EnvironmentState, set: &EnvironmentSet, focal: [i32; 3], now: f64) {
        state.update(
            set,
            &EnvironmentSettings::default(),
            focal,
            &NoVariables,
            now,
            0.016,
        );
    }

    #[test]
    fn the_first_update_snaps_and_walking_blends_over_three_seconds() {
        let (set, _) = hand_made();
        let mut state = EnvironmentState::new();
        // The first environment is applied instantly.
        state_at(&mut state, &set, [2500, 2500, 0], 0.0);
        assert_eq!(state.transition_progress(), 1.0);
        let overworld = state.current().directional_strength;
        assert_eq!(overworld, 4.0);

        // Walking one tile into the town (an instantTransition-free neighbour of the cave test
        // below) starts a blend.
        state_at(&mut state, &set, [2050, 9050, 0], 10.0);
        // 6550 tiles away: a teleport, so the transition is skipped.
        assert_eq!(state.current().directional_strength, 0.0);

        // A short walk blends.
        state_at(&mut state, &set, [2500, 2500, 0], 20.0);
        let mut state = EnvironmentState::new();
        state_at(&mut state, &set, [2990, 3000, 0], 0.0);
        assert_eq!(state.target(&set).key.as_deref(), Some("OVERWORLD"));
        state_at(&mut state, &set, [3010, 3010, 0], 1.0);
        assert_eq!(state.target(&set).area, "TOWN");
        assert_eq!(state.transition_progress(), 0.0);
        state_at(&mut state, &set, [3010, 3010, 0], 2.5);
        let halfway = state.current().ambient_strength;
        // The transition is a smoothstep between the overworld (1) and the town (2) ambient.
        assert!(halfway > 1.0 && halfway < 2.0, "{halfway}");
        assert!(state.transition_progress() > 0.0 && state.transition_progress() < 1.0);
        state_at(&mut state, &set, [3010, 3010, 0], 4.5);
        assert_eq!(state.transition_progress(), 1.0);
        assert_eq!(state.current().ambient_strength, 2.0);
        assert!(state.transition_id() >= 2);
    }

    #[test]
    fn teleports_skip_the_transition() {
        let (set, _) = hand_made();
        let mut state = EnvironmentState::new();
        state_at(&mut state, &set, [2500, 2500, 0], 0.0);
        // 600 tiles away: instant.
        state_at(&mut state, &set, [3050, 3050, 0], 1.0);
        assert_eq!(state.current().fog_depth, 40.0);
        assert_eq!(state.transition_progress(), 1.0);
    }

    #[test]
    fn without_atmospheric_lighting_the_overworld_lights_every_area() {
        let (set, _) = hand_made();
        let settings = EnvironmentSettings {
            atmospheric_lighting: false,
            ..EnvironmentSettings::default()
        };
        let mut state = EnvironmentState::new();
        state.update(&set, &settings, [3050, 3050, 0], &NoVariables, 0.0, 0.016);
        // The town's own fog depth is kept, but the light is the overworld's.
        assert_eq!(state.target(&set).area, "TOWN");
        assert_eq!(state.current().directional_strength, 4.0);
        assert_eq!(state.current().fog_depth, 40.0);
    }

    #[test]
    fn changing_atmospheric_lighting_re_applies_the_current_area() {
        let (set, _) = hand_made();
        let mut state = EnvironmentState::new();
        let on = EnvironmentSettings::default();
        let off = EnvironmentSettings {
            atmospheric_lighting: false,
            ..on
        };
        let update = |state: &mut EnvironmentState, settings: &EnvironmentSettings, now: f64| {
            state.update(&set, settings, [3050, 3050, 0], &NoVariables, now, 0.016);
        };
        update(&mut state, &on, 0.0);
        assert_eq!(state.current().directional_strength, 1.0);
        // Standing still, the switch makes the overworld light the town after the blend.
        update(&mut state, &off, 1.0);
        assert!(state.transition_progress() < 0.1);
        update(&mut state, &off, 5.0);
        assert_eq!(state.current().directional_strength, 4.0);
        // And back.
        update(&mut state, &on, 6.0);
        update(&mut state, &on, 10.0);
        assert_eq!(state.current().directional_strength, 1.0);
    }

    #[test]
    fn distance_fog_can_be_switched_off() {
        let (set, _) = hand_made();
        let town = set.environment_at(3050, 3050, 0);
        let mut settings = EnvironmentSettings::default();
        let fogged = Lighting::from_values(&town.values, &settings, 50.0, false, true, 0.0);
        assert!(fogged.fog_depth > 0.0);
        settings.distance_fog = false;
        let clear = Lighting::from_values(&town.values, &settings, 50.0, false, true, 0.0);
        assert_eq!(clear.fog_depth, 0.0);
    }

    #[test]
    fn instant_transition_environments_do_not_blend() {
        let data = HdData {
            areas: serde_json::from_str(
                r#"[ { "name": "A", "aabbs": [[0, 0, 9, 9]] }, { "name": "B", "aabbs": [[10, 0, 19, 9]] } ]"#,
            )
            .unwrap_or_default(),
            environments: serde_json::from_str(
                r#"[ { "area": "A", "fogDepth": 10 }, { "area": "B", "fogDepth": 90, "instantTransition": true } ]"#,
            )
            .unwrap_or_default(),
            ..HdData::default()
        };
        let areas = Areas::new(&data);
        let set = EnvironmentSet::new(&data, &areas);
        let mut state = EnvironmentState::new();
        state_at(&mut state, &set, [5, 5, 0], 0.0);
        state_at(&mut state, &set, [12, 5, 0], 1.0);
        assert_eq!(state.current().fog_depth, 90.0);
    }

    #[test]
    fn the_shadow_angle_takes_the_short_way_round() {
        let data = HdData {
            areas: serde_json::from_str(
                r#"[ { "name": "A", "aabbs": [[0, 0, 9, 9]] }, { "name": "B", "aabbs": [[10, 0, 19, 9]] } ]"#,
            )
            .unwrap_or_default(),
            environments: serde_json::from_str(
                r#"[ { "area": "A", "shadowAngles": [50, 350] }, { "area": "B", "shadowAngles": [50, 10] } ]"#,
            )
            .unwrap_or_default(),
            ..HdData::default()
        };
        let areas = Areas::new(&data);
        let set = EnvironmentSet::new(&data, &areas);
        let mut state = EnvironmentState::new();
        state_at(&mut state, &set, [5, 5, 0], 0.0);
        state_at(&mut state, &set, [12, 5, 0], 10.0);
        // Halfway between 350 and 370 degrees, not between 350 and 10.
        state_at(&mut state, &set, [12, 5, 0], 11.5);
        let azimuth = state.current().shadow_angles[1].to_degrees();
        assert!((azimuth - 360.0).abs() < 1.0, "{azimuth}");
    }

    #[test]
    fn sky_colour_follows_the_environment_and_the_settings() {
        let (set, _) = hand_made();
        let settings = EnvironmentSettings::default();
        let town = set.environment_at(3050, 3050, 0);
        // The town defines its own fog colour, so it is used instead of the default sky ...
        assert!(town.has_fog_color_override);
        assert_eq!(town.sky_fog_color(&settings), town.values.fog_color);
        // ... unless the user forces the default sky.
        let forced = EnvironmentSettings {
            override_sky: true,
            ..settings
        };
        assert_eq!(town.sky_fog_color(&forced), forced.default_sky_rgb());
        // A decoupled water colour keeps the water blue under a black sky.
        let black = EnvironmentSettings {
            default_sky: DefaultSky::Osrs,
            ..settings
        };
        let overworld = set.environment_at(2500, 2500, 0);
        assert_eq!(overworld.sky_fog_color(&black), [0.0; 3]);
        assert!(overworld.sky_water_color(&black)[2] > 0.5);
    }
}
