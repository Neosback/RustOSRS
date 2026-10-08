use osrs_cache::decode::{
    ArchiveFileProvenance, DecoderContext, decode_floor_overlay, decode_floor_underlay,
    decode_locations, decode_object_definition, decode_sequence_definition, decode_terrain,
    decode_texture_definition, decode_varbit, decode_varp,
};
use osrs_cache::profile::TargetProfile;
use osrs_core::coords::RegionCoord;
use osrs_core::ids::{
    FloorOverlayId, FloorUnderlayId, ObjectId, SequenceId, TextureId, VarbitId, VarpId,
};
use std::panic::{AssertUnwindSafe, catch_unwind};

const TARGET_PROFILE_YAML: &str =
    include_str!("../../../profiles/osrs-live-241-2026-09-30-openrs2-2727.yaml");
const GENERATED_CASES: usize = 512;
const MAX_CASE_BYTES: usize = 256;

fn context() -> Result<DecoderContext, Box<dyn std::error::Error>> {
    let profile = TargetProfile::from_yaml_str(TARGET_PROFILE_YAML)?;
    Ok(DecoderContext::from_profile(&profile)?)
}

fn next_u64(state: &mut u64) -> u64 {
    let mut value = *state;
    value ^= value << 13;
    value ^= value >> 7;
    value ^= value << 17;
    *state = value;
    value
}

fn exercise_all(bytes: &[u8], context: &DecoderContext) {
    let definition_source = ArchiveFileProvenance::new(2, 6, Some(1));
    let floor_source = ArchiveFileProvenance::new(2, 4, Some(1));
    let texture_source = ArchiveFileProvenance::new(9, 0, Some(1));
    let sequence_source = ArchiveFileProvenance::new(2, 12, Some(1));
    let varbit_source = ArchiveFileProvenance::new(2, 14, Some(1));
    let varp_source = ArchiveFileProvenance::new(2, 16, Some(1));
    let terrain_source = ArchiveFileProvenance::new(5, 12_850, Some(0));
    let loc_source = ArchiveFileProvenance::new(5, 12_850, Some(1));

    let result = catch_unwind(AssertUnwindSafe(|| {
        let _ = decode_object_definition(ObjectId::new(1), bytes, context, &definition_source);
        let _ = decode_floor_underlay(FloorUnderlayId::new(1), bytes, context, &floor_source);
        let _ = decode_floor_overlay(FloorOverlayId::new(1), bytes, context, &floor_source);
        let _ = decode_texture_definition(TextureId::new(1), bytes, context, &texture_source);
        let _ = decode_sequence_definition(SequenceId::new(1), bytes, context, &sequence_source);
        let _ = decode_varbit(VarbitId::new(1), bytes, context, &varbit_source);
        let _ = decode_varp(VarpId::new(1), bytes, context, &varp_source);
        let _ = decode_terrain(bytes, context, &terrain_source, RegionCoord::new(50, 50));
        let _ = decode_locations(bytes, context, &loc_source, RegionCoord::new(50, 50));
    }));

    assert!(result.is_ok(), "M3 decoder panicked for input {bytes:02x?}");
}

#[test]
fn arbitrary_bounded_bytes_never_panic_m3_decoders() -> Result<(), Box<dyn std::error::Error>> {
    let context = context()?;

    for corpus in [
        Vec::new(),
        vec![0],
        vec![0xff],
        vec![0; MAX_CASE_BYTES],
        vec![0xff; MAX_CASE_BYTES],
        (0..=255).collect::<Vec<u8>>(),
    ] {
        exercise_all(&corpus, &context);
    }

    let mut state = 0x5255_5354_4f53_5253u64;
    for _ in 0..GENERATED_CASES {
        let length = (next_u64(&mut state) as usize) % (MAX_CASE_BYTES + 1);
        let mut bytes = Vec::with_capacity(length);
        for _ in 0..length {
            bytes.push(next_u64(&mut state) as u8);
        }
        exercise_all(&bytes, &context);
    }

    Ok(())
}
