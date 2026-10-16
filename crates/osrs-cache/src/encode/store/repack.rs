//! Prepare every affected group/table before changing a staged cache.

use super::*;
use std::collections::BTreeSet;

struct PreparedIndex {
    id: u8,
    groups: Vec<(u32, Vec<u8>)>,
    table: Vec<u8>,
}

pub(super) fn repack(
    dir: &Path,
    updates: &[GroupUpdate],
    changed_only: bool,
) -> Result<RepackReport, StoreError> {
    let store = CacheStore::open(dir);
    let mut targets = BTreeSet::new();
    for update in updates {
        if update.index_id == REFERENCE_TABLE_INDEX
            || !targets.insert((update.index_id, update.group_id))
        {
            return Err(invalid("repack requires unique content-index targets"));
        }
    }
    let indices: BTreeSet<_> = targets.iter().map(|(index, _)| *index).collect();
    let mut prepared = Vec::new();
    for index in indices {
        if let Some(index) = prepare_index(&store, index, updates, changed_only)? {
            prepared.push(index);
        }
    }
    let mut report = RepackReport::default();
    for index in prepared {
        for (id, container) in index.groups {
            store.write_raw_group(index.id, id, &container)?;
            report.groups += 1;
        }
        store.write_raw_group(REFERENCE_TABLE_INDEX, u32::from(index.id), &index.table)?;
        report.tables += 1;
    }
    Ok(report)
}

fn prepare_index(
    store: &CacheStore,
    index_id: u8,
    updates: &[GroupUpdate],
    changed_only: bool,
) -> Result<Option<PreparedIndex>, StoreError> {
    let table_raw = store.read_raw_group(REFERENCE_TABLE_INDEX, u32::from(index_id))?
        .ok_or(StoreError::MissingTable { index_id })?;
    let compression = compression_of(&table_raw)?;
    let body = container_body_len(&table_raw).ok_or_else(|| invalid("truncated reference table"))?;
    let suffix = table_raw[body..].to_vec();
    let mut table = ReferenceTable::parse(&decode_container(table_raw)?)?;
    let mut groups = Vec::new();
    for update in updates.iter().filter(|update| update.index_id == index_id) {
        let entry = table.group_mut(update.group_id).ok_or(StoreError::UnknownGroup {
            index_id, group_id: update.group_id,
        })?;
        let old = store.read_raw_group(index_id, update.group_id)?;
        if changed_only && let Some(old) = &old {
            verify_existing(old, entry)?;
            if decode_container(old.clone())? == update.payload {
                continue;
            }
        }
        // These columns require additional digest algorithms not implemented by this writer.
        // Retaining an old digest would publish an internally inconsistent reference table.
        if entry.whirlpool.is_some() || entry.hash.is_some() {
            return Err(invalid("repacking digest-bearing reference tables is unsupported"));
        }
        let compression = old.as_ref().map(|raw| compression_of(raw))
            .transpose()?.unwrap_or(ContainerCompression::Gzip);
        let had_suffix = old.as_ref().is_some_and(|raw| {
            container_body_len(raw).is_some_and(|body| raw.len() == body + 2)
        });
        let mut container = encode_container(update.payload.clone(), compression, None)?;
        entry.version = entry.version.checked_add(1).ok_or(StoreError::VersionOverflow {
            index_id, group_id: update.group_id,
        })?;
        entry.crc = crc32(&container);
        if entry.sizes.is_some() {
            entry.sizes = Some((container.len() as u32, update.payload.len() as u32));
        }
        if had_suffix {
            container.extend_from_slice(&(entry.version as u16).to_be_bytes());
        }
        if container.is_empty() || container.len() > MAX_GROUP_LEN {
            return Err(StoreError::GroupTooLarge { len: container.len() });
        }
        groups.push((update.group_id, container));
    }
    if groups.is_empty() {
        return Ok(None);
    }
    let mut encoded = encode_container(table.encode()?, compression, None)?;
    encoded.extend_from_slice(&suffix);
    if encoded.len() > MAX_GROUP_LEN {
        return Err(StoreError::GroupTooLarge { len: encoded.len() });
    }
    Ok(Some(PreparedIndex { id: index_id, groups, table: encoded }))
}

fn verify_existing(raw: &[u8], entry: &super::super::GroupEntry) -> Result<(), StoreError> {
    let body = container_body_len(raw).ok_or_else(|| invalid("truncated existing container"))?;
    if !matches!(raw.len() - body, 0 | 2) || crc32(&raw[..body]) != entry.crc {
        return Err(invalid("existing group CRC or container suffix disagrees with metadata"));
    }
    if raw.len() == body + 2 && raw[body..] != (entry.version as u16).to_be_bytes() {
        return Err(invalid("existing group version disagrees with metadata"));
    }
    if let Some((compressed, decoded)) = entry.sizes {
        let payload_len = if raw[0] == 0 { body - 5 } else {
            u32::from_be_bytes(raw[5..9].try_into().map_err(|_| invalid("truncated length"))?) as usize
        };
        if compressed as usize != body || decoded as usize != payload_len {
            return Err(invalid("existing group sizes disagree with metadata"));
        }
    }
    Ok(())
}

fn invalid(detail: &'static str) -> StoreError {
    StoreError::CorruptSector { sector: 0, detail }
}
