//! Render profiles.
//!
//! `Reference` is the RuneLite-parity renderer: one scene pass resolving straight into the caller's
//! colour view. It is the default and its output is pinned by a pixel-diff test. `Hd` is the 117 HD
//! port (`docs/plans/117HD-PORT.md`): the scene renders into an HDR offscreen target and a
//! resolve pass writes the displayable result. Phase 0 only provides that frame graph; the HD scene
//! shading itself arrives with the later phases.

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

/// Lighting of the HD scene shader. Placeholder values (an overworld-like sun) until phase 3 ports
/// the per-area environments.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HdLighting {
    /// Unit vector from the scene towards the light (game space: `y` is down).
    pub light_dir: [f32; 3],
    /// Linear directional light colour and strength.
    pub light_color: [f32; 3],
    pub light_strength: f32,
    pub ambient_color: [f32; 3],
    pub ambient_strength: f32,
    pub water_color: [f32; 3],
    pub normal_mapping: bool,
    /// Debug output instead of the lit scene (`DISPLAY_*` of the plugin): 0 off, 1 base colour,
    /// 2 UV, 3 normals, 4 tangent, 5 lighting.
    pub debug_view: u32,
}

impl Default for HdLighting {
    fn default() -> Self {
        // Sun 52 degrees above the horizon from the vanilla light's side; the `OVERWORLD`
        // environment's ambient hue (`#97baff`) lightened, and strengths picked to look right without
        // the plugin's tone mapping.
        let (altitude, azimuth) = (52.0_f32.to_radians(), 235.0_f32.to_radians());
        let light_dir = [
            -altitude.cos() * azimuth.sin(),
            -altitude.sin(),
            -altitude.cos() * azimuth.cos(),
        ];
        Self {
            light_dir,
            light_color: [1.0, 1.0, 1.0],
            light_strength: 1.6,
            ambient_color: [0.6, 0.72, 1.0],
            ambient_strength: 0.9,
            water_color: [0.14, 0.82, 1.0],
            normal_mapping: true,
            debug_view: 0,
        }
    }
}
