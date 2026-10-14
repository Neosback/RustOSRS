//! Append-only writing of a JS5 disk cache (`main_file_cache.dat2` + `.idxN`).
//!
//! The store never edits sectors in place: a written group is appended as a fresh sector chain and
//! its `.idx` entry is repointed, so a failed write can only leave unreferenced garbage at the end
//! of `.dat2`, never a half-overwritten group. `repack_groups` replaces groups of one cache folder
//! and keeps each affected reference table consistent (checksum, sizes, version).

use super::{
    ContainerCompression, ContainerError, ReferenceTable, ReferenceTableError, crc32,
    decode_container, encode_container,
};
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

const DATA_FILE: &str = "main_file_cache.dat2";
const IDX_PREFIX: &str = "main_file_cache.idx";
/// Index 255 holds each content index's reference table at group id = index id.
pub const REFERENCE_TABLE_INDEX: u8 = 255;
const SECTOR_SIZE: usize = 520;
const IDX_ENTRY: usize = 6;
const MAX_GROUP_LEN: usize = 0x00ff_ffff;

#[derive(Debug)]
pub enum StoreError {
    Io(io::Error),
    /// A group is larger than an `.idx` entry's 24-bit length can describe.
    GroupTooLarge {
        len: usize,
    },
    /// A sector header disagrees with the group being read.
    CorruptSector {
        sector: usize,
        detail: &'static str,
    },
    Container(ContainerError),
    Table(ReferenceTableError),
    /// The index has no reference table entry.
    MissingTable {
        index_id: u8,
    },
    /// The reference table has no row for the group (repack only replaces existing groups).
    UnknownGroup {
        index_id: u8,
        group_id: u32,
    },
    /// A container with a compression byte this writer cannot reproduce.
    UnsupportedCompression {
        byte: u8,
    },
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "cache I/O: {error}"),
            Self::GroupTooLarge { len } => write!(formatter, "group of {len} bytes is too large"),
            Self::CorruptSector { sector, detail } => {
                write!(formatter, "corrupt sector {sector}: {detail}")
            }
            Self::Container(error) => write!(formatter, "{error}"),
            Self::Table(error) => write!(formatter, "{error}"),
            Self::MissingTable { index_id } => {
                write!(formatter, "index {index_id} has no reference table")
            }
            Self::UnknownGroup { index_id, group_id } => {
                write!(formatter, "index {index_id} has no group {group_id}")
            }
            Self::UnsupportedCompression { byte } => {
                write!(formatter, "unsupported container compression {byte}")
            }
        }
    }
}

impl std::error::Error for StoreError {}

impl From<io::Error> for StoreError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<ContainerError> for StoreError {
    fn from(error: ContainerError) -> Self {
        Self::Container(error)
    }
}

impl From<ReferenceTableError> for StoreError {
    fn from(error: ReferenceTableError) -> Self {
        Self::Table(error)
    }
}

/// A cache folder opened for reading and appending.
pub struct CacheStore {
    dir: PathBuf,
}

fn header_len(group_id: u32) -> usize {
    if group_id > u32::from(u16::MAX) {
        10
    } else {
        8
    }
}

impl CacheStore {
    /// Create an empty cache folder: a `.dat2` holding only the reserved first sector.
    pub fn create(dir: impl AsRef<Path>) -> Result<Self, StoreError> {
        let dir = dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join(DATA_FILE), vec![0u8; SECTOR_SIZE])?;
        File::create(dir.join(format!("{IDX_PREFIX}{REFERENCE_TABLE_INDEX}")))?;
        Ok(Self { dir })
    }

    pub fn open(dir: impl AsRef<Path>) -> Self {
        Self {
            dir: dir.as_ref().to_path_buf(),
        }
    }

    fn idx_path(&self, index_id: u8) -> PathBuf {
        self.dir.join(format!("{IDX_PREFIX}{index_id}"))
    }

    /// The `(length, first sector)` the index points the group at, or `None` when unset.
    fn entry(&self, index_id: u8, group_id: u32) -> Result<Option<(usize, usize)>, StoreError> {
        let mut file = match File::open(self.idx_path(index_id)) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let offset = u64::from(group_id) * IDX_ENTRY as u64;
        if file.metadata()?.len() < offset + IDX_ENTRY as u64 {
            return Ok(None);
        }
        file.seek(SeekFrom::Start(offset))?;
        let mut raw = [0u8; IDX_ENTRY];
        file.read_exact(&mut raw)?;
        let len = usize::from(raw[0]) << 16 | usize::from(raw[1]) << 8 | usize::from(raw[2]);
        let sector = usize::from(raw[3]) << 16 | usize::from(raw[4]) << 8 | usize::from(raw[5]);
        Ok((len != 0).then_some((len, sector)))
    }

    /// The group's stored bytes (a JS5 container), or `None` when the index has no such group.
    pub fn read_raw_group(
        &self,
        index_id: u8,
        group_id: u32,
    ) -> Result<Option<Vec<u8>>, StoreError> {
        let Some((len, first)) = self.entry(index_id, group_id)? else {
            return Ok(None);
        };
        let mut dat = File::open(self.dir.join(DATA_FILE))?;
        let header = header_len(group_id);
        let payload = SECTOR_SIZE - header;
        let mut out = Vec::with_capacity(len);
        let (mut sector, mut chunk) = (first, 0usize);
        while out.len() < len {
            let mut raw = [0u8; SECTOR_SIZE];
            dat.seek(SeekFrom::Start((sector * SECTOR_SIZE) as u64))?;
            dat.read_exact(&mut raw)?;
            let (id, rest) = if header == 10 {
                (
                    u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]),
                    &raw[4..],
                )
            } else {
                (u32::from(u16::from_be_bytes([raw[0], raw[1]])), &raw[2..])
            };
            let found_chunk = usize::from(u16::from_be_bytes([rest[0], rest[1]]));
            let next =
                usize::from(rest[2]) << 16 | usize::from(rest[3]) << 8 | usize::from(rest[4]);
            if id != group_id || found_chunk != chunk || rest[5] != index_id {
                return Err(StoreError::CorruptSector {
                    sector,
                    detail: "header does not match the group",
                });
            }
            let take = (len - out.len()).min(payload);
            out.extend_from_slice(&raw[header..header + take]);
            sector = next;
            chunk += 1;
        }
        Ok(Some(out))
    }

    /// Append `data` as a new sector chain and point the group's `.idx` entry at it.
    pub fn write_raw_group(
        &self,
        index_id: u8,
        group_id: u32,
        data: &[u8],
    ) -> Result<(), StoreError> {
        if data.is_empty() || data.len() > MAX_GROUP_LEN {
            return Err(StoreError::GroupTooLarge { len: data.len() });
        }
        let mut dat = OpenOptions::new()
            .read(true)
            .write(true)
            .open(self.dir.join(DATA_FILE))?;
        let end = dat.seek(SeekFrom::End(0))? as usize;
        let first = end.div_ceil(SECTOR_SIZE);
        dat.set_len((first * SECTOR_SIZE) as u64)?;
        dat.seek(SeekFrom::Start((first * SECTOR_SIZE) as u64))?;

        let header = header_len(group_id);
        let payload = SECTOR_SIZE - header;
        let chunks = data.len().div_ceil(payload);
        let mut out = Vec::with_capacity(chunks * SECTOR_SIZE);
        for (chunk, bytes) in data.chunks(payload).enumerate() {
            if header == 10 {
                out.extend_from_slice(&group_id.to_be_bytes());
            } else {
                out.extend_from_slice(&(group_id as u16).to_be_bytes());
            }
            out.extend_from_slice(&(chunk as u16).to_be_bytes());
            let next = if chunk + 1 == chunks {
                0
            } else {
                first + chunk + 1
            };
            out.extend_from_slice(&(next as u32).to_be_bytes()[1..]);
            out.push(index_id);
            out.extend_from_slice(bytes);
            out.resize((chunk + 1) * SECTOR_SIZE, 0);
        }
        dat.write_all(&out)?;
        dat.sync_all()?;

        let mut idx = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.idx_path(index_id))?;
        let offset = u64::from(group_id) * IDX_ENTRY as u64;
        if idx.metadata()?.len() < offset {
            idx.set_len(offset)?;
        }
        idx.seek(SeekFrom::Start(offset))?;
        let mut entry = [0u8; IDX_ENTRY];
        entry[..3].copy_from_slice(&(data.len() as u32).to_be_bytes()[1..]);
        entry[3..].copy_from_slice(&(first as u32).to_be_bytes()[1..]);
        idx.write_all(&entry)?;
        idx.sync_all()?;
        Ok(())
    }
}

/// One group to replace: the new decoded payload (already packed with `pack_group` when the group
/// has several files).
pub struct GroupUpdate {
    pub index_id: u8,
    pub group_id: u32,
    pub payload: Vec<u8>,
}

/// What a repack did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RepackReport {
    pub groups: usize,
    pub tables: usize,
}

/// Length of a container's body (header plus data), excluding any trailing 2-byte version.
fn container_body_len(raw: &[u8]) -> Option<usize> {
    let compression = *raw.first()?;
    let data_len = u32::from_be_bytes(raw.get(1..5)?.try_into().ok()?) as usize;
    let header: usize = if compression == 0 { 5 } else { 9 };
    let len = header.checked_add(data_len)?;
    (raw.len() >= len).then_some(len)
}

fn compression_of(raw: &[u8]) -> Result<ContainerCompression, StoreError> {
    match raw.first() {
        Some(0) => Ok(ContainerCompression::None),
        Some(1) => Ok(ContainerCompression::Bzip2),
        Some(2) => Ok(ContainerCompression::Gzip),
        Some(&byte) => Err(StoreError::UnsupportedCompression { byte }),
        None => Err(StoreError::CorruptSector {
            sector: 0,
            detail: "empty container",
        }),
    }
}

/// Replace groups in the cache folder `dir`, updating each affected index's reference table.
///
/// For every update the new container is appended, and its table row gets the new CRC-32 (of the
/// container without its version suffix), compressed and uncompressed sizes (when the table
/// carries them), and `version + 1`. A group's version suffix is kept if the old one had it. The
/// table is then re-encoded with its original compression and appended to index 255. The master
/// checksum table is derived by servers and is not stored in disk caches.
pub fn repack_groups(
    dir: impl AsRef<Path>,
    updates: &[GroupUpdate],
) -> Result<RepackReport, StoreError> {
    let store = CacheStore::open(dir);
    let mut report = RepackReport::default();
    let mut indices: Vec<u8> = updates.iter().map(|update| update.index_id).collect();
    indices.sort_unstable();
    indices.dedup();

    for index_id in indices {
        let table_raw = store
            .read_raw_group(REFERENCE_TABLE_INDEX, u32::from(index_id))?
            .ok_or(StoreError::MissingTable { index_id })?;
        let table_compression = compression_of(&table_raw)?;
        let table_body = container_body_len(&table_raw).ok_or(StoreError::CorruptSector {
            sector: 0,
            detail: "reference table container is truncated",
        })?;
        let table_suffix = table_raw[table_body..].to_vec();
        let mut table = ReferenceTable::parse(&decode_container(table_raw)?)?;

        for update in updates.iter().filter(|update| update.index_id == index_id) {
            let old = store.read_raw_group(index_id, update.group_id)?;
            let compression = match &old {
                Some(raw) => compression_of(raw)?,
                None => ContainerCompression::Gzip,
            };
            let had_suffix = old.as_ref().is_some_and(|raw| {
                container_body_len(raw).is_some_and(|body| raw.len() == body + 2)
            });

            let mut container = encode_container(update.payload.clone(), compression, None)?;
            let entry = table
                .group_mut(update.group_id)
                .ok_or(StoreError::UnknownGroup {
                    index_id,
                    group_id: update.group_id,
                })?;
            entry.crc = crc32(&container);
            if entry.sizes.is_some() {
                entry.sizes = Some((container.len() as u32, update.payload.len() as u32));
            }
            entry.version += 1;
            if had_suffix {
                container.extend_from_slice(&(entry.version as u16).to_be_bytes());
            }
            store.write_raw_group(index_id, update.group_id, &container)?;
            report.groups += 1;
        }

        let mut encoded = encode_container(table.encode()?, table_compression, None)?;
        encoded.extend_from_slice(&table_suffix);
        store.write_raw_group(REFERENCE_TABLE_INDEX, u32::from(index_id), &encoded)?;
        report.tables += 1;
    }
    Ok(report)
}

#[cfg(test)]
mod tests;
