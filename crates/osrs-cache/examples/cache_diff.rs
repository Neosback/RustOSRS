//! Compare two caches group by group (reference-table CRCs): what an update changed, per index.
//!
//! `cargo run --release -p osrs-cache --example cache_diff -- <older cache dir> <newer cache dir>`
//! Both must match a bundled profile (`osrs_cache::registry`). A new layout shows up here first: an index
//! with many changed groups is where to look for a format change.

use osrs_cache::registry::bundled_profiles;
use osrs_cache::transport::CacheRepository;
use std::collections::BTreeMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let old_dir = args.next().ok_or("usage: cache_diff <older> <newer>")?;
    let new_dir = args.next().ok_or("usage: cache_diff <older> <newer>")?;
    let profiles = bundled_profiles()?;
    let (old, old_profile) = CacheRepository::open_matching(&old_dir, &profiles)?;
    let (new, new_profile) = CacheRepository::open_matching(&new_dir, &profiles)?;
    println!("{} -> {}", old_profile.id, new_profile.id);
    let names: BTreeMap<u8, &str> = [
        (0, "animations"),
        (1, "skeletons"),
        (2, "configs"),
        (3, "interfaces"),
        (4, "sound effects"),
        (5, "maps"),
        (6, "music tracks"),
        (7, "models"),
        (8, "sprites"),
        (9, "textures"),
        (10, "binary"),
        (11, "jingles"),
        (12, "clientscripts"),
        (13, "fonts"),
        (14, "music samples"),
        (15, "music patches"),
        (17, "defaults"),
        (18, "world map geography"),
        (19, "world map areas"),
        (20, "world map ground"),
        (21, "dbtables"),
        (22, "animation maya"),
        (24, "gamevals"),
    ]
    .into_iter()
    .collect();
    println!(
        "{:<22} {:>8} {:>8} {:>8} {:>8}",
        "index", "same", "changed", "added", "removed"
    );
    let mut totals = (0, 0, 0, 0);
    let mut indices = old.content_index_ids();
    indices.extend(new.content_index_ids());
    indices.sort_unstable();
    indices.dedup();
    for index in indices {
        let crcs = |repo: &CacheRepository| -> BTreeMap<u32, u32> {
            repo.group_ids(index)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|group| Some((group, repo.group_metadata(index, group).ok()?.crc)))
                .collect()
        };
        let (a, b) = (crcs(&old), crcs(&new));
        let (mut same, mut changed, mut added) = (0, 0, 0);
        for (group, crc) in &b {
            match a.get(group) {
                Some(old_crc) if old_crc == crc => same += 1,
                Some(_) => changed += 1,
                None => added += 1,
            }
        }
        let removed = a.keys().filter(|group| !b.contains_key(group)).count();
        println!(
            "{:<22} {:>8} {:>8} {:>8} {:>8}",
            format!("{index} {}", names.get(&index).unwrap_or(&"")),
            same,
            changed,
            added,
            removed
        );
        totals = (
            totals.0 + same,
            totals.1 + changed,
            totals.2 + added,
            totals.3 + removed,
        );
    }
    println!(
        "{:<22} {:>8} {:>8} {:>8} {:>8}",
        "total", totals.0, totals.1, totals.2, totals.3
    );
    Ok(())
}
