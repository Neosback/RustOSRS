//! Decode every definition family and every map square in the real build-241 cache with the
//! production decoders. Any failure means a decoder disagrees with real data.
use osrs_cache::decode::{
    ArchiveFileProvenance, DecoderContext, decode_floor_overlay, decode_floor_underlay,
    decode_locations, decode_sequence_definition, decode_terrain, decode_texture_definition,
    decode_varbit, decode_varp,
};
use osrs_cache::profile::TargetProfile;
use osrs_cache::transport::CacheRepository;
use osrs_core::coords::RegionCoord;
use osrs_core::ids::{FloorOverlayId, FloorUnderlayId, SequenceId, TextureId, VarbitId, VarpId};
use std::collections::BTreeMap;
use std::env;

const PROFILE_YAML: &str =
    include_str!("../../../profiles/osrs-live-241-2026-09-30-openrs2-2727.yaml");

#[derive(Default)]
struct Tally {
    ok: usize,
    failed: usize,
    errors: BTreeMap<String, Vec<u32>>,
}

impl Tally {
    fn record<T, E: std::fmt::Debug>(&mut self, id: u32, result: Result<T, E>) {
        match result {
            Ok(_) => self.ok += 1,
            Err(error) => {
                self.failed += 1;
                let key: String = format!("{error:?}").chars().take(110).collect();
                let ids = self.errors.entry(key).or_default();
                if ids.len() < 5 {
                    ids.push(id);
                }
            }
        }
    }

    fn report(&self, name: &str) {
        println!("{name}: ok={} failed={}", self.ok, self.failed);
        for (error, ids) in &self.errors {
            println!("    {error}  e.g. ids {ids:?}");
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = env::args()
        .nth(1)
        .ok_or("usage: cache_decode_sweep <cache-dir>")?;
    let profile = TargetProfile::from_yaml_str(PROFILE_YAML)?;
    let cache = CacheRepository::open(&dir, &profile)?;
    let context = DecoderContext::from_profile(&profile)?;

    let mut tally = Tally::default();
    for (id, file) in cache.read_group_files(2, 1)? {
        let src = ArchiveFileProvenance::new(2, 1, Some(id));
        tally.record(
            id,
            decode_floor_underlay(FloorUnderlayId::new(id), &file.bytes, &context, &src),
        );
    }
    tally.report("underlays");

    let mut tally = Tally::default();
    for (id, file) in cache.read_group_files(2, 4)? {
        let src = ArchiveFileProvenance::new(2, 4, Some(id));
        tally.record(
            id,
            decode_floor_overlay(FloorOverlayId::new(id), &file.bytes, &context, &src),
        );
    }
    tally.report("overlays");

    let mut tally = Tally::default();
    for (id, file) in cache.read_group_files(9, 0)? {
        let src = ArchiveFileProvenance::new(9, 0, Some(id));
        tally.record(
            id,
            decode_texture_definition(TextureId::new(id), &file.bytes, &context, &src),
        );
    }
    tally.report("textures");

    let mut tally = Tally::default();
    for (id, file) in cache.read_group_files(2, 12)? {
        let src = ArchiveFileProvenance::new(2, 12, Some(id));
        tally.record(
            id,
            decode_sequence_definition(SequenceId::new(id), &file.bytes, &context, &src),
        );
    }
    tally.report("sequences");

    let mut tally = Tally::default();
    for (id, file) in cache.read_group_files(2, 14)? {
        let src = ArchiveFileProvenance::new(2, 14, Some(id));
        tally.record(
            id,
            decode_varbit(VarbitId::new(id), &file.bytes, &context, &src),
        );
    }
    tally.report("varbits");

    let mut tally = Tally::default();
    for (id, file) in cache.read_group_files(2, 16)? {
        let src = ArchiveFileProvenance::new(2, 16, Some(id));
        tally.record(
            id,
            decode_varp(VarpId::new(id), &file.bytes, &context, &src),
        );
    }
    tally.report("varps");

    let (mut squares, mut missing) = (0_usize, 0_usize);
    let mut terrain = Tally::default();
    let mut locations = Tally::default();
    let mut total_locs = 0_usize;
    for x in 0..=255_i32 {
        for y in 0..=255_i32 {
            let region = RegionCoord::new(x, y);
            let Ok(square) = cache.read_map_square(region) else {
                missing += 1;
                continue;
            };
            squares += 1;
            let key = (x * 256 + y) as u32;
            if std::env::var("SQUARE").is_ok() && key == 25287 {
                println!(
                    "square 98,199 terrain bytes {:?} locs bytes {:?}",
                    square.terrain.bytes, square.locations.bytes
                );
            }
            terrain.record(
                key,
                decode_terrain(
                    &square.terrain.bytes,
                    &context,
                    &square.terrain.provenance,
                    region,
                ),
            );
            match decode_locations(
                &square.locations.bytes,
                &context,
                &square.locations.provenance,
                region,
            ) {
                Ok(locs) => {
                    total_locs += locs.len();
                    locations.ok += 1;
                }
                Err(error) => locations.record::<(), _>(key, Err(error)),
            }
        }
    }
    println!("map squares: present={squares} absent={missing} total_locs={total_locs}");
    terrain.report("terrain");
    locations.report("locations");
    Ok(())
}
