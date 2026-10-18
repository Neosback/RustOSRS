//! Canonical OSRS semantic value types and pure algorithms.
//!
//! This crate is deliberately independent of cache transport, scene ownership,
//! GPU rendering, and editor UI concerns. M2 introduces the reusable semantic
//! foundation in bounded slices so later cache/scene/render layers consume typed
//! canonical values rather than redefining them.

pub mod actor_pose;
pub mod actor_snapshot;
pub mod animation;
pub mod animation_pose;
pub mod audio;
pub mod color_palette;
pub mod contour;
pub mod coordinate_math;
pub mod coords;
pub mod definitions;
pub mod dynamic_model;
pub mod entity;
pub mod entity_effect;
pub mod floor_color;
pub mod ids;
pub mod lighting;
pub mod model;
pub mod model_construction;
pub mod model_identity;
pub mod morph;
pub mod normals;
pub mod object_names;
pub mod orientation;
pub mod player;
pub mod provenance;
pub mod scenario;
pub mod sequence_replacement;
pub mod static_entity;
pub mod trig;
