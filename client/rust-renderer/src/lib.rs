//! Pure Rust rendering core for RustOSRS.
//!
//! The migration boundary is intentionally narrow. TypeScript/cache code may
//! continue to produce decoded scene data while Rust owns packed-vertex
//! semantics, draw planning and the WebGL2 GPU lifecycle.
//!
//! The public boundary is a versioned renderer packet. PicoGL objects,
//! TypeScript class instances, React state and networking objects never cross
//! into this crate.

pub mod draw;
pub mod draw_builder;
pub mod face_builder;
pub mod geometry_builder;
pub mod height_map;
pub mod material;
pub mod model_hash;
pub mod model_info;
pub mod model_lighting;
pub mod model_transform;
pub mod packed_vertex;
pub mod packet;
pub mod static_scene;
pub mod texture_mapper;

#[cfg(target_arch = "wasm32")]
mod webgl;

pub use draw::{DrawRange, DrawStats, filter_draw_ranges};
pub use draw_builder::{PreparedDrawList, prepare_draw_list};
pub use face_builder::{FaceFilter, prepare_model_faces};
pub use geometry_builder::VertexBatchBuilder;
pub use height_map::HeightMap;
pub use material::{Material, decode_material};
pub use model_hash::{hash_model_parts, xxhash32};
pub use model_info::{ModelInfo, ModelInfoDrawCommand, create_model_info_texture_data};
pub use model_lighting::{calculate_model_normals, light_model_faces};
pub use model_transform::skin_skeletal_vertices;
pub use packed_vertex::{PackedVertex, VertexInput};
pub use packet::{RendererPacketError, validate_draw_ranges, validate_geometry};
pub use static_scene::{StaticMapState, validate_static_scene_packet};
pub use texture_mapper::compute_model_uvs;

#[cfg(target_arch = "wasm32")]
pub use geometry_builder::RustVertexBufferBuilder;
#[cfg(target_arch = "wasm32")]
pub use webgl::RustWebGlRenderer;

/// Increment only for a breaking renderer packet/layout change.
pub const RENDERER_ABI_VERSION: u32 = 26;
