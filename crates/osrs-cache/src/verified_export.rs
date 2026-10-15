//! Validated, read-only exported cache sources. Ordinary fingerprint matching stays strict.

mod validation;

use crate::profile::TargetProfile;
use crate::source::CacheSource;
use crate::transport::{CacheProbe, CacheRepository, OpenMatchingError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub const EXPORT_FORMAT: &str = "rustosrs-map-export-v2";
pub const CACHE_MANIFEST: &str = "rustosrs-export.json";

/// A changed map group and the exact packed file content the exporter prepared.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportMapGroup {
    pub region: [i32; 2],
    pub group: u32,
    pub payload_sha256: String,
    pub tiles_changed: usize,
    pub heights_changed: usize,
    pub objects_added: usize,
    pub objects_removed: usize,
}

/// Versioned provenance written beside a completed export. No raw keys or cache data are stored.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportManifest {
    pub format: String,
    pub mode: String,
    pub profile: String,
    pub source_fingerprint: String,
    pub edit_revision: u64,
    pub output_sha256: String,
    pub regions: Vec<ExportMapGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportReadError(pub String);

impl std::fmt::Display for ExportReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for ExportReadError {}

pub(super) fn failure(error: impl std::fmt::Display) -> ExportReadError {
    ExportReadError(error.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileStamp {
    name: String,
    len: u64,
    modified: std::time::SystemTime,
}

/// Capability for a fully checked export. Fields are private so callers cannot bypass validation.
#[derive(Debug)]
pub struct VerifiedExport {
    path: PathBuf,
    profile: TargetProfile,
    fingerprint: String,
    stamps: Vec<FileStamp>,
}

impl VerifiedExport {
    /// Validate lineage, all logical groups, preserved files, map metadata and the output digest.
    pub fn validate(
        original: &Path,
        output: &Path,
        manifest: &ExportManifest,
        profiles: &[TargetProfile],
    ) -> Result<Self, ExportReadError> {
        if manifest.format != EXPORT_FORMAT || manifest.mode != "cache-copy" {
            return Err(failure(
                "Unsupported inspection manifest. Export a new cache copy with this editor.",
            ));
        }
        let (parent, profile) =
            CacheRepository::open_matching(original, profiles).map_err(failure)?;
        if parent.fingerprint() != manifest.source_fingerprint || profile.id != manifest.profile {
            return Err(failure("The export belongs to a different original cache."));
        }
        let output = output.canonicalize().map_err(failure)?;
        let before = file_stamps(&output)?;
        if cache_digest(&output)? != manifest.output_sha256 {
            return Err(failure(
                "Export content does not match its manifest digest.",
            ));
        }
        let probe = CacheProbe::of(&output).map_err(failure)?;
        validation::validate_groups(original, &output, &parent, manifest, &probe.content_indices)?;
        if before != file_stamps(&output)? {
            return Err(failure(
                "Export changed during verification; retry after other writers finish.",
            ));
        }
        Ok(Self {
            path: output,
            profile,
            fingerprint: probe.fingerprint,
            stamps: before,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn source(self) -> CacheSource {
        CacheSource::VerifiedExport(Arc::new(self))
    }

    pub(crate) fn open_repository(
        &self,
    ) -> Result<(CacheRepository, TargetProfile), OpenMatchingError> {
        let stamps =
            file_stamps(&self.path).map_err(|e| OpenMatchingError::Unreadable(e.to_string()))?;
        if stamps != self.stamps {
            return Err(OpenMatchingError::Unreadable(
                "Export files changed; validate the export again.".to_owned(),
            ));
        }
        let opened =
            CacheRepository::open_verified_export(&self.path, &self.profile, &self.fingerprint)?;
        if file_stamps(&self.path).map_err(|e| OpenMatchingError::Unreadable(e.to_string()))?
            != self.stamps
        {
            return Err(OpenMatchingError::Unreadable(
                "Export changed while its reader was opening; validate again.".to_owned(),
            ));
        }
        Ok(opened)
    }
}

fn file_stamps(dir: &Path) -> Result<Vec<FileStamp>, ExportReadError> {
    let mut stamps = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(failure)? {
        let entry = entry.map_err(failure)?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == "main_file_cache.dat2"
            || name
                .strip_prefix("main_file_cache.idx")
                .is_some_and(|s| s.parse::<u8>().is_ok())
        {
            if !entry.file_type().map_err(failure)?.is_file() {
                return Err(failure("Cache files must be regular files."));
            }
            let metadata = entry.metadata().map_err(failure)?;
            stamps.push(FileStamp {
                name,
                len: metadata.len(),
                modified: metadata.modified().map_err(failure)?,
            });
        }
    }
    stamps.sort_by(|a, b| a.name.cmp(&b.name));
    if !stamps.iter().any(|s| s.name == "main_file_cache.dat2") {
        return Err(failure("Export has no main_file_cache.dat2."));
    }
    Ok(stamps)
}

/// SHA-256 over the names, lengths and complete stored bytes of every cache file.
pub fn cache_digest(dir: &Path) -> Result<String, ExportReadError> {
    let mut hash = Sha256::new();
    hash.update(b"rustosrs-export-content-v1\0");
    let mut buffer = [0u8; 64 * 1024];
    for stamp in file_stamps(dir)? {
        hash.update((stamp.name.len() as u64).to_be_bytes());
        hash.update(stamp.name.as_bytes());
        hash.update(stamp.len.to_be_bytes());
        let mut file = std::fs::File::open(dir.join(stamp.name)).map_err(failure)?;
        loop {
            let count = file.read(&mut buffer).map_err(failure)?;
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
        }
    }
    Ok(format!("{:x}", hash.finalize()))
}

pub fn payload_digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests;
