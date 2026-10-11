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
