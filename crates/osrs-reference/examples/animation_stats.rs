//! Stats: how animated objects animate (legacy frame sequences vs skeletal) in the real cache.
use osrs_cache::decode::{ArchiveFileProvenance, DecoderContext, decode_object_definition, decode_sequence_definition};
use osrs_cache::profile::TargetProfile;
use osrs_cache::transport::CacheRepository;
use osrs_core::ids::{ObjectId, SequenceId};
use std::collections::HashMap;

const PROFILE_YAML: &str = include_str!("../../../profiles/osrs-live-241-2026-09-30-openrs2-2727.yaml");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::args().nth(1).ok_or("cache dir")?;
    let profile = TargetProfile::from_yaml_str(PROFILE_YAML)?;
    let cache = CacheRepository::open(&dir, &profile)?;
    let context = DecoderContext::from_profile(&profile)?;
    let mut sequences = HashMap::new();
    for (id, file) in cache.read_group_files(2, 12)? {
        let src = ArchiveFileProvenance::new(2, 12, Some(id));
        sequences.insert(id, decode_sequence_definition(SequenceId::new(id), &file.bytes, &context, &src)?);
    }
    let (mut legacy, mut skeletal, mut both, mut none) = (0, 0, 0, 0);
    for (id, file) in cache.read_group_files(2, 6)? {
        let src = ArchiveFileProvenance::new(2, 6, Some(id));
        let def = decode_object_definition(ObjectId::new(id), &file.bytes, &context, &src)?;
        let Some(animation) = def.animation else { continue };
        let Some(seq) = sequences.get(&animation.get()) else { none += 1; continue };
        match (!seq.frame_ids.is_empty(), seq.skeletal_animation.is_some()) {
            (true, false) => legacy += 1,
            (false, true) => skeletal += 1,
            (true, true) => both += 1,
            (false, false) => none += 1,
        }
    }
    println!("animated object defs: legacy-frames={legacy} skeletal={skeletal} both={both} neither={none}");
    let total = sequences.len();
    let skel = sequences.values().filter(|s| s.skeletal_animation.is_some()).count();
    println!("sequences: total={total} skeletal={skel}");
    Ok(())
}
