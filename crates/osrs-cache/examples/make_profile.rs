//! Write a target profile for a downloaded cache.
//!
//! `cargo run --release -p osrs-cache --example make_profile -- <cache dir> --build 240 --openrs2-id 2720 \
//!    --timestamp "2026-09-23 14:00:06" [--size-mib 182] [--template profiles/<existing>.yaml]`
//!
//! The new profile is the template (default: the bundled 241 profile) with the cache identity replaced by
//! what the directory actually contains: its fingerprint, group counts and the build you name. Revision
//! gates are re-derived from the build (`osrs_cache::build_format`) and the result is validated before it
//! is written to `profiles/`. Add the file to `osrs_cache::registry` afterwards.

use osrs_cache::build_format::gates_required_for_build;
use osrs_cache::profile::TargetProfile;
use osrs_cache::transport::CacheProbe;
use serde_yaml_ng::Value;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let dir =
        PathBuf::from(args.next().ok_or(
            "usage: make_profile <cache dir> --build N --openrs2-id ID --timestamp \"..\"",
        )?);
    let (mut build, mut id, mut timestamp, mut size_mib, mut template) =
        (None, None, None, None, None);
    while let Some(flag) = args.next() {
        let value = args.next().ok_or("flag without a value")?;
        match flag.as_str() {
            "--build" => build = Some(value.parse::<u32>()?),
            "--openrs2-id" => id = Some(value.parse::<u64>()?),
            "--timestamp" => timestamp = Some(value),
            "--size-mib" => size_mib = Some(value.parse::<u64>()?),
            "--template" => template = Some(PathBuf::from(value)),
            other => return Err(format!("unknown flag {other}").into()),
        }
    }
    let build = build.ok_or("--build is required")?;
    let id = id.ok_or("--openrs2-id is required")?;
    let timestamp = timestamp.ok_or("--timestamp is required")?;
    let template_path = template
        .unwrap_or_else(|| PathBuf::from("profiles/osrs-live-241-2026-09-30-openrs2-2727.yaml"));

    eprintln!("fingerprinting {} (reads every group)...", dir.display());
    let probe = CacheProbe::of(&dir)?;
    eprintln!(
        "fingerprint {}, {} content indices, {} groups",
        probe.fingerprint,
        probe.content_indices.len(),
        probe.groups_total
    );

    let mut profile: Value = serde_yaml_ng::from_str(&std::fs::read_to_string(&template_path)?)?;
    let date = timestamp.split(' ').next().unwrap_or("unknown");
    let profile_id = format!("osrs-live-{build}-{date}-openrs2-{id}");
    set(&mut profile, &["id"], profile_id.clone().into());
    set(&mut profile, &["cache_source", "cache_id"], id.into());
    set(
        &mut profile,
        &["cache_source", "uri"],
        format!("https://archive.openrs2.org/caches/runescape/{id}").into(),
    );
    set(&mut profile, &["cache_source", "build"], build.into());
    set(
        &mut profile,
        &["cache_source", "timestamp"],
        timestamp.into(),
    );
    set(
        &mut profile,
        &["cache_source", "groups_present"],
        probe.groups_total.into(),
    );
    set(
        &mut profile,
        &["cache_source", "groups_total"],
        probe.groups_total.into(),
    );
    if let Some(size) = size_mib {
        set(
            &mut profile,
            &["cache_source", "reported_size_mib"],
            size.into(),
        );
    }
    set(
        &mut profile,
        &["cache_fingerprint", "value"],
        probe.fingerprint.clone().into(),
    );
    set(
        &mut profile,
        &["cache_fingerprint", "state"],
        "verified_by_make_profile".into(),
    );
    set(
        &mut profile,
        &["transport_evidence", "groups_enumerated"],
        probe.groups_total.into(),
    );
    set(
        &mut profile,
        &["xtea", "provider_id"],
        format!("openrs2-cache-{id}").into(),
    );
    // Gates the build requires must say so.
    for gate in gates_required_for_build(build) {
        set(
            &mut profile,
            &["revision_gates", gate, "state"],
            "required".into(),
        );
    }

    let text = serde_yaml_ng::to_string(&profile)?;
    // Validate exactly as the editor will.
    let parsed = TargetProfile::from_yaml_str(&text)?;
    let out = PathBuf::from("profiles").join(format!("{profile_id}.yaml"));
    std::fs::write(&out, text)?;
    eprintln!(
        "wrote {} (build {}, fingerprint {}...)",
        out.display(),
        parsed.cache_source.build,
        &probe.fingerprint[..12]
    );
    Ok(())
}

/// Set a nested key, creating maps on the way.
fn set(root: &mut Value, path: &[&str], value: Value) {
    let mut node = root;
    for key in path {
        let Value::Mapping(map) = node else {
            return;
        };
        node = map
            .entry(Value::String((*key).to_owned()))
            .or_insert(Value::Mapping(Default::default()));
    }
    *node = value;
}
