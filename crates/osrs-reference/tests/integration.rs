//! Single integration-test binary for `osrs-reference`: one link and one process launch instead of one
//! per file. Select a former target with `cargo test -p osrs-reference --test integration <module>::`.

#[path = "m10_model_data_contour.rs"]
mod m10_model_data_contour;
#[path = "m5_fixture_runner.rs"]
mod m5_fixture_runner;
#[path = "m5_normalized_model_fixtures.rs"]
mod m5_normalized_model_fixtures;
#[path = "m7.rs"]
mod m7;
#[path = "m8.rs"]
mod m8;
#[path = "m9_semantic_closure.rs"]
mod m9_semantic_closure;
#[path = "terrain_oracle_diff.rs"]
mod terrain_oracle_diff;
