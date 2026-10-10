//! Decode every build-241 object definition with the production decoder and report failures.
use osrs_cache::decode::{ArchiveFileProvenance, DecoderContext, decode_object_definition};
use osrs_cache::profile::TargetProfile;
use osrs_cache::transport::CacheRepository;
use osrs_core::ids::ObjectId;
use std::collections::BTreeMap;
use std::env;

const PROFILE_YAML: &str =
    include_str!("../../../profiles/osrs-live-241-2026-09-30-openrs2-2727.yaml");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = env::args()
        .nth(1)
        .ok_or("usage: object_decode_sweep <cache-dir>")?;
    let profile = TargetProfile::from_yaml_str(PROFILE_YAML)?;
    let cache = CacheRepository::open(&dir, &profile)?;
    let context = DecoderContext::from_profile(&profile)?;
    let files = cache.read_group_files(2, 6)?;
    let (mut ok, mut failed) = (0_usize, 0_usize);
    let mut raise_samples = 0;
    let (mut non_flat, mut contour, mut both, mut animated, mut morph, mut recolor_all, mut raised) =
        (0, 0, 0, 0, 0, 0, 0);
    let mut errors: BTreeMap<String, usize> = BTreeMap::new();
    for (id, file) in &files {
        let source = ArchiveFileProvenance::new(2, 6, Some(*id));
        match decode_object_definition(ObjectId::new(*id), &file.bytes, &context, &source) {
            Ok(definition) => {
                ok += 1;
                non_flat += usize::from(definition.non_flat_shading);
                contour += usize::from(definition.contour_clip.is_some());
                both +=
                    usize::from(definition.non_flat_shading && definition.contour_clip.is_some());
                animated += usize::from(definition.animation.is_some());
                morph += usize::from(definition.morphs.is_some());
                recolor_all += usize::from(definition.full_recolor.is_some());
                raised += usize::from(definition.ground_raise != 0);
                if definition.ground_raise != 0
                    && env::var("RAISE").is_ok()
                    && raise_samples < 40
                    && (*id % 37 == 0)
                {
                    raise_samples += 1;
                    println!(
                        "raise id={id} value={} name={:?} size={}x{} contour={:?} models={}",
                        definition.ground_raise,
                        definition.name,
                        definition.size_x,
                        definition.size_y,
                        definition.contour_clip,
                        definition.models.is_some()
                    );
                }
            }
            Err(error) => {
                failed += 1;
                println!(
                    "failed id {id}: {} bytes, {:?}",
                    file.bytes.len(),
                    error.kind()
                );
                if env::var("DUMP").is_ok() {
                    let head: Vec<String> = file
                        .bytes
                        .iter()
                        .take(usize::MAX)
                        .map(|b| format!("{b:02x}"))
                        .collect();
                    println!("  head: {}", head.join(" "));
                }
                let key = format!("{:?}", error.kind());
                *errors.entry(key.chars().take(120).collect()).or_default() += 1;
            }
        }
    }
    println!(
        "object definitions: total={} ok={ok} failed={failed}",
        files.len()
    );
    println!(
        "  non_flat_shading={non_flat} contour_clip={contour} both={both} animated={animated} morphs={morph} full_recolor={recolor_all} ground_raise!=0={raised}"
    );
    for (error, count) in errors {
        println!("{count} x {error}");
    }
    Ok(())
}
