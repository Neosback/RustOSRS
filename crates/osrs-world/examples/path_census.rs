//! Export empirical path, overlay shape (`0..=11`), companion underlay, and bare shoulder underlay
//! statistics across the cache.
//!
//! `RUSTOSRS_TARGET_CACHE_DIR=... cargo run --release -p osrs-world --example path_census -- <out.json>`

use osrs_core::coords::RegionCoord;
use osrs_scene::path_catalog::PathCatalog;
use osrs_world::{WorldDefinitions, load_floor_palette};
use serde_json::{Value, json};
use std::collections::BTreeMap as JsonMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::var("RUSTOSRS_TARGET_CACHE_DIR")?;
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "paths.json".to_owned());
    let palette = load_floor_palette(&dir)?;
    let definitions = WorldDefinitions::open(&dir)?;
    let mut catalog = PathCatalog::default();
    let mut regions = 0_u32;

    for x in 0..=120 {
        for y in 0..=220 {
            let Ok(Some(map)) = definitions.region(RegionCoord::new(x, y)) else {
                continue;
            };
            regions += 1;
            catalog.observe_region(&map.terrain);
        }
    }

    let mut overlays = JsonMap::new();
    for (id, profile) in &catalog.overlays {
        let swatch = palette.overlays.iter().find(|s| s.id == *id);
        let companions: Vec<Value> = profile
            .top_companions(5)
            .into_iter()
            .map(|(u, n)| json!({ "underlay": u, "tiles": n }))
            .collect();
        let shoulders: Vec<Value> = profile
            .top_shoulders(5)
            .into_iter()
            .map(|(u, n)| json!({ "underlay": u, "tiles": n }))
            .collect();
        overlays.insert(
            id.to_string(),
            json!({
                "id": id,
                "rgb": swatch.map(|s| s.rgb),
                "primary_rgb": swatch.map(|s| s.primary_rgb),
                "secondary_rgb": swatch.and_then(|s| s.secondary_rgb),
                "texture": swatch.and_then(|s| s.texture),
                "hide_underlay": swatch.map(|s| s.hide_underlay),
                "total_tiles": profile.total_tiles,
                "flat_tiles": profile.flat_tiles,
                "shaped_tiles": profile.shaped_tiles,
                "shape_histogram": profile.shape_histogram,
                "companion_underlays": companions,
                "shoulder_underlays": shoulders,
            }),
        );
    }

    let doc = json!({
        "format": "rustosrs-path-catalog",
        "version": 1,
        "regions_scanned": regions,
        "overlays": overlays,
    });
    std::fs::write(&out, serde_json::to_vec_pretty(&doc)?)?;
    println!(
        "Scanned {regions} regions, {} overlays -> {out}",
        catalog.overlays.len()
    );
    Ok(())
}
