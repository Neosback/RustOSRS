//! The plugin settings the JSON conditions can read (`HDVariables`): season, texture toggles and
//! ground blending. Upstream: `rs117.hd.utils.HDVariables` (BSD 2-Clause, see `THIRD_PARTY.md`).

use crate::expr::{Value, Variables};
use serde::{Deserialize, Serialize};

/// `SeasonalTheme`, in the plugin's ordinal order (the JSON compares `season` with these).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SeasonalTheme {
    Automatic,
    #[default]
    Summer,
    Autumn,
    Winter,
}

impl SeasonalTheme {
    pub const fn ordinal(self) -> i64 {
        self as i64
    }

    fn named(name: &str) -> Option<Self> {
        Some(match name {
            "AUTOMATIC" => Self::Automatic,
            "SUMMER" => Self::Summer,
            "AUTUMN" => Self::Autumn,
            "WINTER" => Self::Winter,
            _ => return None,
        })
    }
}

/// `VanillaShadowMode`: whether the shadows Jagex baked into models are hidden (the HD
/// plugin renders its own).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum VanillaShadowMode {
    /// Always show them.
    Show,
    /// Hide them, except where models keep them for PvM (falling crystals of the Olm fight).
    #[default]
    ShowInPvm,
    /// As `ShowInPvm`, and also turn off the dynamic shadows of such models.
    PreferInPvm,
}

impl VanillaShadowMode {
    pub const ALL: [Self; 3] = [Self::Show, Self::ShowInPvm, Self::PreferInPvm];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Show => "Always show",
            Self::ShowInPvm => "Show in PvM",
            Self::PreferInPvm => "Prefer in PvM",
        }
    }
}

/// Settings that change which materials and overrides are active. Defaults follow
/// `HdPluginConfig`, except that the seasonal theme is pinned to summer so a scene does not
/// change with the date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HdSettings {
    pub season: SeasonalTheme,
    pub seasonal_foliage: bool,
    /// "Object textures": HD replacements of the vanilla model textures.
    pub model_textures: bool,
    pub ground_textures: bool,
    pub ground_blending_colors: bool,
    pub ground_blending_textures: bool,
    pub vanilla_shadows: VanillaShadowMode,
}

impl Default for HdSettings {
    fn default() -> Self {
        Self {
            season: SeasonalTheme::Summer,
            seasonal_foliage: true,
            model_textures: true,
            ground_textures: true,
            ground_blending_colors: true,
            ground_blending_textures: true,
            vanilla_shadows: VanillaShadowMode::ShowInPvm,
        }
    }
}

impl HdSettings {
    /// `configHideFakeShadows`.
    pub const fn hide_fake_shadows(self) -> bool {
        !matches!(self.vanilla_shadows, VanillaShadowMode::Show)
    }

    /// `configGroundBlending`.
    pub const fn ground_blending(self) -> bool {
        self.ground_blending_colors || self.ground_blending_textures
    }
}

impl Variables for HdSettings {
    fn get(&self, name: &str) -> Option<Value> {
        let flag = |value: bool| Some(Value::Bool(value));
        match name {
            "season" => Some(Value::Int(self.season.ordinal())),
            "seasonalFoliage" => flag(self.seasonal_foliage),
            "modelTextures" => flag(self.model_textures),
            "groundTextures" | "textures" => flag(self.ground_textures),
            "blendingColors" => flag(self.ground_blending_colors),
            "blendingTextures" => flag(self.ground_blending_textures),
            "blending" => flag(self.ground_blending()),
            "blendingBoth" => flag(self.ground_blending_colors && self.ground_blending_textures),
            "blendingColorsOnly" => {
                flag(self.ground_blending_colors && !self.ground_blending_textures)
            }
            "blendingTexturesOnly" => {
                flag(!self.ground_blending_colors && self.ground_blending_textures)
            }
            "hdInfernalCape" => flag(true),
            "legacyInfernalCape" => flag(false),
            _ => name
                .strip_prefix("SeasonalTheme.")
                .and_then(SeasonalTheme::named)
                .map(|theme| Value::Int(theme.ordinal())),
        }
    }
}
