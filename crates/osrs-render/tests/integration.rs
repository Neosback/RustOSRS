//! Single integration-test binary for `osrs-render`: one link and one process launch instead of one
//! per file. Select a former target with `cargo test -p osrs-render --test integration <module>::`.

#[path = "hd_material_binding.rs"]
mod hd_material_binding;
#[path = "hd_phase0.rs"]
mod hd_phase0;
#[path = "m10_classification.rs"]
mod m10_classification;
#[path = "m10_extraction.rs"]
mod m10_extraction;
#[path = "m10_face_preparation.rs"]
mod m10_face_preparation;
#[path = "m10_material_texture.rs"]
mod m10_material_texture;
#[path = "m10_zones.rs"]
mod m10_zones;
#[path = "theme_shaders.rs"]
mod theme_shaders;
