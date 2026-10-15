use super::*;
use crate::encode::{CacheStore, GroupEntry, ReferenceTable, crc32, decode_container};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn validate_groups(
    original: &Path,
    output: &Path,
    parent: &CacheRepository,
    manifest: &ExportManifest,
    output_indices: &[u8],
) -> Result<(), ExportReadError> {
    let indices = parent.content_index_ids();
    if output_indices != indices {
        return Err(failure("Export added or removed a cache index."));
    }
    let original = CacheStore::open(original);
    let output = CacheStore::open(output);
    let mut changed = BTreeMap::new();
    let mut regions = BTreeSet::new();
    for entry in &manifest.regions {
        let [x, y] = entry.region;
        if !(0..=255).contains(&x)
            || !(0..=255).contains(&y)
            || entry.group != ((x as u32) << 8 | y as u32)
            || changed.insert(entry.group, entry).is_some()
            || !regions.insert(entry.region)
        {
            return Err(failure("Manifest has a duplicate or invalid map region."));
        }
    }
    let mut seen = BTreeSet::new();
    for index in indices {
        let old_raw = raw(&original, 255, u32::from(index))?;
        let new_raw = raw(&output, 255, u32::from(index))?;
        let mut expected =
            ReferenceTable::parse(&decode_container(old_raw.clone()).map_err(failure)?)
                .map_err(failure)?;
        if index != 5 && old_raw != new_raw {
            return Err(failure(format!("Export modified reference table {index}.")));
        }
        for row in &mut expected.groups {
            let old = raw(&original, index, row.id)?;
            let new = raw(&output, index, row.id)?;
            if index == 5
                && let Some(entry) = changed.get(&row.id)
            {
                validate_map_group(&old, &new, row, entry)?;
                seen.insert(row.id);
            } else if old != new {
                return Err(failure(format!(
                    "Unlisted change at index {index}, group {}.",
                    row.id
                )));
            }
        }
        let actual =
            ReferenceTable::parse(&decode_container(new_raw).map_err(failure)?).map_err(failure)?;
        if actual != expected {
            return Err(failure(format!(
                "Reference table {index} differs from the permitted export updates."
            )));
        }
    }
    if seen.len() != changed.len() {
        return Err(failure(
            "Manifest refers to a map group absent from the original cache.",
        ));
    }
    Ok(())
}

fn raw(store: &CacheStore, index: u8, group: u32) -> Result<Vec<u8>, ExportReadError> {
    store
        .read_raw_group(index, group)
        .map_err(failure)?
        .ok_or_else(|| failure(format!("Missing index {index}, group {group}.")))
}

fn body_len(raw: &[u8]) -> Result<usize, ExportReadError> {
    let encoded = raw
        .get(1..5)
        .ok_or_else(|| failure("Truncated container header."))?;
    let size = u32::from_be_bytes(encoded.try_into().map_err(failure)?) as usize;
    let header = if raw[0] == 0 { 5usize } else { 9usize };
    let body = header
        .checked_add(size)
        .ok_or_else(|| failure("Container size overflow."))?;
    if raw.len() != body && raw.len() != body + 2 {
        return Err(failure("Invalid container length or version suffix."));
    }
    Ok(body)
}

fn validate_map_group(
    old: &[u8],
    new: &[u8],
    row: &mut GroupEntry,
    entry: &ExportMapGroup,
) -> Result<(), ExportReadError> {
    let old_body = body_len(old)?;
    let new_body = body_len(new)?;
    if old.first() != new.first() || (old.len() > old_body) != (new.len() > new_body) {
        return Err(failure(
            "Export changed group compression or version-suffix policy.",
        ));
    }
    let payload = decode_container(new.to_vec()).map_err(failure)?;
    if payload_digest(&payload) != entry.payload_sha256 {
        return Err(failure(format!(
            "Map group {} differs from its prepared payload.",
            row.id
        )));
    }
    let original_payload = decode_container(old.to_vec()).map_err(failure)?;
    let original_files =
        crate::transport::split_group_files(&original_payload, &row.file_ids, row.file_ids.len())
            .map_err(|e| failure(format!("{e:?}")))?;
    let files = crate::transport::split_group_files(&payload, &row.file_ids, row.file_ids.len())
        .map_err(|e| failure(format!("{e:?}")))?;
    for ((id, before), (_, after)) in original_files.iter().zip(&files) {
        if *id > 1 && before != after {
            return Err(failure(format!(
                "Map group {} modified preserved file {id}.",
                row.id
            )));
        }
    }
    if row.hash.is_some() || row.whirlpool.is_some() {
        return Err(failure(
            "Export verification does not support hashed map reference tables.",
        ));
    }
    row.crc = crc32(&new[..new_body]);
    row.version = row
        .version
        .checked_add(1)
        .ok_or_else(|| failure("Group version overflow."))?;
    if row.sizes.is_some() {
        row.sizes = Some((new_body as u32, payload.len() as u32));
    }
    if new.len() > new_body && new[new_body..] != (row.version as u16).to_be_bytes() {
        return Err(failure(
            "Map container version suffix disagrees with its table row.",
        ));
    }
    Ok(())
}
