//! Canonical OSRS semantic value types and pure algorithms.
//!
//! This crate is deliberately independent of cache transport, scene ownership,
//! GPU rendering, and editor UI concerns. M2 introduces the reusable semantic
//! foundation in bounded slices so later cache/scene/render layers consume typed
//! canonical values rather than redefining them.

pub mod coordinate_math;
pub mod coords;
pub mod definitions;
pub mod floor_color;
pub mod ids;
pub mod lighting;
pub mod model;
pub mod model_construction;
pub mod normals;
pub mod orientation;
pub mod provenance;
pub mod static_entity;
