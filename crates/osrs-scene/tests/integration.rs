//! Single integration-test binary for `osrs-scene`: one link and one process launch instead of one
//! per file. Select a former target with `cargo test -p osrs-scene --test integration <module>::`.

#[path = "m6_exit_golden_scenes.rs"]
mod m6_exit_golden_scenes;
#[path = "m6_loc_placement.rs"]
mod m6_loc_placement;
#[path = "m6_placement_matrix.rs"]
mod m6_placement_matrix;
#[path = "m6_scene_insertion.rs"]
mod m6_scene_insertion;
#[path = "m6_terrain_shape_gallery.rs"]
mod m6_terrain_shape_gallery;
#[path = "m7_scene_normal_reconciliation.rs"]
mod m7_scene_normal_reconciliation;
#[path = "m8_active_morph_placement.rs"]
mod m8_active_morph_placement;
