//! Render profiles.
//!
//! `Reference` is the RuneLite-parity renderer: one scene pass resolving straight into the caller's
//! colour view. It is the default and its output is pinned by a pixel-diff test. `Hd` is the 117 HD
//! port (`docs/plans/117HD-PORT.md`): the scene renders into an HDR offscreen target and a
//! resolve pass writes the displayable result. Phase 0 only provides that frame graph; the HD scene
//! shading itself arrives with the later phases.

use osrs_hd::environment::Lighting;

/// Which renderer draws the frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RenderProfile {
    /// RuneLite GPU plugin parity (default).
    #[default]
    Reference,
    /// 117 HD port.
    Hd,
}

impl RenderProfile {
    pub const ALL: [Self; 2] = [Self::Reference, Self::Hd];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Reference => "Reference (RuneLite)",
            Self::Hd => "HD (117 HD port)",
        }
    }
}

/// Settings of the HD resolve pass. Growing into the plugin's tone mapping / colour filter config.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HdParams {
    /// Linear exposure applied before the display transform.
    pub exposure: f32,
    /// Strength of the phase-0 placeholder vignette (`0` = none): proof that the resolve pass runs.
    pub placeholder_vignette: f32,
}

impl Default for HdParams {
    fn default() -> Self {
        Self {
            exposure: 1.0,
            placeholder_vignette: 0.35,
        }
    }
}

/// Lighting of the HD scene shader: the current area environment plus the shading options.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HdLighting {
    /// Ambient and directional light, fog, sky and water colours of the current environment
    /// (`osrs_hd::environment`).
    pub lighting: Lighting,
    pub normal_mapping: bool,
    /// Parallax occlusion mapping for materials with displacement maps (RLHD default: on).
    pub parallax_mapping: bool,
    /// Animation clock of the water, caustics and scrolling textures, in seconds.
    pub elapsed_seconds: f32,
    /// Debug output instead of the lit scene (`DISPLAY_*` of the plugin): 0 off, 1 base colour,
    /// 2 UV, 3 normals, 4 tangent, 5 lighting.
    pub debug_view: u32,
}

impl Default for HdLighting {
    fn default() -> Self {
        Self::overworld()
    }
}

impl HdLighting {
    /// The `OVERWORLD` environment's look (the default), for headless renders without an
    /// environment table: a white sun 52 degrees above the south-west horizon, blue ambient light, no fog.
    pub fn overworld() -> Self {
        let values = osrs_hd::environment::EnvValues {
            ambient_color: osrs_hd::environment::srgb_to_linear3([0.592, 0.729, 1.0]),
            ambient_strength: 1.0,
            directional_color: [1.0; 3],
            directional_strength: 4.0,
            shadow_angles: osrs_hd::environment::sun_angles(52.0, 235.0),
            fog_depth: 6.5,
            ..osrs_hd::environment::Environment::fallback().values
        };
        let settings = osrs_hd::environment::EnvironmentSettings::default();
        let mut values = values;
        values.fog_color = settings.default_sky_rgb();
        values.water_color = settings.default_sky_rgb();
        Self {
            lighting: Lighting::from_values(&values, &settings, 50.0, false, true, 0.0),
            normal_mapping: true,
            parallax_mapping: true,
            elapsed_seconds: 0.0,
            debug_view: 0,
        }
    }
}
