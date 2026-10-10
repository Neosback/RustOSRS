//! Errors for cache-to-scene world assembly.

use osrs_cache::{
    decode::{DecodeError, DecoderContextError},
    object_model::ObjectModelResolveError,
    profile::TargetProfileError,
    transport::{CacheError, CacheRepositoryOpenError},
};
use osrs_scene::{
    SceneGridError, terrain_build::TerrainColorError, terrain_load::TerrainLoadError,
};
use std::{error::Error, fmt};

/// Any failure while assembling a scene from the cache.
#[derive(Debug)]
pub enum WorldError {
    Profile(TargetProfileError),
    Open(CacheRepositoryOpenError),
    Context(DecoderContextError),
    Cache(CacheError),
    Decode(DecodeError),
    TerrainLoad(TerrainLoadError),
    TerrainColor(TerrainColorError),
    Scene(SceneGridError),
    ModelResolve(ObjectModelResolveError),
    Contour(osrs_core::contour::ContourGroundError),
    Lighting(osrs_core::lighting::LightingError),
    Placement(String),
}

impl fmt::Display for WorldError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Profile(error) => write!(formatter, "target profile: {error}"),
            Self::Open(error) => write!(formatter, "opening cache: {error}"),
            Self::Context(error) => write!(formatter, "decoder context: {error}"),
            Self::Cache(error) => write!(formatter, "cache read: {error}"),
            Self::Decode(error) => write!(formatter, "decode: {error:?}"),
            Self::TerrainLoad(error) => write!(formatter, "terrain load: {error}"),
            Self::TerrainColor(error) => write!(formatter, "terrain color: {error}"),
            Self::Scene(error) => write!(formatter, "scene: {error}"),
            Self::ModelResolve(error) => write!(formatter, "model resolve: {error}"),
            Self::Contour(error) => write!(formatter, "contour: {error}"),
            Self::Lighting(error) => write!(formatter, "lighting: {error}"),
            Self::Placement(detail) => write!(formatter, "placement: {detail}"),
        }
    }
}

impl Error for WorldError {}

macro_rules! impl_from {
    ($($variant:ident($source:ty)),* $(,)?) => {
        $(impl From<$source> for WorldError {
            fn from(value: $source) -> Self {
                Self::$variant(value)
            }
        })*
    };
}

impl_from!(
    Profile(TargetProfileError),
    Open(CacheRepositoryOpenError),
    Context(DecoderContextError),
    Cache(CacheError),
    Decode(DecodeError),
    TerrainLoad(TerrainLoadError),
    TerrainColor(TerrainColorError),
    Scene(SceneGridError),
    ModelResolve(ObjectModelResolveError),
    Contour(osrs_core::contour::ContourGroundError),
    Lighting(osrs_core::lighting::LightingError),
);
