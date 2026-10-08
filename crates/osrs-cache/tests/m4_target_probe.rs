use osrs_cache::decode::{ArchiveFileProvenance, DecoderContext, decode_model_data};
use osrs_cache::profile::TargetProfile;
use osrs_cache::transport::CacheRepository;
use osrs_core::ids::ModelId;
use osrs_core::model::ModelEncoding;
use std::collections::BTreeMap;
use std::env;

const TARGET_PROFILE_YAML: &str = include_str!(
    "../../../profiles/osrs-live-241-2026-09-30-openrs2-2727.yaml"
);

fn family(bytes: &[u8]) -> &'static str {
    match bytes.get(bytes.len().saturating_sub(2)..) {
        Some([0xff, 0xfd]) => "fffd",
        Some([0xff, 0xfe]) => "fffe",
        Some([0xff, 0xff]) => "ffff",
        _ => "legacy",
    }
}

#[test]
#[ignore = "downloads/scans the pinned target cache in a dedicated temporary workflow"]
fn probe_and_decode_build_241_models() -> Result<(), Box<dyn std::error::Error>> {
    let cache_dir = env::var("RUSTOSRS_TARGET_CACHE_DIR")?;
    let profile = TargetProfile::from_yaml_str(TARGET_PROFILE_YAML)?;
    let context = DecoderContext::from_profile(&profile)?;
    let repository = CacheRepository::open(cache_dir, &profile)?;
    let groups = repository.group_ids(7)?;

    assert_eq!(groups.len(), 62_043);

    let mut counts = BTreeMap::<&'static str, usize>::new();
    let mut decoded_counts = BTreeMap::<&'static str, usize>::new();
    let mut texture_render_types = BTreeMap::<u8, usize>::new();
    let mut models_with_biases = 0usize;
    let mut models_with_skeletal_data = 0usize;
    let mut multi_file_groups = 0usize;
    let mut empty_models = 0usize;

    for group_id in groups {
        let metadata = repository.group_metadata(7, group_id)?;
        if metadata.file_ids.len() != 1 {
            multi_file_groups += 1;
        }

        for file_id in metadata.file_ids {
            let file = repository.read_file(7, group_id, file_id, None)?;
            if file.bytes.is_empty() {
                empty_models += 1;
                continue;
            }

            let kind = family(&file.bytes);
            *counts.entry(kind).or_default() += 1;
            if kind == "fffd" {
                let footer = file.bytes.len() - 26;
                let texture_count = usize::from(file.bytes[footer + 4]);
                for &render_type in &file.bytes[..texture_count] {
                    *texture_render_types.entry(render_type).or_default() += 1;
                }
            }

            let source = ArchiveFileProvenance::new(7, group_id, Some(file_id));
            let model = decode_model_data(
                &file.bytes,
                &context,
                &source,
                ModelId::new(group_id),
            )
            .map_err(|error| format!("model {group_id} file {file_id}: {error}"))?;

            let decoded_kind = match model.format().encoding {
                ModelEncoding::TrailerFfFd => "fffd",
                ModelEncoding::TrailerFfFe => "fffe",
                ModelEncoding::TrailerFfFf => "ffff",
                ModelEncoding::Legacy => "legacy",
            };
            *decoded_counts.entry(decoded_kind).or_default() += 1;
            if model.face_biases().is_some() {
                models_with_biases += 1;
            }
            if model.skeletal_vertices().is_some() {
                models_with_skeletal_data += 1;
            }
        }
    }

    println!("M4_MODEL_SWEEP encoded_counts={counts:?}");
    println!("M4_MODEL_SWEEP decoded_counts={decoded_counts:?}");
    println!("M4_MODEL_SWEEP texture_render_types={texture_render_types:?}");
    println!(
        "M4_MODEL_SWEEP models_with_biases={models_with_biases} models_with_skeletal_data={models_with_skeletal_data} multi_file_groups={multi_file_groups} empty_models={empty_models}"
    );

    assert_eq!(counts, BTreeMap::from([("fffd", 35_103), ("fffe", 26_940)]));
    assert_eq!(decoded_counts, counts);
    assert_eq!(multi_file_groups, 0);
    assert_eq!(empty_models, 0);
    Ok(())
}
