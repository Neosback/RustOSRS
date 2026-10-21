//! Versioned, validated map/patch transport for reproduction infrastructure.
//!
//! Private records are wire data only. Canonical constructors establish semantic values;
//! no second document, history or operation executor lives here. See semantic-transport.md.
mod values;
use crate::{HeightVertexCoord, MapDocument, MapPatch, MapPosition, ObjectPlacement};
use osrs_core::provenance::{Digest256, DigestParseError};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashSet};
use values::{HeightRecord, ObjectRecord, OperationRecord, TerrainRecord};

/// Complete map transport schema; source/profile/availability/workspace domains are separate.
pub const MAP_SNAPSHOT_FORMAT_V1: &str = "rustosrs-map-snapshot-v1";
/// Exact ordered map transaction transport schema; decoding never executes it.
pub const MAP_PATCH_FORMAT_V1: &str = "rustosrs-map-patch-json-v1";

/// A transport cannot enter semantic authority until every record and content digest is valid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticTransportError {
    Json(String),
    /// Value construction failed in a particular snapshot collection or patch operation.
    Record {
        collection: &'static str,
        index: usize,
        error: Box<Self>,
    },
    UnsupportedFormat {
        expected: &'static str,
        actual: String,
    },
    UnsupportedDigest {
        expected: &'static str,
        actual: String,
    },
    InvalidDigest(DigestParseError),
    InvalidValue {
        field: &'static str,
        value: u8,
    },
    DuplicateTerrain(MapPosition),
    DuplicateHeight(HeightVertexCoord),
    DuplicateObject(ObjectPlacement),
    DigestMismatch {
        expected: Digest256,
        actual: Digest256,
    },
}
impl std::fmt::Display for SemanticTransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json(error) => write!(f, "Semantic transport JSON: {error}"),
            Self::Record {
                collection,
                index,
                error,
            } => write!(f, "{collection}[{index}]: {error}"),
            Self::UnsupportedFormat { expected, actual } => {
                write!(f, "Unsupported format {actual:?}; expected {expected}")
            }
            Self::UnsupportedDigest { expected, actual } => {
                write!(f, "Unsupported digest {actual:?}; expected {expected}")
            }
            Self::InvalidDigest(error) => write!(f, "Invalid content digest: {error}"),
            Self::InvalidValue { field, value } => write!(f, "Invalid {field}: {value}"),
            Self::DuplicateTerrain(coord) => write!(f, "Duplicate terrain position {coord:?}"),
            Self::DuplicateHeight(coord) => write!(f, "Duplicate height vertex {coord:?}"),
            Self::DuplicateObject(placement) => {
                write!(f, "Duplicate object occurrence {placement:?}")
            }
            Self::DigestMismatch { expected, actual } => write!(
                f,
                "Content digest mismatch: expected {expected}, decoded {actual}"
            ),
        }
    }
}
impl std::error::Error for SemanticTransportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Record { error, .. } => Some(error.as_ref()),
            Self::InvalidDigest(error) => Some(error),
            _ => None,
        }
    }
}
fn record_error(
    collection: &'static str,
    index: usize,
    error: SemanticTransportError,
) -> SemanticTransportError {
    SemanticTransportError::Record {
        collection,
        index,
        error: Box::new(error),
    }
}
impl From<serde_json::Error> for SemanticTransportError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error.to_string())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Header {
    format: String,
    digest_algorithm: String,
    digest: String,
}
impl Header {
    fn new(format: &str, algorithm: &str, digest: Digest256) -> Self {
        Self {
            format: format.into(),
            digest_algorithm: algorithm.into(),
            digest: digest.to_string(),
        }
    }
    fn validate(
        &self,
        format: &'static str,
        algorithm: &'static str,
    ) -> Result<Digest256, SemanticTransportError> {
        if self.format != format {
            return Err(SemanticTransportError::UnsupportedFormat {
                expected: format,
                actual: self.format.clone(),
            });
        }
        if self.digest_algorithm != algorithm {
            return Err(SemanticTransportError::UnsupportedDigest {
                expected: algorithm,
                actual: self.digest_algorithm.clone(),
            });
        }
        Digest256::from_lower_hex(&self.digest).map_err(SemanticTransportError::InvalidDigest)
    }
}
fn verify(expected: Digest256, actual: Digest256) -> Result<(), SemanticTransportError> {
    if expected != actual {
        return Err(SemanticTransportError::DigestMismatch { expected, actual });
    }
    Ok(())
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    header: Header,
    terrain: Vec<TerrainRecord>,
    heights: Vec<HeightRecord>,
    objects: Vec<ObjectRecord>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Patch {
    header: Header,
    operations: Vec<OperationRecord>,
}
impl MapDocument {
    /// Encode complete canonical map content, with its v1 digest, as reproducible JSON.
    ///
    /// This is an explicit infrastructure snapshot, not a project save or a whole-workspace
    /// serialization. Source/profile/build, availability, scenarios and appearance are excluded.
    /// No history/revision/dirty state is read or changed. JSON layout is not hashed.
    pub fn to_snapshot_json_v1(&self) -> Result<String, SemanticTransportError> {
        let snapshot = Snapshot {
            header: Header::new(
                MAP_SNAPSHOT_FORMAT_V1,
                crate::MAP_DOCUMENT_DIGEST_ALGORITHM_V1,
                self.content_digest_v1(),
            ),
            terrain: self.terrain_entries().map(TerrainRecord::from).collect(),
            heights: self.height_entries().map(HeightRecord::from).collect(),
            objects: self
                .object_entries()
                .into_iter()
                .map(ObjectRecord::from)
                .collect(),
        };
        Ok(serde_json::to_string_pretty(&snapshot)?)
    }
    /// Decode a complete map, rejecting invalid/duplicate records, unsupported data and wrong hashes.
    ///
    /// Every value is validated before constructing a new canonical map. Sparse default terrain
    /// normalizes normally; duplicate positions/occurrences are rejected even when values agree.
    /// The returned map is infrastructure input, never permission to replace a live edit session.
    pub fn from_snapshot_json_v1(text: &str) -> Result<Self, SemanticTransportError> {
        let snapshot: Snapshot = serde_json::from_str(text)?;
        let expected = snapshot.header.validate(
            MAP_SNAPSHOT_FORMAT_V1,
            crate::MAP_DOCUMENT_DIGEST_ALGORITHM_V1,
        )?;
        let mut coordinates = BTreeSet::new();
        let mut terrain = Vec::with_capacity(snapshot.terrain.len());
        for (index, record) in snapshot.terrain.into_iter().enumerate() {
            let (coord, tile) = record
                .decode()
                .map_err(|error| record_error("terrain", index, error))?;
            if !coordinates.insert(coord) {
                return Err(SemanticTransportError::DuplicateTerrain(coord));
            }
            terrain.push((coord, tile));
        }
        let mut vertices = BTreeSet::new();
        let mut heights = Vec::with_capacity(snapshot.heights.len());
        for (index, record) in snapshot.heights.into_iter().enumerate() {
            let (coord, value) = record
                .decode()
                .map_err(|error| record_error("heights", index, error))?;
            if !vertices.insert(coord) {
                return Err(SemanticTransportError::DuplicateHeight(coord));
            }
            heights.push((coord, value));
        }
        let mut occurrences = HashSet::new();
        let mut objects = Vec::with_capacity(snapshot.objects.len());
        for (index, record) in snapshot.objects.into_iter().enumerate() {
            let placement = record
                .decode()
                .map_err(|error| record_error("objects", index, error))?;
            if !occurrences.insert(placement) {
                return Err(SemanticTransportError::DuplicateObject(placement));
            }
            objects.push(placement);
        }
        let document = Self::from_parts(terrain, heights, objects);
        verify(expected, document.content_digest_v1())?;
        Ok(document)
    }
}
impl MapPatch {
    /// Encode the exact ordered map patch and digest. Impact is derived, never persisted.
    /// Feature/intent/source context belongs to a reproduction envelope. This does not execute it.
    pub fn to_patch_json_v1(&self) -> Result<String, SemanticTransportError> {
        Ok(serde_json::to_string_pretty(&Patch {
            header: Header::new(
                MAP_PATCH_FORMAT_V1,
                crate::MAP_PATCH_DIGEST_ALGORITHM_V1,
                self.content_digest_v1(),
            ),
            operations: self
                .ops()
                .iter()
                .copied()
                .map(OperationRecord::from)
                .collect(),
        })?)
    }
    /// Decode exact operation order/no-ops/before-state, recompute impact and check the v1 digest.
    ///
    /// This validates value domains and integrity, not applicability to a particular document.
    /// Live application still requires the normal edit gateway and semantic validation policy.
    pub fn from_patch_json_v1(text: &str) -> Result<Self, SemanticTransportError> {
        let record: Patch = serde_json::from_str(text)?;
        let expected = record
            .header
            .validate(MAP_PATCH_FORMAT_V1, crate::MAP_PATCH_DIGEST_ALGORITHM_V1)?;
        let patch = Self::new(
            record
                .operations
                .into_iter()
                .enumerate()
                .map(|(index, record)| {
                    record
                        .decode()
                        .map_err(|error| record_error("operations", index, error))
                })
                .collect::<Result<Vec<_>, _>>()?,
        );
        verify(expected, patch.content_digest_v1())?;
        Ok(patch)
    }
}
#[cfg(test)]
mod tests;
