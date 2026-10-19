//! Single integration-test binary for `osrs-world`: one link and one process launch instead of one
//! per file. Select a former target with `cargo test -p osrs-world --test integration <module>::`.

#[path = "collision_projection.rs"]
mod collision_projection;
#[path = "hd_environment_pixels.rs"]
mod hd_environment_pixels;
#[path = "hd_extract.rs"]
mod hd_extract;
#[path = "hd_profile_pixels.rs"]
mod hd_profile_pixels;
#[path = "hd_shadow_pixels.rs"]
mod hd_shadow_pixels;
#[path = "hd_water_pixels.rs"]
mod hd_water_pixels;
#[path = "lumbridge_terrain_cache.rs"]
mod lumbridge_terrain_cache;
#[path = "lumbridge_world_scene.rs"]
mod lumbridge_world_scene;
#[path = "object_catalog_real.rs"]
mod object_catalog_real;
#[path = "path_height_projection.rs"]
mod path_height_projection;
#[path = "reference_profile_pixels.rs"]
mod reference_profile_pixels;
#[path = "worldmap_cache.rs"]
mod worldmap_cache;
