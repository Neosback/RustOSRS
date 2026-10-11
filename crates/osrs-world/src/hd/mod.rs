//! 117 HD extraction: the render-profile-specific half of scene extraction.
//!
//! [`HdContext`] holds the resolved plugin data (materials, ground materials, tile and model
//! overrides) once per process; [`procedural`] ports `ProceduralGenerator`;
//! [`terrain`] and [`model`] turn terrain tiles and loc models into the HD vertex stream.

pub mod extract;
pub mod model;
pub mod procedural;
pub mod terrain;

use osrs_hd::{
    Gamevals, HdData, HdError,
    materials::MaterialSet,
    settings::HdSettings,
    tiles::{Areas, GroundMaterials, TileOverrides, WaterTypes},
};
use std::path::{Path, PathBuf};

/// Where the vendored plugin data lives in the source tree.
pub fn default_assets_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("RUSTOSRS_HD_ASSETS") {
        return PathBuf::from(dir);
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/117hd")
}

/// Resolved 117 HD data for one settings combination.
pub struct HdContext {
    pub settings: HdSettings,
    pub data: HdData,
    pub materials: MaterialSet,
    pub grounds: GroundMaterials,
    pub water: WaterTypes,
    pub areas: Areas,
    pub tile_overrides: TileOverrides,
    pub models: model::ModelOverrides,
}

impl HdContext {
    /// Load `assets` (the `assets/117hd` directory) and resolve it under `settings`.
    /// `vanilla_textures` are the texture ids the cache provides.
    pub fn load(
        assets: &Path,
        settings: HdSettings,
        vanilla_textures: impl IntoIterator<Item = i32>,
    ) -> Result<Self, HdError> {
        Ok(Self::new(HdData::load(assets)?, settings, vanilla_textures))
    }

    pub fn new(
        data: HdData,
        settings: HdSettings,
        vanilla_textures: impl IntoIterator<Item = i32>,
    ) -> Self {
        let materials = MaterialSet::build(&data, &settings, vanilla_textures);
        let grounds = GroundMaterials::new(&data, &materials);
        let water = WaterTypes::new(&data);
        let areas = Areas::new(&data);
        let tile_overrides = TileOverrides::new(&data, &grounds, &water, &|id| {
            Gamevals::resolve(&data.gamevals.objects, id)
        });
        let models = model::ModelOverrides::with_settings(&data, &materials, &settings);
        Self {
            settings,
            data,
            materials,
            grounds,
            water,
            areas,
            tile_overrides,
            models,
        }
    }
}
