//! RustOSRS-owned read-only cache transport and logical-file repository API.
//!
//! `rune-fs` is deliberately confined to this module. Public callers receive
//! RustOSRS-owned metadata, byte containers, provenance, and typed errors.

use crate::decode::{
    ArchiveFileProvenance, DecoderContext, DecoderContextError, XteaKeyProvenance,
    resolve_map_square,
};
use crate::profile::TargetProfile;
use osrs_core::coords::RegionCoord;
use runefs::codec::{Buffer, Encoded};
use runefs::{Dat2, Indices, MAIN_DATA, REFERENCE_TABLE_ID};
use sha2::{Digest, Sha256};
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

const CACHE_FINGERPRINT_DOMAIN: &[u8] = b"rustosrs-cache-v1\0";
const DEFAULT_MAX_ENCODED_GROUP_BYTES: usize = 256 * 1024 * 1024;
const DEFAULT_MAX_DECODED_GROUP_BYTES: usize = 512 * 1024 * 1024;
const DEFAULT_MAX_FILES_PER_GROUP: usize = 65_536;

/// Allocation/splitting policy for cache transport. These are safety limits,
/// not OSRS semantic constants, and callers may select stricter or larger values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheReadLimits {
    pub max_encoded_group_bytes: usize,
    pub max_decoded_group_bytes: usize,
    pub max_files_per_group: usize,
}

impl Default for CacheReadLimits {
    fn default() -> Self {
        Self {
            max_encoded_group_bytes: DEFAULT_MAX_ENCODED_GROUP_BYTES,
            max_decoded_group_bytes: DEFAULT_MAX_DECODED_GROUP_BYTES,
            max_files_per_group: DEFAULT_MAX_FILES_PER_GROUP,
        }
    }
}

/// Caller-owned XTEA input. Raw key words are intentionally not exposed by
/// accessors or `Debug`; only provider/key identity is copied into artifacts.
pub struct XteaInput {
    keys: [u32; 4],
    provenance: XteaKeyProvenance,
}

impl XteaInput {
    pub const fn new(keys: [u32; 4], provenance: XteaKeyProvenance) -> Self {
        Self { keys, provenance }
    }

    pub const fn provenance(&self) -> &XteaKeyProvenance {
        &self.provenance
    }
}

impl fmt::Debug for XteaInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("XteaInput")
            .field("keys", &"<redacted>")
            .field("provenance", &self.provenance)
            .finish()
    }
}

/// RustOSRS-owned reference-table metadata for one logical group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheGroupMetadata {
    pub index_id: u8,
    pub group_id: u32,
    pub name_hash: i32,
    pub crc: u32,
    pub reference_hash: i32,
    pub whirlpool: [u8; 64],
    pub version: u32,
    pub file_ids: Vec<u32>,
    pub encoded_length: usize,
}

/// Bytes for one logical group plus cache identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheGroupBytes {
    pub provenance: ArchiveFileProvenance,
    pub bytes: Vec<u8>,
}

/// Bytes for one logical archive file plus exact index/group/file provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheFile {
    pub provenance: ArchiveFileProvenance,
    pub bytes: Vec<u8>,
}

/// Build-profile-aware terrain/location files for one map square.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapSquareData {
    pub region: RegionCoord,
    pub terrain: CacheFile,
    pub locations: CacheFile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheErrorKind {
    DependencyFailure {
        operation: &'static str,
        detail: String,
    },
    DependencyPanic {
        operation: &'static str,
    },
    MissingIndex,
    MissingGroup,
    MissingFile,
    MissingMetadata,
    LimitExceeded {
        field: &'static str,
        requested: usize,
        limit: usize,
    },
    MalformedContainer {
        detail: String,
    },
    MalformedFileTable {
        detail: String,
    },
    FingerprintMismatch {
        expected: String,
        actual: String,
    },
    MapResolution {
        detail: String,
    },
}

/// Provenance-rich transport/repository failure. No `rune-fs` type is exposed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheError {
    profile_id: String,
    build: u32,
    index_id: Option<u8>,
    group_id: Option<u32>,
    file_id: Option<u32>,
    kind: CacheErrorKind,
}

impl CacheError {
    fn new(context: &DecoderContext, kind: CacheErrorKind) -> Self {
        Self {
            profile_id: context.target_provenance().profile_id().to_owned(),
            build: context.build(),
            index_id: None,
            group_id: None,
            file_id: None,
            kind,
        }
    }

    fn at_group(mut self, index_id: u8, group_id: u32) -> Self {
        self.index_id = Some(index_id);
        self.group_id = Some(group_id);
        self
    }

    fn at_file(mut self, index_id: u8, group_id: u32, file_id: u32) -> Self {
        self.index_id = Some(index_id);
        self.group_id = Some(group_id);
        self.file_id = Some(file_id);
        self
    }

    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }

    pub const fn build(&self) -> u32 {
        self.build
    }

    pub const fn index_id(&self) -> Option<u8> {
        self.index_id
    }

    pub const fn group_id(&self) -> Option<u32> {
        self.group_id
    }

    pub const fn file_id(&self) -> Option<u32> {
        self.file_id
    }

    pub const fn kind(&self) -> &CacheErrorKind {
        &self.kind
    }
}

impl fmt::Display for CacheError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cache transport error [profile={}, build={}",
            self.profile_id, self.build
        )?;
        if let Some(index_id) = self.index_id {
            write!(formatter, ", index={index_id}")?;
        }
        if let Some(group_id) = self.group_id {
            write!(formatter, ", group={group_id}")?;
        }
        if let Some(file_id) = self.file_id {
            write!(formatter, ", file={file_id}")?;
        }
        write!(formatter, "]: ")?;
        match &self.kind {
            CacheErrorKind::DependencyFailure { operation, detail } => {
                write!(formatter, "{operation} failed: {detail}")
            }
            CacheErrorKind::DependencyPanic { operation } => {
                write!(
                    formatter,
                    "{operation} panicked inside private transport dependency"
                )
            }
            CacheErrorKind::MissingIndex => formatter.write_str("index is not present"),
            CacheErrorKind::MissingGroup => formatter.write_str("group is not present"),
            CacheErrorKind::MissingFile => formatter.write_str("file is not present"),
            CacheErrorKind::MissingMetadata => formatter.write_str("group metadata is not present"),
            CacheErrorKind::LimitExceeded {
                field,
                requested,
                limit,
            } => write!(
                formatter,
                "{field} exceeds transport limit: requested {requested}, limit {limit}"
            ),
            CacheErrorKind::MalformedContainer { detail } => {
                write!(formatter, "malformed group compression container: {detail}")
            }
            CacheErrorKind::MalformedFileTable { detail } => {
                write!(formatter, "malformed logical-file table: {detail}")
            }
            CacheErrorKind::FingerprintMismatch { expected, actual } => write!(
                formatter,
                "cache fingerprint mismatch: expected {expected}, actual {actual}"
            ),
            CacheErrorKind::MapResolution { detail } => write!(formatter, "{detail}"),
        }
    }
}

impl std::error::Error for CacheError {}

#[derive(Debug)]
pub enum CacheRepositoryOpenError {
    InvalidTarget(DecoderContextError),
    Cache(CacheError),
}

impl fmt::Display for CacheRepositoryOpenError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTarget(error) => write!(formatter, "invalid cache target: {error}"),
            Self::Cache(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CacheRepositoryOpenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidTarget(error) => Some(error),
            Self::Cache(error) => Some(error),
        }
    }
}

/// Verified target cache repository. Construction validates the target profile,
/// opens the private transport, and proves the complete cache fingerprint before
/// semantic decoding is allowed.
pub struct CacheRepository {
    transport: CacheTransport,
    context: DecoderContext,
    limits: CacheReadLimits,
    fingerprint: ComputedFingerprint,
}

impl CacheRepository {
    pub fn open(
        path: impl AsRef<Path>,
        profile: &TargetProfile,
    ) -> Result<Self, CacheRepositoryOpenError> {
        Self::open_with_limits(path, profile, CacheReadLimits::default())
    }

    pub fn open_with_limits(
        path: impl AsRef<Path>,
        profile: &TargetProfile,
        limits: CacheReadLimits,
    ) -> Result<Self, CacheRepositoryOpenError> {
        let context = DecoderContext::from_profile(profile)
            .map_err(CacheRepositoryOpenError::InvalidTarget)?;
        validate_limits(&context, limits).map_err(CacheRepositoryOpenError::Cache)?;

        let transport = dependency_call(&context, "open cache", || CacheTransport::open(path))
            .map_err(CacheRepositoryOpenError::Cache)?;
        let fingerprint = dependency_call(&context, "fingerprint cache", || {
            transport.fingerprint_v1(limits.max_encoded_group_bytes)
        })
        .map_err(CacheRepositoryOpenError::Cache)?;
        let expected = context.target_provenance().cache_fingerprint().to_string();
        let actual = fingerprint.to_hex();
        if expected != actual {
            return Err(CacheRepositoryOpenError::Cache(CacheError::new(
                &context,
                CacheErrorKind::FingerprintMismatch { expected, actual },
            )));
        }

        Ok(Self {
            transport,
            context,
            limits,
            fingerprint,
        })
    }

    pub const fn context(&self) -> &DecoderContext {
        &self.context
    }

    pub fn fingerprint(&self) -> String {
        self.fingerprint.to_hex()
    }

    pub fn content_index_ids(&self) -> Vec<u8> {
        self.transport.content_index_ids()
    }

    pub fn group_ids(&self, index_id: u8) -> Result<Vec<u32>, CacheError> {
        self.transport
            .group_ids(index_id)
            .map_err(|error| self.map_transport_error(error, Some(index_id), None, None))
    }

    pub fn group_metadata(
        &self,
        index_id: u8,
        group_id: u32,
    ) -> Result<CacheGroupMetadata, CacheError> {
        let metadata = self
            .transport
            .group_metadata(index_id, group_id)
            .map_err(|error| {
                self.map_transport_error(error, Some(index_id), Some(group_id), None)
            })?;
        if metadata.file_ids.len() > self.limits.max_files_per_group {
            return Err(CacheError::new(
                &self.context,
                CacheErrorKind::LimitExceeded {
                    field: "files per group",
                    requested: metadata.file_ids.len(),
                    limit: self.limits.max_files_per_group,
                },
            )
            .at_group(index_id, group_id));
        }
        Ok(metadata)
    }

    /// Read exact encoded reference-table bytes for one logical content index.
    pub fn read_encoded_reference_table(
        &self,
        index_id: u8,
    ) -> Result<CacheGroupBytes, CacheError> {
        let group_id = u32::from(index_id);
        let bytes = self.read_dependency_group(
            "read encoded reference table",
            REFERENCE_TABLE_ID,
            group_id,
            None,
            false,
        )?;
        Ok(CacheGroupBytes {
            provenance: ArchiveFileProvenance::new(REFERENCE_TABLE_ID, group_id, None),
            bytes,
        })
    }

    pub fn read_encoded_group(
        &self,
        index_id: u8,
        group_id: u32,
    ) -> Result<CacheGroupBytes, CacheError> {
        let bytes =
            self.read_dependency_group("read encoded group", index_id, group_id, None, false)?;
        Ok(CacheGroupBytes {
            provenance: ArchiveFileProvenance::new(index_id, group_id, None),
            bytes,
        })
    }

    pub fn read_decoded_group(
        &self,
        index_id: u8,
        group_id: u32,
        xtea: Option<&XteaInput>,
    ) -> Result<CacheGroupBytes, CacheError> {
        let keys = xtea.map(|input| input.keys);
        let bytes = self.read_dependency_group("decode group", index_id, group_id, keys, true)?;
        let mut provenance = ArchiveFileProvenance::new(index_id, group_id, None);
        if let Some(input) = xtea {
            provenance = provenance.with_xtea(input.provenance.clone());
        }
        Ok(CacheGroupBytes { provenance, bytes })
    }

    pub fn read_file(
        &self,
        index_id: u8,
        group_id: u32,
        file_id: u32,
        xtea: Option<&XteaInput>,
    ) -> Result<CacheFile, CacheError> {
        let metadata = self.group_metadata(index_id, group_id)?;
        let decoded = self.read_decoded_group(index_id, group_id, xtea)?;
        let files = split_group_files(
            &decoded.bytes,
            &metadata.file_ids,
            self.limits.max_files_per_group,
        )
        .map_err(|kind| CacheError::new(&self.context, kind).at_group(index_id, group_id))?;

        let bytes = files
            .into_iter()
            .find_map(|(id, bytes)| (id == file_id).then_some(bytes))
            .ok_or_else(|| {
                CacheError::new(&self.context, CacheErrorKind::MissingFile)
                    .at_file(index_id, group_id, file_id)
            })?;

        let mut provenance = ArchiveFileProvenance::new(index_id, group_id, Some(file_id));
        if let Some(input) = xtea {
            provenance = provenance.with_xtea(input.provenance.clone());
        }
        Ok(CacheFile { provenance, bytes })
    }

    pub fn read_map_square(&self, region: RegionCoord) -> Result<MapSquareData, CacheError> {
        let resolved = resolve_map_square(&self.context, region).map_err(|error| {
            CacheError::new(
                &self.context,
                CacheErrorKind::MapResolution {
                    detail: error.to_string(),
                },
            )
        })?;
        let terrain_file = resolved.terrain().file_id().ok_or_else(|| {
            CacheError::new(
                &self.context,
                CacheErrorKind::MapResolution {
                    detail: "resolved terrain source did not contain a file id".to_owned(),
                },
            )
        })?;
        let location_file = resolved.locations().file_id().ok_or_else(|| {
            CacheError::new(
                &self.context,
                CacheErrorKind::MapResolution {
                    detail: "resolved location source did not contain a file id".to_owned(),
                },
            )
        })?;

        Ok(MapSquareData {
            region,
            terrain: self.read_file(
                resolved.terrain().index_id(),
                resolved.group_id(),
                terrain_file,
                None,
            )?,
            locations: self.read_file(
                resolved.locations().index_id(),
                resolved.group_id(),
                location_file,
                None,
            )?,
        })
    }

    fn read_dependency_group(
        &self,
        operation: &'static str,
        index_id: u8,
        group_id: u32,
        xtea: Option<[u32; 4]>,
        decode: bool,
    ) -> Result<Vec<u8>, CacheError> {
        let result = dependency_call(&self.context, operation, || {
            if decode {
                self.transport.read_decoded_group(
                    index_id,
                    group_id,
                    xtea,
                    self.limits.max_encoded_group_bytes,
                    self.limits.max_decoded_group_bytes,
                )
            } else {
                self.transport.read_encoded_group_limited(
                    index_id,
                    group_id,
                    self.limits.max_encoded_group_bytes,
                )
            }
        });
        result.map_err(|error| match error.kind {
            CacheErrorKind::MissingIndex
            | CacheErrorKind::MissingGroup
            | CacheErrorKind::DependencyFailure { .. }
            | CacheErrorKind::DependencyPanic { .. }
            | CacheErrorKind::LimitExceeded { .. }
            | CacheErrorKind::MalformedContainer { .. } => error.at_group(index_id, group_id),
            _ => error.at_group(index_id, group_id),
        })
    }

    fn map_transport_error(
        &self,
        error: TransportError,
        index_id: Option<u8>,
        group_id: Option<u32>,
        file_id: Option<u32>,
    ) -> CacheError {
        let kind = match error {
            TransportError::Dependency(error) => CacheErrorKind::DependencyFailure {
                operation: "cache metadata",
                detail: error,
            },
            TransportError::MissingIndex(_) => CacheErrorKind::MissingIndex,
            TransportError::MissingGroup { .. } => CacheErrorKind::MissingGroup,
            TransportError::MissingMetadata { .. } => CacheErrorKind::MissingMetadata,
            TransportError::LimitExceeded {
                field,
                requested,
                limit,
            } => CacheErrorKind::LimitExceeded {
                field,
                requested,
                limit,
            },
            TransportError::MalformedContainer(detail) => {
                CacheErrorKind::MalformedContainer { detail }
            }
        };
        let mut mapped = CacheError::new(&self.context, kind);
        mapped.index_id = index_id;
        mapped.group_id = group_id;
        mapped.file_id = file_id;
        mapped
    }
}

fn validate_limits(context: &DecoderContext, limits: CacheReadLimits) -> Result<(), CacheError> {
    for (field, value) in [
        ("max encoded group bytes", limits.max_encoded_group_bytes),
        ("max decoded group bytes", limits.max_decoded_group_bytes),
        ("max files per group", limits.max_files_per_group),
    ] {
        if value == 0 {
            return Err(CacheError::new(
                context,
                CacheErrorKind::LimitExceeded {
                    field,
                    requested: 0,
                    limit: 0,
                },
            ));
        }
    }
    Ok(())
}

fn dependency_call<T>(
    context: &DecoderContext,
    operation: &'static str,
    action: impl FnOnce() -> TransportResult<T>,
) -> Result<T, CacheError> {
    match catch_unwind(AssertUnwindSafe(action)) {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => {
            let kind = match error {
                TransportError::Dependency(detail) => {
                    CacheErrorKind::DependencyFailure { operation, detail }
                }
                TransportError::MissingIndex(_) => CacheErrorKind::MissingIndex,
                TransportError::MissingGroup { .. } => CacheErrorKind::MissingGroup,
                TransportError::MissingMetadata { .. } => CacheErrorKind::MissingMetadata,
                TransportError::LimitExceeded {
                    field,
                    requested,
                    limit,
                } => CacheErrorKind::LimitExceeded {
                    field,
                    requested,
                    limit,
                },
                TransportError::MalformedContainer(detail) => {
                    CacheErrorKind::MalformedContainer { detail }
                }
            };
            Err(CacheError::new(context, kind))
        }
        Err(_) => Err(CacheError::new(
            context,
            CacheErrorKind::DependencyPanic { operation },
        )),
    }
}

fn split_group_files(
    decoded: &[u8],
    file_ids: &[u32],
    max_files: usize,
) -> Result<Vec<(u32, Vec<u8>)>, CacheErrorKind> {
    if file_ids.is_empty() {
        return Err(CacheErrorKind::MalformedFileTable {
            detail: "reference metadata declares zero logical files".to_owned(),
        });
    }
    if file_ids.len() > max_files {
        return Err(CacheErrorKind::LimitExceeded {
            field: "files per group",
            requested: file_ids.len(),
            limit: max_files,
        });
    }
    if file_ids.len() == 1 {
        return Ok(vec![(file_ids[0], decoded.to_vec())]);
    }
    let Some(&chunk_byte) = decoded.last() else {
        return Err(CacheErrorKind::MalformedFileTable {
            detail: "multi-file group is empty".to_owned(),
        });
    };
    let chunk_count = usize::from(chunk_byte);
    if chunk_count == 0 {
        return Err(CacheErrorKind::MalformedFileTable {
            detail: "multi-file group declares zero chunks".to_owned(),
        });
    }

    let table_values = chunk_count.checked_mul(file_ids.len()).ok_or_else(|| {
        CacheErrorKind::MalformedFileTable {
            detail: "chunk/file table entry count overflowed usize".to_owned(),
        }
    })?;
    let table_bytes =
        table_values
            .checked_mul(4)
            .ok_or_else(|| CacheErrorKind::MalformedFileTable {
                detail: "chunk/file table byte size overflowed usize".to_owned(),
            })?;
    let trailer_bytes =
        table_bytes
            .checked_add(1)
            .ok_or_else(|| CacheErrorKind::MalformedFileTable {
                detail: "chunk/file trailer size overflowed usize".to_owned(),
            })?;
    let table_start = decoded.len().checked_sub(trailer_bytes).ok_or_else(|| {
        CacheErrorKind::MalformedFileTable {
            detail: "chunk table extends before start of decoded group".to_owned(),
        }
    })?;

    let mut totals = vec![0usize; file_ids.len()];
    let mut table_offset = table_start;
    for _ in 0..chunk_count {
        let mut chunk_size = 0i64;
        for total in &mut totals {
            let delta = read_table_i32(decoded, &mut table_offset)?;
            chunk_size = chunk_size.checked_add(i64::from(delta)).ok_or_else(|| {
                CacheErrorKind::MalformedFileTable {
                    detail: "chunk size accumulator overflowed i64".to_owned(),
                }
            })?;
            let chunk_size =
                usize::try_from(chunk_size).map_err(|_| CacheErrorKind::MalformedFileTable {
                    detail: "chunk table produced a negative file segment size".to_owned(),
                })?;
            *total = total.checked_add(chunk_size).ok_or_else(|| {
                CacheErrorKind::MalformedFileTable {
                    detail: "logical file length overflowed usize".to_owned(),
                }
            })?;
            if *total > table_start {
                return Err(CacheErrorKind::MalformedFileTable {
                    detail: "logical file length exceeds decoded data area".to_owned(),
                });
            }
        }
    }
    if table_offset != decoded.len() - 1 {
        return Err(CacheErrorKind::MalformedFileTable {
            detail: "chunk table length did not match trailer".to_owned(),
        });
    }
    let combined = totals
        .iter()
        .try_fold(0usize, |sum, value| sum.checked_add(*value));
    if combined != Some(table_start) {
        return Err(CacheErrorKind::MalformedFileTable {
            detail: "logical file lengths do not exactly cover decoded data area".to_owned(),
        });
    }

    let mut files = totals
        .iter()
        .map(|size| Vec::with_capacity(*size))
        .collect::<Vec<_>>();
    table_offset = table_start;
    let mut data_offset = 0usize;
    for _ in 0..chunk_count {
        let mut chunk_size = 0i64;
        for file in &mut files {
            let delta = read_table_i32(decoded, &mut table_offset)?;
            chunk_size = chunk_size.checked_add(i64::from(delta)).ok_or_else(|| {
                CacheErrorKind::MalformedFileTable {
                    detail: "chunk size accumulator overflowed i64".to_owned(),
                }
            })?;
            let chunk_size =
                usize::try_from(chunk_size).map_err(|_| CacheErrorKind::MalformedFileTable {
                    detail: "chunk table produced a negative file segment size".to_owned(),
                })?;
            let end = data_offset.checked_add(chunk_size).ok_or_else(|| {
                CacheErrorKind::MalformedFileTable {
                    detail: "decoded data cursor overflowed usize".to_owned(),
                }
            })?;
            let segment = decoded.get(data_offset..end).ok_or_else(|| {
                CacheErrorKind::MalformedFileTable {
                    detail: "chunk data extends into the file table".to_owned(),
                }
            })?;
            if end > table_start {
                return Err(CacheErrorKind::MalformedFileTable {
                    detail: "chunk data extends into the file table".to_owned(),
                });
            }
            file.extend_from_slice(segment);
            data_offset = end;
        }
    }
    if data_offset != table_start {
        return Err(CacheErrorKind::MalformedFileTable {
            detail: "chunk data did not exactly consume decoded data area".to_owned(),
        });
    }

    Ok(file_ids.iter().copied().zip(files).collect())
}

fn read_table_i32(decoded: &[u8], offset: &mut usize) -> Result<i32, CacheErrorKind> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| CacheErrorKind::MalformedFileTable {
            detail: "chunk table cursor overflowed usize".to_owned(),
        })?;
    let bytes = decoded
        .get(*offset..end)
        .ok_or_else(|| CacheErrorKind::MalformedFileTable {
            detail: "truncated chunk table".to_owned(),
        })?;
    *offset = end;
    Ok(i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

#[derive(Debug)]
enum TransportError {
    Dependency(String),
    MissingIndex(u8),
    MissingGroup {
        index: u8,
        group: u32,
    },
    MissingMetadata {
        index: u8,
        group: u32,
    },
    LimitExceeded {
        field: &'static str,
        requested: usize,
        limit: usize,
    },
    MalformedContainer(String),
}

impl From<runefs::Error> for TransportError {
    fn from(value: runefs::Error) -> Self {
        Self::Dependency(value.to_string())
    }
}

type TransportResult<T> = std::result::Result<T, TransportError>;

#[derive(Clone, Copy, Eq, PartialEq, Hash)]
struct ComputedFingerprint([u8; 32]);

impl ComputedFingerprint {
    fn to_hex(self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut output = String::with_capacity(64);
        for byte in self.0 {
            output.push(HEX[(byte >> 4) as usize] as char);
            output.push(HEX[(byte & 0x0f) as usize] as char);
        }
        output
    }
}

impl fmt::Debug for ComputedFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("ComputedFingerprint")
            .field(&self.to_hex())
            .finish()
    }
}

struct CacheTransport {
    data: Dat2,
    indices: Indices,
}

impl CacheTransport {
    fn open(path: impl AsRef<Path>) -> TransportResult<Self> {
        let path = path.as_ref();
        Ok(Self {
            data: Dat2::new(path.join(MAIN_DATA))?,
            indices: Indices::new(path)?,
        })
    }

    fn content_index_ids(&self) -> Vec<u8> {
        let mut ids: Vec<u8> = (&self.indices)
            .into_iter()
            .map(|(id, _)| *id)
            .filter(|id| *id != REFERENCE_TABLE_ID)
            .collect();
        ids.sort_unstable();
        ids
    }

    fn group_ids(&self, index_id: u8) -> TransportResult<Vec<u32>> {
        let index = self
            .indices
            .get(&index_id)
            .ok_or(TransportError::MissingIndex(index_id))?;
        let mut ids: Vec<u32> = index.metadata.iter().map(|metadata| metadata.id).collect();
        ids.sort_unstable();
        Ok(ids)
    }

    fn group_count(&self) -> TransportResult<usize> {
        self.content_index_ids()
            .into_iter()
            .try_fold(0usize, |total, index| {
                self.group_ids(index).and_then(|groups| {
                    total
                        .checked_add(groups.len())
                        .ok_or(TransportError::LimitExceeded {
                            field: "group count",
                            requested: usize::MAX,
                            limit: usize::MAX - total,
                        })
                })
            })
    }

    fn group_id_by_name(&self, index_id: u8, name: &str) -> TransportResult<u32> {
        let index = self
            .indices
            .get(&index_id)
            .ok_or(TransportError::MissingIndex(index_id))?;
        let hash = jagex_name_hash(name);
        index
            .metadata
            .iter()
            .find(|metadata| metadata.name_hash == hash)
            .map(|metadata| metadata.id)
            .ok_or(TransportError::MissingGroup {
                index: index_id,
                group: 0,
            })
    }

    fn group_metadata(&self, index_id: u8, group_id: u32) -> TransportResult<CacheGroupMetadata> {
        let index = self
            .indices
            .get(&index_id)
            .ok_or(TransportError::MissingIndex(index_id))?;
        let metadata = index
            .metadata
            .iter()
            .find(|metadata| metadata.id == group_id)
            .ok_or(TransportError::MissingMetadata {
                index: index_id,
                group: group_id,
            })?;
        let archive = index
            .archive_refs
            .get(&group_id)
            .ok_or(TransportError::MissingGroup {
                index: index_id,
                group: group_id,
            })?;
        Ok(CacheGroupMetadata {
            index_id,
            group_id,
            name_hash: metadata.name_hash,
            crc: metadata.crc,
            reference_hash: metadata.hash,
            whirlpool: metadata.whirlpool,
            version: metadata.version,
            file_ids: metadata.valid_ids.clone(),
            encoded_length: archive.length,
        })
    }

    fn archive_ref(&self, index_id: u8, group_id: u32) -> TransportResult<&runefs::ArchiveRef> {
        let index = self
            .indices
            .get(&index_id)
            .ok_or(TransportError::MissingIndex(index_id))?;
        index
            .archive_refs
            .get(&group_id)
            .ok_or(TransportError::MissingGroup {
                index: index_id,
                group: group_id,
            })
    }

    fn read_encoded_group_limited(
        &self,
        index_id: u8,
        group_id: u32,
        max_encoded_bytes: usize,
    ) -> TransportResult<Vec<u8>> {
        let archive = self.archive_ref(index_id, group_id)?;
        if archive.length > max_encoded_bytes {
            return Err(TransportError::LimitExceeded {
                field: "encoded group bytes",
                requested: archive.length,
                limit: max_encoded_bytes,
            });
        }
        Ok(self.data.read(archive)?.finalize())
    }

    fn read_decoded_group(
        &self,
        index_id: u8,
        group_id: u32,
        xtea: Option<[u32; 4]>,
        max_encoded_bytes: usize,
        max_decoded_bytes: usize,
    ) -> TransportResult<Vec<u8>> {
        let encoded = self.read_encoded_group_limited(index_id, group_id, max_encoded_bytes)?;
        validate_encoded_container(&encoded, max_encoded_bytes, max_decoded_bytes)?;
        let mut buffer = Buffer::<Encoded>::from(encoded);
        if let Some(keys) = xtea {
            buffer = buffer.with_xtea_keys(keys);
        }
        let decoded = buffer.decode()?.finalize();
        if decoded.len() > max_decoded_bytes {
            return Err(TransportError::LimitExceeded {
                field: "decoded group bytes",
                requested: decoded.len(),
                limit: max_decoded_bytes,
            });
        }
        Ok(decoded)
    }

    fn read_encoded_reference_table_limited(
        &self,
        index_id: u8,
        max_encoded_bytes: usize,
    ) -> TransportResult<Vec<u8>> {
        self.read_encoded_group_limited(REFERENCE_TABLE_ID, u32::from(index_id), max_encoded_bytes)
    }

    fn fingerprint_v1(&self, max_encoded_bytes: usize) -> TransportResult<ComputedFingerprint> {
        let mut digest = Sha256::new();
        digest.update(CACHE_FINGERPRINT_DOMAIN);

        for index_id in self.content_index_ids() {
            digest.update(u16::from(index_id).to_be_bytes());
            let reference_bytes =
                self.read_encoded_reference_table_limited(index_id, max_encoded_bytes)?;
            update_hashed_blob(&mut digest, &reference_bytes);

            for group_id in self.group_ids(index_id)? {
                digest.update(group_id.to_be_bytes());
                let group =
                    self.read_encoded_group_limited(index_id, group_id, max_encoded_bytes)?;
                update_hashed_blob(&mut digest, &group);
            }
        }

        Ok(ComputedFingerprint(digest.finalize().into()))
    }
}

fn validate_encoded_container(
    encoded: &[u8],
    max_encoded_bytes: usize,
    max_decoded_bytes: usize,
) -> TransportResult<()> {
    if encoded.len() > max_encoded_bytes {
        return Err(TransportError::LimitExceeded {
            field: "encoded group bytes",
            requested: encoded.len(),
            limit: max_encoded_bytes,
        });
    }
    if encoded.len() < 5 {
        return Err(TransportError::MalformedContainer(
            "container is shorter than compression byte plus length".to_owned(),
        ));
    }
    let compression = encoded[0];
    if compression > 2 {
        return Err(TransportError::MalformedContainer(format!(
            "unsupported OSRS compression id {compression}"
        )));
    }
    let compressed_len =
        u32::from_be_bytes([encoded[1], encoded[2], encoded[3], encoded[4]]) as usize;
    if compressed_len > max_encoded_bytes {
        return Err(TransportError::LimitExceeded {
            field: "compressed payload bytes",
            requested: compressed_len,
            limit: max_encoded_bytes,
        });
    }

    let (header_len, decoded_len) = if compression == 0 {
        (5usize, compressed_len)
    } else {
        if encoded.len() < 9 {
            return Err(TransportError::MalformedContainer(
                "compressed container is missing decompressed length".to_owned(),
            ));
        }
        let decoded_len =
            u32::from_be_bytes([encoded[5], encoded[6], encoded[7], encoded[8]]) as usize;
        (9usize, decoded_len)
    };
    if decoded_len > max_decoded_bytes {
        return Err(TransportError::LimitExceeded {
            field: "declared decoded group bytes",
            requested: decoded_len,
            limit: max_decoded_bytes,
        });
    }
    let payload_end = header_len.checked_add(compressed_len).ok_or_else(|| {
        TransportError::MalformedContainer("payload end overflowed usize".to_owned())
    })?;
    if payload_end > encoded.len() {
        return Err(TransportError::MalformedContainer(format!(
            "declared compressed payload ends at {payload_end}, container length is {}",
            encoded.len()
        )));
    }
    let trailing = encoded.len() - payload_end;
    if trailing != 0 && trailing != 2 {
        return Err(TransportError::MalformedContainer(format!(
            "container has {trailing} trailing bytes; expected zero or two-byte version"
        )));
    }
    Ok(())
}

fn update_hashed_blob(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_be_bytes());
    digest.update(Sha256::digest(bytes));
}

fn jagex_name_hash(name: &str) -> i32 {
    name.chars().fold(0i32, |hash, character| {
        hash.wrapping_mul(31).wrapping_add(character as i32)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::path::PathBuf;

    const MAP_INDEX: u8 = 5;
    const CONFIG_INDEX: u8 = 2;
    const MODEL_INDEX: u8 = 7;
    const LUMBRIDGE_REGION: &str = "50_50";
    const LEGACY_LUMBRIDGE_XTEA: [u32; 4] =
        [3_030_157_619, 2_364_842_415, 3_297_319_647, 1_973_582_566];
    const LEGACY_CACHE_FINGERPRINT: &str =
        "ad37f18dedd911eba2085d06029f2edf5db3c6285e56f7cb38eddd1cdce04636";
    const BUILD_241_CACHE_FINGERPRINT: &str =
        "ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38";

    fn repository_cache() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../rs-cache-master/data/osrs_cache")
    }

    #[test]
    fn name_hash_matches_known_rs_cache_example() {
        assert_eq!(jagex_name_hash("huffman"), 1_258_058_669);
    }

    #[test]
    fn legacy_cache_enumerates_indices_groups_and_metadata() -> TransportResult<()> {
        let cache = CacheTransport::open(repository_cache())?;
        let indices = cache.content_index_ids();
        assert!(indices.contains(&0));
        assert!(indices.contains(&CONFIG_INDEX));
        assert!(indices.contains(&MAP_INDEX));
        assert!(!indices.contains(&REFERENCE_TABLE_ID));
        let groups = cache.group_ids(MAP_INDEX)?;
        assert!(!groups.is_empty());
        let metadata = cache.group_metadata(MAP_INDEX, groups[0])?;
        assert_eq!(metadata.index_id, MAP_INDEX);
        assert_eq!(metadata.group_id, groups[0]);
        assert!(!metadata.file_ids.is_empty());
        Ok(())
    }

    #[test]
    fn legacy_cache_supports_name_lookup_and_plain_decompression() -> TransportResult<()> {
        let cache = CacheTransport::open(repository_cache())?;
        let map_group = cache.group_id_by_name(MAP_INDEX, &format!("m{LUMBRIDGE_REGION}"))?;
        let encoded = cache.read_encoded_group_limited(MAP_INDEX, map_group, usize::MAX)?;
        let decoded =
            cache.read_decoded_group(MAP_INDEX, map_group, None, usize::MAX, usize::MAX)?;
        assert!(!encoded.is_empty());
        assert!(!decoded.is_empty());
        Ok(())
    }

    #[test]
    fn legacy_cache_supports_lumbridge_xtea_boundary() -> TransportResult<()> {
        let cache = CacheTransport::open(repository_cache())?;
        let loc_group = cache.group_id_by_name(MAP_INDEX, &format!("l{LUMBRIDGE_REGION}"))?;
        let decoded = cache.read_decoded_group(
            MAP_INDEX,
            loc_group,
            Some(LEGACY_LUMBRIDGE_XTEA),
            usize::MAX,
            usize::MAX,
        )?;
        assert!(!decoded.is_empty());
        Ok(())
    }

    #[test]
    fn fingerprint_v1_is_deterministic_for_legacy_fixture() -> TransportResult<()> {
        let cache = CacheTransport::open(repository_cache())?;
        let first = cache.fingerprint_v1(usize::MAX)?;
        let second = cache.fingerprint_v1(usize::MAX)?;
        assert_eq!(first, second);
        assert_eq!(first.to_hex(), LEGACY_CACHE_FINGERPRINT);
        Ok(())
    }

    #[test]
    fn safe_file_split_preserves_sparse_file_ids_and_negative_deltas() {
        let mut decoded = b"abcde".to_vec();
        decoded.extend_from_slice(&3i32.to_be_bytes());
        decoded.extend_from_slice(&(-1i32).to_be_bytes());
        decoded.push(1);

        let files = split_group_files(&decoded, &[0, 7], 8);
        assert_eq!(files, Ok(vec![(0, b"abc".to_vec()), (7, b"de".to_vec())]));
    }

    #[test]
    fn safe_file_split_rejects_truncated_or_inconsistent_tables() {
        let truncated = split_group_files(&[1], &[0, 1], 8);
        assert!(matches!(
            truncated,
            Err(CacheErrorKind::MalformedFileTable { .. })
        ));

        let mut inconsistent = b"abc".to_vec();
        inconsistent.extend_from_slice(&2i32.to_be_bytes());
        inconsistent.extend_from_slice(&2i32.to_be_bytes());
        inconsistent.push(1);
        assert!(matches!(
            split_group_files(&inconsistent, &[0, 1], 8),
            Err(CacheErrorKind::MalformedFileTable { .. })
        ));
    }

    #[test]
    fn container_validation_rejects_unbounded_declared_output_before_decode() {
        let bytes = [2, 0, 0, 0, 1, 0x7f, 0xff, 0xff, 0xff, 0];
        assert!(matches!(
            validate_encoded_container(&bytes, 1024, 4096),
            Err(TransportError::LimitExceeded {
                field: "declared decoded group bytes",
                ..
            })
        ));
    }

    #[test]
    fn configured_build_241_cache_matches_pinned_source_shape() -> TransportResult<()> {
        let Some(path) = env::var_os("RUSTOSRS_TARGET_CACHE_DIR") else {
            return Ok(());
        };
        let cache = CacheTransport::open(path)?;
        let indices = cache.content_index_ids();
        let group_count = cache.group_count()?;
        eprintln!("build-241 physical content indices={indices:?}");
        eprintln!("build-241 physical group count={group_count}");

        assert_eq!(indices.len(), 23);
        assert!(!indices.contains(&16));
        assert!(!indices.contains(&23));
        assert!(indices.contains(&CONFIG_INDEX));
        assert!(indices.contains(&MAP_INDEX));
        assert!(indices.contains(&MODEL_INDEX));
        assert_eq!(group_count, 117_584);

        let map_index = cache
            .indices
            .get(&MAP_INDEX)
            .ok_or(TransportError::MissingIndex(MAP_INDEX))?;
        let named_group_count = map_index
            .metadata
            .iter()
            .filter(|metadata| metadata.name_hash != 0)
            .count();
        eprintln!("build-241 map groups with nonzero name hashes={named_group_count}");
        assert_eq!(named_group_count, 0);

        let map_groups = cache.group_ids(MAP_INDEX)?;
        let Some(map_group) = map_groups.first().copied() else {
            return Err(TransportError::MissingGroup {
                index: MAP_INDEX,
                group: 0,
            });
        };
        let metadata = cache.group_metadata(MAP_INDEX, map_group)?;
        assert_eq!(metadata.file_ids, vec![0, 1]);
        let model_groups = cache.group_ids(MODEL_INDEX)?;
        let Some(model_group) = model_groups.first().copied() else {
            return Err(TransportError::MissingGroup {
                index: MODEL_INDEX,
                group: 0,
            });
        };
        assert!(
            !cache
                .read_decoded_group(MAP_INDEX, map_group, None, usize::MAX, usize::MAX)?
                .is_empty()
        );
        assert!(
            !cache
                .read_decoded_group(MODEL_INDEX, model_group, None, usize::MAX, usize::MAX)?
                .is_empty()
        );
        assert!(
            !cache
                .read_decoded_group(CONFIG_INDEX, 6, None, usize::MAX, usize::MAX)?
                .is_empty()
        );

        let fingerprint = cache.fingerprint_v1(usize::MAX)?;
        assert_eq!(fingerprint.to_hex(), BUILD_241_CACHE_FINGERPRINT);
        Ok(())
    }
}
