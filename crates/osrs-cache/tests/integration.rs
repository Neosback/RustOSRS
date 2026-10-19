//! Single integration-test binary for `osrs-cache`: one link and one process launch instead of one
//! per file. Select a former target with `cargo test -p osrs-cache --test integration <module>::`.

#[path = "m3_decode_fixtures.rs"]
mod m3_decode_fixtures;
#[path = "m3_fuzz_smoke.rs"]
mod m3_fuzz_smoke;
#[path = "m4_model_fuzz_smoke.rs"]
mod m4_model_fuzz_smoke;
#[path = "m4_target_probe.rs"]
mod m4_target_probe;
#[path = "npc_import.rs"]
mod npc_import;
