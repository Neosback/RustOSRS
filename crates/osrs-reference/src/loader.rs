//! Offline fixture loading with path containment and expected-output integrity checks.

use crate::comparator::{ExactMismatch, compare_exact};
use crate::fixture::{FixtureManifest, ManifestError};
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

/// Checked-in fixture repository rooted at `reference-fixtures/` or an equivalent test root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureRepository {
    root: PathBuf,
}

impl FixtureRepository {
    /// Open an existing fixture root and canonicalize it once for containment checks.
    pub fn new(root: impl AsRef<Path>) -> Result<Self, FixtureLoadError> {
        let requested = root.as_ref();
        let canonical = fs::canonicalize(requested).map_err(|error| FixtureLoadError::Io {
            operation: "canonicalize fixture root",
            path: requested.to_path_buf(),
            detail: error.to_string(),
        })?;
        Ok(Self { root: canonical })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Load one manifest plus its normalized input and expected-output bytes.
    ///
    /// All paths are fixture-root relative, may not contain parent/root/prefix
    /// components, and are canonicalized before use so a symlink cannot escape
    /// the fixture root. Expected bytes are rejected unless their SHA-256 exactly
    /// matches the manifest identity.
    pub fn load(
        &self,
        manifest_relative_path: impl AsRef<Path>,
    ) -> Result<LoadedFixture, FixtureLoadError> {
        let manifest_path = self.resolve_existing("manifest", manifest_relative_path.as_ref())?;
        let manifest_text = fs::read_to_string(&manifest_path).map_err(|error| FixtureLoadError::Io {
            operation: "read fixture manifest",
            path: manifest_path.clone(),
            detail: error.to_string(),
        })?;
        let manifest = FixtureManifest::parse_yaml(&manifest_text).map_err(FixtureLoadError::Manifest)?;

        let input_path = self.resolve_existing("input", Path::new(&manifest.input))?;
        let expected_path = self.resolve_existing("expected", Path::new(&manifest.expected))?;
        let input = fs::read(&input_path).map_err(|error| FixtureLoadError::Io {
            operation: "read fixture input",
            path: input_path.clone(),
            detail: error.to_string(),
        })?;
        let expected = fs::read(&expected_path).map_err(|error| FixtureLoadError::Io {
            operation: "read fixture expected output",
            path: expected_path.clone(),
            detail: error.to_string(),
        })?;

        let actual_sha256 = sha256_hex(&expected);
        if !actual_sha256.eq_ignore_ascii_case(&manifest.expected_sha256) {
            return Err(FixtureLoadError::ExpectedHashMismatch {
                path: expected_path,
                declared: manifest.expected_sha256.clone(),
                actual: actual_sha256,
            });
        }

        Ok(LoadedFixture {
            manifest_path,
            input_path,
            expected_path,
            manifest,
            input,
            expected,
        })
    }

    fn resolve_existing(
        &self,
        field: &'static str,
        relative: &Path,
    ) -> Result<PathBuf, FixtureLoadError> {
        validate_relative_path(field, relative)?;
        let candidate = self.root.join(relative);
        let canonical = fs::canonicalize(&candidate).map_err(|error| FixtureLoadError::Io {
            operation: "canonicalize fixture path",
            path: candidate,
            detail: error.to_string(),
        })?;
        if !canonical.starts_with(&self.root) {
            return Err(FixtureLoadError::OutsideRoot {
                field,
                path: relative.to_path_buf(),
            });
        }
        Ok(canonical)
    }
}

/// Fully verified fixture payload ready for an offline Rust differential test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedFixture {
    pub manifest_path: PathBuf,
    pub input_path: PathBuf,
    pub expected_path: PathBuf,
    pub manifest: FixtureManifest,
    pub input: Vec<u8>,
    pub expected: Vec<u8>,
}

impl LoadedFixture {
    /// Exact comparison against this fixture's already hash-verified expected bytes.
    pub fn compare_actual(&self, actual: &[u8]) -> Result<(), ExactMismatch> {
        compare_exact(&self.expected, actual)
    }
}

fn validate_relative_path(field: &'static str, path: &Path) -> Result<(), FixtureLoadError> {
    if path.as_os_str().is_empty() {
        return Err(FixtureLoadError::UnsafePath {
            field,
            path: path.to_path_buf(),
        });
    }
    for component in path.components() {
        if matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        ) {
            return Err(FixtureLoadError::UnsafePath {
                field,
                path: path.to_path_buf(),
            });
        }
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixtureLoadError {
    Io {
        operation: &'static str,
        path: PathBuf,
        detail: String,
    },
    Manifest(ManifestError),
    UnsafePath {
        field: &'static str,
        path: PathBuf,
    },
    OutsideRoot {
        field: &'static str,
        path: PathBuf,
    },
    ExpectedHashMismatch {
        path: PathBuf,
        declared: String,
        actual: String,
    },
}

impl fmt::Display for FixtureLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                operation,
                path,
                detail,
            } => write!(formatter, "{operation} `{}` failed: {detail}", path.display()),
            Self::Manifest(error) => error.fmt(formatter),
            Self::UnsafePath { field, path } => write!(
                formatter,
                "fixture {field} path `{}` must be non-empty and root-relative without parent traversal",
                path.display()
            ),
            Self::OutsideRoot { field, path } => write!(
                formatter,
                "fixture {field} path `{}` resolves outside the fixture root",
                path.display()
            ),
            Self::ExpectedHashMismatch {
                path,
                declared,
                actual,
            } => write!(
                formatter,
                "fixture expected output `{}` SHA-256 mismatch: manifest={declared}, actual={actual}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for FixtureLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Manifest(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FixtureLoadError, FixtureRepository, sha256_hex};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let root = std::env::temp_dir().join(format!(
            "rustosrs-m5-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("manifest"))?;
        fs::create_dir_all(root.join("model"))?;
        Ok(root)
    }

    fn write_fixture(root: &PathBuf, declared_hash: &str) -> Result<(), Box<dyn std::error::Error>> {
        fs::write(root.join("model/input.json"), b"{\"orientation\":4}\n")?;
        fs::write(root.join("model/expected.json"), b"{\"mirror\":true}\n")?;
        let manifest = format!(
            r#"fixture_id: model.mirror.typed.rotated_true.orientation_4
schema_version: 1
owned_specs:
  - MODEL-BUILD-002
parity_level: P1
oracle:
  kind: melxin-deob
  repository: melxin/runelite
  commit: 1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc
  files:
    - path: runescape-client/src/main/java/ObjectComposition.java
      blob: 079451cd9a6dcfd2666efd15b0524250eaafe4c4
      symbol: getModelData
harness:
  path: tools/deob-harness
  revision: m5-loader-test
input: model/input.json
expected: model/expected.json
expected_sha256: {declared_hash}
normalization:
  - preserve integer order
"#
        );
        fs::write(root.join("manifest/model-mirror.yaml"), manifest)?;
        Ok(())
    }

    #[test]
    fn loads_hash_verified_fixture_and_compares_exactly() -> Result<(), Box<dyn std::error::Error>> {
        let root = temp_root("load")?;
        let expected = b"{\"mirror\":true}\n";
        write_fixture(&root, &sha256_hex(expected))?;

        let repository = FixtureRepository::new(&root)?;
        let fixture = repository.load("manifest/model-mirror.yaml")?;
        assert_eq!(fixture.input, b"{\"orientation\":4}\n");
        assert_eq!(fixture.expected, expected);
        assert!(fixture.compare_actual(expected).is_ok());
        assert!(fixture.compare_actual(b"{\"mirror\":false}\n").is_err());

        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn rejects_expected_output_hash_drift() -> Result<(), Box<dyn std::error::Error>> {
        let root = temp_root("hash")?;
        write_fixture(&root, &"0".repeat(64))?;
        let repository = FixtureRepository::new(&root)?;
        assert!(matches!(
            repository.load("manifest/model-mirror.yaml"),
            Err(FixtureLoadError::ExpectedHashMismatch { .. })
        ));
        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn rejects_parent_traversal_before_file_access() -> Result<(), Box<dyn std::error::Error>> {
        let root = temp_root("path")?;
        let repository = FixtureRepository::new(&root)?;
        assert!(matches!(
            repository.load("../outside.yaml"),
            Err(FixtureLoadError::UnsafePath {
                field: "manifest",
                ..
            })
        ));
        fs::remove_dir_all(root)?;
        Ok(())
    }
}
