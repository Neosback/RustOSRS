//! Versioned reference-fixture manifest schema and validation.

use serde::Deserialize;
use std::{collections::HashSet, error::Error, fmt};

/// Fixture-manifest schema version currently understood by RustOSRS.
pub const SUPPORTED_SCHEMA_VERSION: u32 = 1;

/// Canonical parity layer owned by a source-derived fixture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum ParityLevel {
    P0,
    P1,
    P2,
    P3,
    P4,
}

/// Source file contributing to the reference oracle for a fixture.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct OracleFile {
    pub path: String,
    pub blob: String,
    #[serde(default)]
    pub symbol: Option<String>,
}

/// Exact source/oracle provenance for a fixture.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct OracleManifest {
    pub kind: String,
    #[serde(default)]
    pub repository: Option<String>,
    #[serde(default)]
    pub commit: Option<String>,
    #[serde(default)]
    pub files: Vec<OracleFile>,
}

/// Harness provenance used to generate or normalize fixture output.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct HarnessManifest {
    pub path: String,
    pub revision: String,
}

/// Versioned manifest for one canonical reference fixture.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FixtureManifest {
    pub fixture_id: String,
    pub schema_version: u32,
    pub owned_specs: Vec<String>,
    pub parity_level: ParityLevel,
    pub oracle: OracleManifest,
    pub harness: HarnessManifest,
    pub input: String,
    pub expected: String,
    pub expected_sha256: String,
    #[serde(default)]
    pub normalization: Vec<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

impl FixtureManifest {
    /// Parse and validate a YAML fixture manifest.
    pub fn parse_yaml(input: &str) -> Result<Self, ManifestError> {
        let manifest: Self = serde_yaml_ng::from_str(input)
            .map_err(|error| ManifestError::Syntax(error.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Validate schema and provenance invariants that serde alone cannot express.
    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.schema_version != SUPPORTED_SCHEMA_VERSION {
            return Err(ManifestError::UnsupportedSchemaVersion(self.schema_version));
        }
        require_non_empty(&self.fixture_id, "fixture_id")?;
        if self.owned_specs.is_empty() {
            return Err(ManifestError::MissingOwnedSpecs);
        }
        let mut owned_specs = HashSet::new();
        for spec in &self.owned_specs {
            require_non_empty(spec, "owned_specs[]")?;
            if !owned_specs.insert(spec.as_str()) {
                return Err(ManifestError::DuplicateOwnedSpec(spec.clone()));
            }
        }

        require_non_empty(&self.oracle.kind, "oracle.kind")?;
        if self.oracle.repository.is_some() != self.oracle.commit.is_some() {
            return Err(ManifestError::OracleRepositoryCommitPair);
        }
        if let Some(repository) = &self.oracle.repository {
            require_non_empty(repository, "oracle.repository")?;
        }
        if let Some(commit) = &self.oracle.commit {
            validate_hex_id(commit, 40, "oracle.commit")?;
        }
        if self.oracle.commit.is_none() && self.oracle.files.is_empty() {
            return Err(ManifestError::MissingOracleProvenance);
        }
        let mut oracle_paths = HashSet::new();
        for file in &self.oracle.files {
            require_non_empty(&file.path, "oracle.files[].path")?;
            validate_hex_id(&file.blob, 40, "oracle.files[].blob")?;
            if !oracle_paths.insert(file.path.as_str()) {
                return Err(ManifestError::DuplicateOracleFile(file.path.clone()));
            }
            if let Some(symbol) = &file.symbol {
                require_non_empty(symbol, "oracle.files[].symbol")?;
            }
        }

        require_non_empty(&self.harness.path, "harness.path")?;
        require_non_empty(&self.harness.revision, "harness.revision")?;
        require_non_empty(&self.input, "input")?;
        require_non_empty(&self.expected, "expected")?;
        validate_hex_id(&self.expected_sha256, 64, "expected_sha256")
            .map_err(|_| ManifestError::InvalidExpectedSha256)?;
        Ok(())
    }
}

fn require_non_empty(value: &str, field: &'static str) -> Result<(), ManifestError> {
    if value.trim().is_empty() {
        Err(ManifestError::EmptyField(field))
    } else {
        Ok(())
    }
}

fn validate_hex_id(
    value: &str,
    expected_len: usize,
    field: &'static str,
) -> Result<(), ManifestError> {
    if value.len() != expected_len || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ManifestError::InvalidHexId {
            field,
            expected_len,
        });
    }
    Ok(())
}

/// Fixture-manifest parsing or schema-validation failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    Syntax(String),
    UnsupportedSchemaVersion(u32),
    EmptyField(&'static str),
    MissingOwnedSpecs,
    DuplicateOwnedSpec(String),
    OracleRepositoryCommitPair,
    MissingOracleProvenance,
    DuplicateOracleFile(String),
    InvalidHexId {
        field: &'static str,
        expected_len: usize,
    },
    InvalidExpectedSha256,
}

impl fmt::Display for ManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax(message) => write!(formatter, "invalid fixture YAML: {message}"),
            Self::UnsupportedSchemaVersion(version) => {
                write!(formatter, "unsupported fixture schema version {version}")
            }
            Self::EmptyField(field) => {
                write!(formatter, "fixture field `{field}` must not be empty")
            }
            Self::MissingOwnedSpecs => {
                write!(formatter, "fixture must own at least one canonical spec")
            }
            Self::DuplicateOwnedSpec(spec) => {
                write!(formatter, "fixture lists canonical spec `{spec}` more than once")
            }
            Self::OracleRepositoryCommitPair => formatter.write_str(
                "oracle.repository and oracle.commit must either both be present or both be absent",
            ),
            Self::MissingOracleProvenance => formatter.write_str(
                "fixture oracle must provide an exact commit or at least one blob-pinned source file",
            ),
            Self::DuplicateOracleFile(path) => {
                write!(formatter, "fixture oracle lists source file `{path}` more than once")
            }
            Self::InvalidHexId {
                field,
                expected_len,
            } => write!(
                formatter,
                "fixture field `{field}` must be exactly {expected_len} hexadecimal characters"
            ),
            Self::InvalidExpectedSha256 => formatter.write_str(
                "expected_sha256 must be exactly 64 hexadecimal characters",
            ),
        }
    }
}

impl Error for ManifestError {}

#[cfg(test)]
mod tests {
    use super::{FixtureManifest, ManifestError, ParityLevel};

    const VALID_MANIFEST: &str = r#"
fixture_id: normals.merge.coincident_triangle.hide_false
schema_version: 1
owned_specs:
  - NORMALS-002
parity_level: P1
oracle:
  kind: melxin-deob
  repository: melxin/runelite
  commit: 1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc
  files:
    - path: runescape-client/src/main/java/ModelData.java
      blob: 2cc9406b2504fbd4fae0c0c952aa2d133809e928
      symbol: method5262
harness:
  path: tools/deob-harness
  revision: fixture-schema-test
input: normals/merge_coincident_triangle.input.json
expected: normals/merge_coincident_triangle.expected.json
expected_sha256: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
normalization:
  - preserve integer order
notes: parser smoke test
"#;

    #[test]
    fn parses_valid_manifest() {
        let result = FixtureManifest::parse_yaml(VALID_MANIFEST);
        let Ok(manifest) = result else {
            panic!("valid fixture manifest failed to parse");
        };

        assert_eq!(manifest.schema_version, 1);
        assert_eq!(manifest.parity_level, ParityLevel::P1);
        assert_eq!(manifest.owned_specs, ["NORMALS-002"]);
    }

    #[test]
    fn rejects_unsupported_schema_version() {
        let input = VALID_MANIFEST.replace("schema_version: 1", "schema_version: 2");
        let result = FixtureManifest::parse_yaml(&input);

        assert!(matches!(
            result,
            Err(ManifestError::UnsupportedSchemaVersion(2))
        ));
    }

    #[test]
    fn rejects_empty_owned_specs() {
        let input = VALID_MANIFEST.replace("owned_specs:\n  - NORMALS-002", "owned_specs: []");
        let result = FixtureManifest::parse_yaml(&input);

        assert!(matches!(result, Err(ManifestError::MissingOwnedSpecs)));
    }

    #[test]
    fn rejects_missing_exact_oracle_provenance() {
        let input = VALID_MANIFEST
            .replace("  repository: melxin/runelite\n", "")
            .replace(
                "  commit: 1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc\n",
                "",
            )
            .replace(
                "  files:\n    - path: runescape-client/src/main/java/ModelData.java\n      blob: 2cc9406b2504fbd4fae0c0c952aa2d133809e928\n      symbol: method5262\n",
                "  files: []\n",
            );
        assert!(matches!(
            FixtureManifest::parse_yaml(&input),
            Err(ManifestError::MissingOracleProvenance)
        ));
    }

    #[test]
    fn rejects_unpaired_repository_and_commit() {
        let input = VALID_MANIFEST.replace("  repository: melxin/runelite\n", "");
        assert!(matches!(
            FixtureManifest::parse_yaml(&input),
            Err(ManifestError::OracleRepositoryCommitPair)
        ));
    }

    #[test]
    fn rejects_invalid_blob_identity() {
        let input =
            VALID_MANIFEST.replace("2cc9406b2504fbd4fae0c0c952aa2d133809e928", "not-a-blob");
        assert!(matches!(
            FixtureManifest::parse_yaml(&input),
            Err(ManifestError::InvalidHexId {
                field: "oracle.files[].blob",
                ..
            })
        ));
    }

    #[test]
    fn rejects_malformed_yaml() {
        let result = FixtureManifest::parse_yaml("fixture_id: [");

        assert!(matches!(result, Err(ManifestError::Syntax(_))));
    }
}
