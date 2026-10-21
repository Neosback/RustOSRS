//! Publish only fully prepared and independently reopened output.

mod staging;
pub(crate) use staging::publish_new_file;

use super::prepare::verify_files;
use super::*;
use osrs_cache::decode::DecoderContext;
use osrs_cache::encode::{
    CacheStore, ContainerCompression, GroupUpdate, decode_container, encode_container, pack_group,
    repack_groups, split_group,
};
use osrs_cache::verified_export::{
    CACHE_MANIFEST, EXPORT_FORMAT, ExportManifest, ExportMapGroup, VerifiedExport, cache_digest,
    payload_digest,
};
use std::path::Path;

#[derive(Debug, Clone)]
pub(crate) struct ExportVerificationReport {
    pub stamp: ExportStamp,
    pub out_dir: PathBuf,
    pub manifest: ExportManifest,
    pub unchanged: usize,
}

pub(crate) fn publish(
    prepared: &PreparedExport,
    mode: ExportMode,
    out: &Path,
) -> Result<ExportVerificationReport, String> {
    publish_with(prepared, mode, out, |_| Ok(()))
}

#[cfg(test)]
pub(crate) fn publish_fixture(
    prepared: &PreparedExport,
    out: &Path,
    profiles: &[osrs_cache::profile::TargetProfile],
) -> Result<ExportVerificationReport, String> {
    publish_with_profiles(prepared, ExportMode::CacheCopy, out, profiles, |_| Ok(()))
}

fn publish_with(
    prepared: &PreparedExport,
    mode: ExportMode,
    out: &Path,
    before_verify: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<ExportVerificationReport, String> {
    let profiles = osrs_cache::registry::bundled_profiles().map_err(|e| e.to_string())?;
    publish_with_profiles(prepared, mode, out, &profiles, before_verify)
}

fn publish_with_profiles(
    prepared: &PreparedExport,
    mode: ExportMode,
    out: &Path,
    profiles: &[osrs_cache::profile::TargetProfile],
    before_verify: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<ExportVerificationReport, String> {
    if !prepared.can_publish() {
        return Err("Export has blockers or contains no changes.".to_owned());
    }
    let CacheSource::Dir(source) = &prepared.source else {
        return Err("Export requires the supported original cache folder.".to_owned());
    };
    let (parent, profile) = osrs_cache::transport::CacheRepository::open_matching(source, profiles)
        .map_err(|e| e.to_string())?;
    if parent.fingerprint() != prepared.fingerprint || profile.id != prepared.profile {
        return Err("Original cache changed after validation; validate again.".to_owned());
    }
    let stage = staging::Staging::new(source, out)?;
    let mut manifest = manifest_of(prepared, mode)?;
    write_prepared(prepared, mode, source, stage.path())?;
    let conversions: Vec<_> = prepared.regions.iter().flat_map(|region| region.conversions.iter()).map(|c| serde_json::json!({
        "x": c.position.tile.x, "y": c.position.tile.y, "plane": c.position.plane.index().get(), "authored": c.authored, "reloaded": c.reloaded
    })).collect();
    if !conversions.is_empty() {
        let report = serde_json::to_vec_pretty(
            &serde_json::json!({"policy":"eiken-truncate-v1", "heights":conversions}),
        )
        .map_err(|e| e.to_string())?;
        staging::write_file(&stage.path().join("height-conversions.json"), &report)?;
    }
    before_verify(stage.path())?;
    verify_prepared(prepared, mode, stage.path(), &profile)?;
    manifest.output_sha256 = match mode {
        ExportMode::CacheCopy => cache_digest(stage.path()).map_err(|e| e.to_string())?,
        ExportMode::Loose => loose_digest(prepared, stage.path())?,
    };
    if mode == ExportMode::CacheCopy {
        VerifiedExport::validate(source, stage.path(), &manifest, profiles)
            .map_err(|e| e.to_string())?;
    }
    let name = if mode == ExportMode::CacheCopy {
        CACHE_MANIFEST
    } else {
        "manifest.json"
    };
    let text = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
    staging::write_file(&stage.path().join(name), &text)?;
    // Re-read the report too: publication never succeeds with an unwritten/partial manifest.
    let stored: ExportManifest =
        serde_json::from_slice(&std::fs::read(stage.path().join(name)).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    if stored.output_sha256 != manifest.output_sha256 {
        return Err("Manifest verification failed.".to_owned());
    }
    stage.publish(out)?;
    Ok(ExportVerificationReport {
        stamp: prepared.stamp,
        out_dir: out.to_path_buf(),
        manifest,
        unchanged: prepared.unchanged,
    })
}

fn payload(region: &PreparedRegion) -> Result<Vec<u8>, String> {
    let parts: Vec<&[u8]> = region
        .files
        .iter()
        .map(|(_, bytes)| bytes.as_slice())
        .collect();
    pack_group(&parts).map_err(|e| e.to_string())
}

fn manifest_of(prepared: &PreparedExport, mode: ExportMode) -> Result<ExportManifest, String> {
    let regions = prepared
        .regions
        .iter()
        .map(|r| {
            Ok(ExportMapGroup {
                region: [r.region.x, r.region.y],
                group: r.group,
                payload_sha256: payload_digest(&payload(r)?),
                tiles_changed: r.changes.tiles,
                heights_changed: r.changes.heights,
                objects_added: r.changes.objects_added,
                objects_removed: r.changes.objects_removed,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(ExportManifest {
        format: EXPORT_FORMAT.to_owned(),
        mode: mode.name().to_owned(),
        profile: prepared.profile.clone(),
        source_fingerprint: prepared.fingerprint.clone(),
        edit_revision: prepared.stamp.edit_revision,
        output_sha256: String::new(),
        regions,
    })
}

fn write_prepared(
    prepared: &PreparedExport,
    mode: ExportMode,
    source: &Path,
    out: &Path,
) -> Result<(), String> {
    match mode {
        ExportMode::Loose => {
            std::fs::create_dir(out.join("5")).map_err(|e| e.to_string())?;
            for r in &prepared.regions {
                let bytes = encode_container(payload(r)?, ContainerCompression::Gzip, None)
                    .map_err(|e| e.to_string())?;
                staging::write_file(&out.join(format!("5/{}.dat", r.group)), &bytes)?;
            }
        }
        ExportMode::CacheCopy => {
            staging::copy_cache(source, out)?;
            let updates = prepared
                .regions
                .iter()
                .map(|r| {
                    Ok(GroupUpdate {
                        index_id: 5,
                        group_id: r.group,
                        payload: payload(r)?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            repack_groups(out, &updates).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn verify_prepared(
    prepared: &PreparedExport,
    mode: ExportMode,
    out: &Path,
    profile: &osrs_cache::profile::TargetProfile,
) -> Result<(), String> {
    let context = DecoderContext::from_profile(profile).map_err(|e| e.to_string())?;
    let store = CacheStore::open(out);
    for r in &prepared.regions {
        let raw = match mode {
            ExportMode::Loose => {
                std::fs::read(out.join(format!("5/{}.dat", r.group))).map_err(|e| e.to_string())?
            }
            ExportMode::CacheCopy => store
                .read_raw_group(5, r.group)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("Missing written map group {}.", r.group))?,
        };
        let decoded = decode_container(raw).map_err(|e| e.to_string())?;
        let files = split_group(&decoded, r.files.len()).map_err(|e| e.to_string())?;
        if files != r.files {
            return Err(format!(
                "Written group {} differs from the prepared files.",
                r.group
            ));
        }
        verify_files(r.region, r.group, &files, &r.expected, &context).map_err(|e| e.message)?;
    }
    Ok(())
}

fn loose_digest(prepared: &PreparedExport, out: &Path) -> Result<String, String> {
    let mut digests = Vec::new();
    for r in &prepared.regions {
        digests.extend_from_slice(&r.group.to_be_bytes());
        let bytes =
            std::fs::read(out.join(format!("5/{}.dat", r.group))).map_err(|e| e.to_string())?;
        digests.extend_from_slice(payload_digest(&bytes).as_bytes());
    }
    Ok(payload_digest(&digests))
}

#[cfg(test)]
mod tests;
