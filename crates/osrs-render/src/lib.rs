//! Renderer-owned CPU extraction and headless structural rendering core.
//!
//! M10 keeps this crate free of wgpu/device state. It consumes authoritative
//! semantic data, produces disposable immutable render snapshots, and preserves
//! enough provenance/metadata for exact structural verification before M11.

mod alpha;
mod classification;
mod coordinates;
mod draw_plan;
mod generation;
mod geometry;
pub mod gpu;
mod hd_geometry;
mod material;
mod mesh;
mod priority;
mod snapshot;
mod uv;
mod zone;

pub use alpha::ReferenceFaceAlpha;
pub use classification::{
    RenderClassification, RenderPath, RenderRequirement, RenderRequirements, classify_renderable,
};
pub use coordinates::{RenderCoordinateError, RenderOrigin, RenderPoint};
pub use draw_plan::{RenderDrawPlan, RenderScenePass};
pub use generation::SemanticGeneration;
pub use geometry::{
    GeometryBuilder, ModelPlacement, PackedVertex, SceneGeometry, TERRAIN_SKIP_COLOR,
    ZONE_LOCAL_UNITS, ZoneGeometry, ZoneGroup, face_uvs as vanilla_face_uvs,
};
pub use hd_geometry::{
    HD_FLAG_FLAT_NORMALS, HD_FLAG_NO_SHADOW_CAST, HD_FLAG_NO_SHADOW_RECEIVE, HD_FLAG_SHADOW_NORMAL_BIAS, HD_FLAG_TERRAIN, HD_FLAG_UPWARDS_NORMALS,
    HD_FLAG_VANILLA_UVS, HD_MATERIAL_INDEX_MASK, HD_MATERIAL_OVERLAY, HdFace, HdVertex, oct_decode,
    oct_encode,
};
pub use material::{
    MaterialHandle, MaterialTable, MaterialTableError, REFERENCE_TEXTURE_ANIMATION_UNIT,
    RenderMaterial, TextureAnimationVector,
};
pub use mesh::{RenderExtractionError, RenderMesh, RenderPlacement};
pub use priority::{
    ReferencePriorityFace, ReferencePriorityOrder, ReferencePriorityThresholds,
    prepare_reference_priority_order,
};
pub use snapshot::RenderSnapshot;
pub use uv::{
    ReferenceFaceUvs, ReferenceUv, ReferenceUvError, ReferenceUvMode, prepare_reference_face_uvs,
};
pub use zone::{
    RENDER_ZONE_SIZE_TILES, RenderTileBounds, RenderZoneError, RenderZoneKey, ZoneBuildTicket,
    ZoneInvalidationTracker,
};
