use osrs_cache::profile::TargetProfile;
use osrs_cache::transport::CacheRepository;
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
fn probe_build_241_model_families() -> Result<(), Box<dyn std::error::Error>> {
    let cache_dir = env::var("RUSTOSRS_TARGET_CACHE_DIR")?;
    let profile = TargetProfile::from_yaml_str(TARGET_PROFILE_YAML)?;
    let repository = CacheRepository::open(cache_dir, &profile)?;
    let groups = repository.group_ids(7)?;

    assert_eq!(groups.len(), 62_043);

    let mut counts = BTreeMap::<&'static str, usize>::new();
    let mut samples = BTreeMap::<&'static str, Vec<u32>>::new();
    let mut texture_render_types = BTreeMap::<u8, usize>::new();
    let mut texture_type_samples = BTreeMap::<u8, Vec<u32>>::new();
    let mut shortest_by_family = BTreeMap::<&'static str, (usize, u32, Vec<u8>)>::new();
    let mut multi_file_groups = 0usize;
    let mut empty_models = 0usize;
    let mut shortest = usize::MAX;
    let mut longest = 0usize;

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
            shortest = shortest.min(file.bytes.len());
            longest = longest.max(file.bytes.len());
            let kind = family(&file.bytes);
            *counts.entry(kind).or_default() += 1;
            let entry = samples.entry(kind).or_default();
            if entry.len() < 12 {
                entry.push(group_id);
            }

            let replace_shortest = shortest_by_family
                .get(kind)
                .is_none_or(|(length, _, _)| file.bytes.len() < *length);
            if replace_shortest {
                shortest_by_family.insert(
                    kind,
                    (file.bytes.len(), group_id, file.bytes.iter().copied().take(256).collect()),
                );
            }

            if kind == "fffd" {
                if file.bytes.len() < 26 {
                    return Err(format!("FFFD model {group_id} shorter than footer").into());
                }
                let footer = file.bytes.len() - 26;
                let texture_count = usize::from(file.bytes[footer + 4]);
                if texture_count > footer {
                    return Err(format!("FFFD model {group_id} texture prefix exceeds footer").into());
                }
                for &render_type in &file.bytes[..texture_count] {
                    *texture_render_types.entry(render_type).or_default() += 1;
                    let type_samples = texture_type_samples.entry(render_type).or_default();
                    if type_samples.len() < 12 && !type_samples.contains(&group_id) {
                        type_samples.push(group_id);
                    }
                }
            }
        }
    }

    println!("M4_MODEL_PROBE counts={counts:?}");
    println!("M4_MODEL_PROBE samples={samples:?}");
    println!("M4_MODEL_PROBE texture_render_types={texture_render_types:?}");
    println!("M4_MODEL_PROBE texture_type_samples={texture_type_samples:?}");
    println!("M4_MODEL_PROBE shortest_by_family={shortest_by_family:?}");
    println!(
        "M4_MODEL_PROBE multi_file_groups={multi_file_groups} empty_models={empty_models} shortest={shortest} longest={longest}"
    );
    Ok(())
}
