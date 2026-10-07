//! Versioned reference-fixture manifest schema and validation.

use serde::Deserialize;
use std::{error::Error, fmt};

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

    /// Validate schema-level invariants that serde alone cannot express.
    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.schema_version != SUPPORTED_SCHEMA_VERSION {
            return Err(ManifestError::UnsupportedSchemaVersion(self.schema_version));
        }
        if self.fixture_id.trim().is_empty() {
            return Err(ManifestError::EmptyField("fixture_id"));
        }
        if self.owned_specs.is_empty() {
            return Err(ManifestError::MissingOwnedSpecs);
        }
        if self.oracle.kind.trim().is_empty() {
            return Err(ManifestError::EmptyField("oracle.kind"));
        }
        if self.harness.path.trim().is_empty() {
            return Err(ManifestError::EmptyField("harness.path"));
        }
        if self.harness.revision.trim().is_empty() {
            return Err(ManifestError::EmptyField("harness.revision"));
        }
        if self.input.trim().is_empty() {
            return Err(ManifestError::EmptyField("input"));
        }
        if self.expected.trim().is_empty() {
            return Err(ManifestError::EmptyField("expected"));
        }
        if self.expected_sha256.len() != 64
            || !self.expected_sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(ManifestError::InvalidExpectedSha256);
        }
        Ok(())
    }
}

/// Fixture-manifest parsing or schema-validation failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    Syntax(String),
    UnsupportedSchemaVersion(u32),
    EmptyField(&'static str),
    MissingOwnedSpecs,
    InvalidExpectedSha256,
}

impl fmt::Display for ManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax(message) => write!(formatter, "invalid fixture YAML: {message}"),
            Self::UnsupportedSchemaVersion(version) => {
                write!(formatter, "unsupported fixture schema version {version}")
            }
            Self::EmptyField(field) => write!(formatter, "fixture field `{field}` must not be empty"),
            Self::MissingOwnedSpecs => write!(formatter, "fixture must own at least one canonical spec"),
            Self::InvalidExpectedSha256 => {
                write!(formatter, "expected_sha256 must be exactly 64 hexadecimal characters")
            }
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
    fn rejects_malformed_yaml() {
        let result = FixtureManifest::parse_yaml("fixture_id: [");

        assert!(matches!(result, Err(ManifestError::Syntax(_))));
    }
}
