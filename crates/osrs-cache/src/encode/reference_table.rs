//! The JS5 reference table: per index, every group's checksum, version, and file list.
//!
//! This codec keeps every field the format carries (including the revision and optional size,
//! hash, and whirlpool columns) so that `encode(parse(bytes)) == bytes` for real tables. It works
//! on decompressed table bytes; container compression is `encode_container`'s job.

use std::fmt;

const FLAG_NAMED: u8 = 1;
const FLAG_WHIRLPOOL: u8 = 2;
const FLAG_SIZES: u8 = 4;
const FLAG_HASH: u8 = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceTableError {
    /// The table ended before a field was complete.
    Truncated { what: &'static str },
    /// Bytes remained after the last field.
    TrailingBytes { remaining: usize },
    /// A value cannot be written (non-ascending ids, or a column that disagrees with the flags).
    Invalid { what: &'static str },
}

impl fmt::Display for ReferenceTableError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated { what } => write!(formatter, "reference table truncated at {what}"),
            Self::TrailingBytes { remaining } => {
                write!(formatter, "{remaining} bytes follow the reference table")
            }
            Self::Invalid { what } => write!(formatter, "invalid reference table: {what}"),
        }
    }
}

impl std::error::Error for ReferenceTableError {}

/// One group's row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupEntry {
    pub id: u32,
    pub name_hash: Option<i32>,
    /// CRC-32 of the group's container (see `crc32_of_container`).
    pub crc: u32,
    pub hash: Option<i32>,
    pub whirlpool: Option<[u8; 64]>,
    /// Compressed and uncompressed sizes (flag 4).
    pub sizes: Option<(u32, u32)>,
    pub version: u32,
    pub file_ids: Vec<u32>,
    /// One name hash per file (named tables only).
    pub file_name_hashes: Option<Vec<i32>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceTable {
    pub protocol: u8,
    /// The table revision (protocol 6 and later).
    pub revision: Option<u32>,
    pub flags: u8,
    pub groups: Vec<GroupEntry>,
}

struct Reader<'a> {
    bytes: &'a [u8],
}

impl<'a> Reader<'a> {
    fn take(&mut self, count: usize, what: &'static str) -> Result<&'a [u8], ReferenceTableError> {
        if self.bytes.len() < count {
            return Err(ReferenceTableError::Truncated { what });
        }
        let (head, tail) = self.bytes.split_at(count);
        self.bytes = tail;
        Ok(head)
    }

    fn u8(&mut self, what: &'static str) -> Result<u8, ReferenceTableError> {
        Ok(self.take(1, what)?[0])
    }

    fn u16(&mut self, what: &'static str) -> Result<u32, ReferenceTableError> {
        let b = self.take(2, what)?;
        Ok(u32::from(u16::from_be_bytes([b[0], b[1]])))
    }

    fn u32(&mut self, what: &'static str) -> Result<u32, ReferenceTableError> {
        let b = self.take(4, what)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// A count or id delta: a `u16`, or (protocol 7+) a "big smart" of two or four bytes.
    fn count(&mut self, protocol: u8, what: &'static str) -> Result<u32, ReferenceTableError> {
        if protocol < 7 {
            return self.u16(what);
        }
        match self.bytes.first() {
            None => Err(ReferenceTableError::Truncated { what }),
            Some(first) if first & 0x80 == 0 => self.u16(what),
            Some(_) => Ok(self.u32(what)? & 0x7fff_ffff),
        }
    }
}

fn push_count(out: &mut Vec<u8>, protocol: u8, value: u32) -> Result<(), ReferenceTableError> {
    if protocol < 7 {
        let value = u16::try_from(value).map_err(|_| ReferenceTableError::Invalid {
            what: "value exceeds 16 bits for this protocol",
        })?;
        out.extend_from_slice(&value.to_be_bytes());
    } else if value < 0x8000 {
        out.extend_from_slice(&(value as u16).to_be_bytes());
    } else if value <= 0x7fff_ffff {
        out.extend_from_slice(&(value | 0x8000_0000).to_be_bytes());
    } else {
        return Err(ReferenceTableError::Invalid {
            what: "value exceeds 31 bits",
        });
    }
    Ok(())
}

impl ReferenceTable {
    /// Parse decompressed reference-table bytes.
    pub fn parse(bytes: &[u8]) -> Result<Self, ReferenceTableError> {
        let mut r = Reader { bytes };
        let protocol = r.u8("protocol")?;
        let revision = (protocol >= 6).then(|| r.u32("revision")).transpose()?;
        let flags = r.u8("flags")?;
        let count = r.count(protocol, "group count")? as usize;

        let mut ids = Vec::with_capacity(count);
        let mut id = 0u32;
        for _ in 0..count {
            id = id.checked_add(r.count(protocol, "group id")?).ok_or(
                ReferenceTableError::Invalid {
                    what: "group id overflow",
                },
            )?;
            ids.push(id);
        }
        let named = flags & FLAG_NAMED != 0;
        let column = |r: &mut Reader<'_>, present: bool, what| -> Result<Vec<Option<i32>>, _> {
            (0..count)
                .map(|_| present.then(|| r.u32(what).map(|v| v as i32)).transpose())
                .collect()
        };
        let name_hashes = column(&mut r, named, "group name hash")?;
        let crcs = (0..count)
            .map(|_| r.u32("crc"))
            .collect::<Result<Vec<_>, _>>()?;
        let hashes = column(&mut r, flags & FLAG_HASH != 0, "group hash")?;
        let whirlpools = (0..count)
            .map(|_| {
                (flags & FLAG_WHIRLPOOL != 0)
                    .then(|| {
                        let mut w = [0u8; 64];
                        w.copy_from_slice(r.take(64, "whirlpool")?);
                        Ok(w)
                    })
                    .transpose()
            })
            .collect::<Result<Vec<_>, ReferenceTableError>>()?;
        let sizes = (0..count)
            .map(|_| {
                (flags & FLAG_SIZES != 0)
                    .then(|| Ok((r.u32("compressed size")?, r.u32("uncompressed size")?)))
                    .transpose()
            })
            .collect::<Result<Vec<_>, ReferenceTableError>>()?;
        let versions = (0..count)
            .map(|_| r.u32("version"))
            .collect::<Result<Vec<_>, _>>()?;
        let file_counts = (0..count)
            .map(|_| r.count(protocol, "file count").map(|c| c as usize))
            .collect::<Result<Vec<_>, _>>()?;
        let mut file_ids = Vec::with_capacity(count);
        for &files in &file_counts {
            let mut ids = Vec::with_capacity(files);
            let mut id = 0u32;
            for _ in 0..files {
                id = id.checked_add(r.count(protocol, "file id")?).ok_or(
                    ReferenceTableError::Invalid {
                        what: "file id overflow",
                    },
                )?;
                ids.push(id);
            }
            file_ids.push(ids);
        }
        let mut groups = Vec::with_capacity(count);
        for (index, files) in file_ids.into_iter().enumerate() {
            let file_name_hashes = named
                .then(|| {
                    files
                        .iter()
                        .map(|_| r.u32("file name hash").map(|v| v as i32))
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?;
            groups.push(GroupEntry {
                id: ids[index],
                name_hash: name_hashes[index],
                crc: crcs[index],
                hash: hashes[index],
                whirlpool: whirlpools[index],
                sizes: sizes[index],
                version: versions[index],
                file_ids: files,
                file_name_hashes,
            });
        }
        if !r.bytes.is_empty() {
            return Err(ReferenceTableError::TrailingBytes {
                remaining: r.bytes.len(),
            });
        }
        Ok(Self {
            protocol,
            revision,
            flags,
            groups,
        })
    }

    /// Encode to decompressed reference-table bytes.
    pub fn encode(&self) -> Result<Vec<u8>, ReferenceTableError> {
        let invalid = |what| ReferenceTableError::Invalid { what };
        let (named, whirlpool, sized, hashed) = (
            self.flags & FLAG_NAMED != 0,
            self.flags & FLAG_WHIRLPOOL != 0,
            self.flags & FLAG_SIZES != 0,
            self.flags & FLAG_HASH != 0,
        );
        if (self.protocol >= 6) != self.revision.is_some() {
            return Err(invalid("revision must be present exactly for protocol 6+"));
        }
        let mut out = vec![self.protocol];
        if let Some(revision) = self.revision {
            out.extend_from_slice(&revision.to_be_bytes());
        }
        out.push(self.flags);
        push_count(&mut out, self.protocol, self.groups.len() as u32)?;

        let mut previous = 0u32;
        for group in &self.groups {
            let delta = group
                .id
                .checked_sub(previous)
                .ok_or(invalid("group ids must be ascending"))?;
            push_count(&mut out, self.protocol, delta)?;
            previous = group.id;
        }
        for group in &self.groups {
            if group.name_hash.is_some() != named
                || group.hash.is_some() != hashed
                || group.whirlpool.is_some() != whirlpool
                || group.sizes.is_some() != sized
                || group.file_name_hashes.is_some() != named
            {
                return Err(invalid("a column disagrees with the table flags"));
            }
        }
        if named {
            for group in &self.groups {
                out.extend_from_slice(&group.name_hash.unwrap_or_default().to_be_bytes());
            }
        }
        for group in &self.groups {
            out.extend_from_slice(&group.crc.to_be_bytes());
        }
        if hashed {
            for group in &self.groups {
                out.extend_from_slice(&group.hash.unwrap_or_default().to_be_bytes());
            }
        }
        if whirlpool {
            for group in &self.groups {
                out.extend_from_slice(&group.whirlpool.unwrap_or([0; 64]));
            }
        }
        if sized {
            for group in &self.groups {
                let (compressed, uncompressed) = group.sizes.unwrap_or_default();
                out.extend_from_slice(&compressed.to_be_bytes());
                out.extend_from_slice(&uncompressed.to_be_bytes());
            }
        }
        for group in &self.groups {
            out.extend_from_slice(&group.version.to_be_bytes());
        }
        for group in &self.groups {
            push_count(&mut out, self.protocol, group.file_ids.len() as u32)?;
        }
        for group in &self.groups {
            let mut previous = 0u32;
            for &file in &group.file_ids {
                let delta = file
                    .checked_sub(previous)
                    .ok_or(invalid("file ids must be ascending"))?;
                push_count(&mut out, self.protocol, delta)?;
                previous = file;
            }
        }
        if named {
            for group in &self.groups {
                let hashes = group.file_name_hashes.as_deref().unwrap_or_default();
                if hashes.len() != group.file_ids.len() {
                    return Err(invalid("file name hashes must match the file list"));
                }
                for hash in hashes {
                    out.extend_from_slice(&hash.to_be_bytes());
                }
            }
        }
        Ok(out)
    }

    pub fn group(&self, id: u32) -> Option<&GroupEntry> {
        self.groups
            .binary_search_by_key(&id, |group| group.id)
            .ok()
            .map(|index| &self.groups[index])
    }

    pub fn group_mut(&mut self, id: u32) -> Option<&mut GroupEntry> {
        self.groups
            .binary_search_by_key(&id, |group| group.id)
            .ok()
            .map(|index| &mut self.groups[index])
    }
}

#[cfg(test)]
mod tests;
