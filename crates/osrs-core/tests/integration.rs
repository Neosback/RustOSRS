//! Single integration-test binary for `osrs-core`: one link and one process launch instead of one
//! per file. Select a former target with `cargo test -p osrs-core --test integration <module>::`.

#[path = "entity_effect_runtime.rs"]
mod entity_effect_runtime;
#[path = "m4_fixture_manifest.rs"]
mod m4_fixture_manifest;
#[path = "m4_instance_transform.rs"]
mod m4_instance_transform;
