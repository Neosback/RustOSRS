//! Single integration-test binary for `osrs-hd`: one link and one process launch instead of one
//! per file. Select a former target with `cargo test -p osrs-hd --test integration <module>::`.

#[path = "data.rs"]
mod data;
#[path = "environments.rs"]
mod environments;
#[path = "material_replacement_parity.rs"]
mod material_replacement_parity;
#[path = "resolve.rs"]
mod resolve;
#[path = "themes.rs"]
mod themes;
#[path = "tile_override_real_data.rs"]
mod tile_override_real_data;
