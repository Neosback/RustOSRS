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
pub mod height_map;
pub mod material;
pub mod model_info;
pub mod packed_vertex;
pub mod packet;
pub mod static_scene;

#[cfg(target_arch = "wasm32")]
mod webgl;

pub use draw::{DrawRange, DrawStats, filter_draw_ranges};
pub use height_map::HeightMap;
pub use material::{Material, decode_material};
pub use model_info::{ModelInfo, ModelInfoDrawCommand, create_model_info_texture_data};
pub use packed_vertex::{PackedVertex, VertexInput};
pub use packet::{RendererPacketError, validate_draw_ranges, validate_geometry};
pub use static_scene::{StaticMapState, validate_static_scene_packet};

#[cfg(target_arch = "wasm32")]
pub use webgl::RustWebGlRenderer;

/// Increment only for a breaking renderer packet/layout change.
pub const RENDERER_ABI_VERSION: u32 = 14;
