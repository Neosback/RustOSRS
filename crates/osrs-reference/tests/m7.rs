//! Bundled M7 normals, lighting, and scene finalization parity suite.

#[path = "m7/m7_base_normals.rs"]
mod m7_base_normals;
#[path = "m7/m7_composite_admission.rs"]
mod m7_composite_admission;
#[path = "m7/m7_identity_threading.rs"]
mod m7_identity_threading;
#[path = "m7/m7_initial_scene_finalization.rs"]
mod m7_initial_scene_finalization;
#[path = "m7/m7_lighting.rs"]
mod m7_lighting;
#[path = "m7/m7_normal_merge.rs"]
mod m7_normal_merge;
#[path = "m7/m7_static_entity_lifecycle.rs"]
mod m7_static_entity_lifecycle;
