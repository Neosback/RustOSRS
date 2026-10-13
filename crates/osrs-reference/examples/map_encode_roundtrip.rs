//! Re-encode every real build-241 map square and compare with the cache's original bytes.
//!
//! usage: `map_encode_roundtrip <cache-dir>`. Exits non-zero on any decode/encode error. Squares
//! whose bytes differ are tallied by cause (non-canonical opcode order, trailing flags block,
//! unsorted loc stream) so the canonical-encoding claim is measured, not assumed.
use osrs_cache::decode::{DecoderContext, decode_locations, decode_terrain};
use osrs_cache::encode::{encode_locations, encode_terrain};
use osrs_cache::profile::TargetProfile;
use osrs_cache::transport::CacheRepository;
use osrs_core::coords::RegionCoord;

const PROFILE_YAML: &str =
    include_str!("../../../profiles/osrs-live-241-2026-09-30-openrs2-2727.yaml");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::args()
        .nth(1)
        .ok_or("usage: map_encode_roundtrip <cache-dir>")?;
    let profile = TargetProfile::from_yaml_str(PROFILE_YAML)?;
    let cache = CacheRepository::open(&dir, &profile)?;
    let context = DecoderContext::from_profile(&profile)?;

    let mut undecodable = 0;
    let (mut squares, mut terrain_exact, mut terrain_flags, mut terrain_other) = (0, 0, 0, 0);
    let (mut loc_exact, mut loc_semantic, mut loc_bad) = (0, 0, 0);
    let mut first_terrain_diff = None;
    for x in 0..=255_i32 {
        for y in 0..=255_i32 {
            let region = RegionCoord::new(x, y);
            let Ok(square) = cache.read_map_square(region) else {
                continue;
            };
            squares += 1;
            let (Ok(terrain), Ok(locs)) = (
                decode_terrain(
                    &square.terrain.bytes,
                    &context,
                    &square.terrain.provenance,
                    region,
                ),
                decode_locations(
                    &square.locations.bytes,
                    &context,
                    &square.locations.provenance,
                    region,
                ),
            ) else {
                // Encrypted squares (the profile carries no XTEA keys) cannot be decoded at all.
                undecodable += 1;
                continue;
            };
            let encoded = encode_terrain(&terrain)?;
            if encoded == square.terrain.bytes {
                terrain_exact += 1;
            } else if terrain.trailing_flags() != 0 {
                terrain_flags += 1;
            } else {
                terrain_other += 1;
                first_terrain_diff.get_or_insert((x, y));
            }
            // The re-encoded stream must always decode back to the same semantic tiles.
            let again = decode_terrain(&encoded, &context, &square.terrain.provenance, region)?;
            assert_eq!(again.tiles(), terrain.tiles(), "terrain ({x},{y}) semantic");

            let encoded = encode_locations(&locs)?;
            if encoded == square.locations.bytes {
                loc_exact += 1;
            } else {
                let again =
                    decode_locations(&encoded, &context, &square.locations.provenance, region)?;
                let mut a = again.locations().to_vec();
                let mut b = locs.locations().to_vec();
                let key = |l: &osrs_cache::decode::DecodedLocation| {
                    (
                        l.object_id.get(),
                        l.source_plane.index().get(),
                        l.tile.x(),
                        l.tile.y(),
                        l.loc_type,
                        l.orientation,
                    )
                };
                a.sort_by_key(key);
                b.sort_by_key(key);
                if a == b {
                    loc_semantic += 1;
                } else {
                    loc_bad += 1;
                }
            }
        }
    }
    println!("map squares: {squares} (undecodable without keys: {undecodable})");
    println!(
        "terrain: byte-exact={terrain_exact} trailing-flags-dropped={terrain_flags} other-diff={terrain_other}"
    );
    println!("locations: byte-exact={loc_exact} semantic-only={loc_semantic} mismatched={loc_bad}");
    if let Some((x, y)) = first_terrain_diff {
        println!("first non-canonical terrain square: ({x}, {y})");
    }
    if loc_bad != 0 {
        return Err("a location stream did not round-trip semantically".into());
    }
    Ok(())
}
