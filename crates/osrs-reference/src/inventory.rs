//! Deterministic repository-wide discovery of checked-in fixture manifests.

use crate::loader::{FixtureLoadError, FixtureRepository, LoadedFixture};
use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

/// Deterministically discovered and integrity-verified fixture set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureInventory {
    fixtures: Vec<LoadedFixture>,
}

impl FixtureInventory {
    /// Discover every `.yaml` manifest below `reference-fixtures/manifest`,
    /// sort by root-relative path, load/hash-verify each fixture, and reject
    /// duplicate fixture IDs.
    pub fn discover(repository: &FixtureRepository) -> Result<Self, FixtureInventoryError> {
        let manifest_root = repository.root().join("manifest");
        let mut manifest_paths = Vec::new();
        collect_manifest_paths(&manifest_root, &mut manifest_paths)?;
        manifest_paths.sort();

        if manifest_paths.is_empty() {
            return Err(FixtureInventoryError::EmptyInventory);
        }

        let mut fixture_ids = HashSet::new();
        let mut fixtures = Vec::with_capacity(manifest_paths.len());
        for manifest_path in manifest_paths {
            let relative = manifest_path
                .strip_prefix(repository.root())
                .map_err(|_| FixtureInventoryError::OutsideFixtureRoot(manifest_path.clone()))?;
            let fixture = repository
                .load(relative)
                .map_err(FixtureInventoryError::Load)?;
            if !fixture_ids.insert(fixture.manifest.fixture_id.clone()) {
                return Err(FixtureInventoryError::DuplicateFixtureId(
                    fixture.manifest.fixture_id,
                ));
            }
            fixtures.push(fixture);
        }

        Ok(Self { fixtures })
    }

    pub fn fixtures(&self) -> &[LoadedFixture] {
        &self.fixtures
    }

    pub fn len(&self) -> usize {
        self.fixtures.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fixtures.is_empty()
    }
}

fn collect_manifest_paths(
    directory: &Path,
    output: &mut Vec<PathBuf>,
) -> Result<(), FixtureInventoryError> {
    let entries = fs::read_dir(directory).map_err(|error| FixtureInventoryError::Io {
        operation: "read fixture manifest directory",
        path: directory.to_path_buf(),
        detail: error.to_string(),
    })?;

    for entry in entries {
        let entry = entry.map_err(|error| FixtureInventoryError::Io {
            operation: "read fixture manifest directory entry",
            path: directory.to_path_buf(),
            detail: error.to_string(),
        })?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| FixtureInventoryError::Io {
                operation: "read fixture manifest entry type",
                path: path.clone(),
                detail: error.to_string(),
            })?;

        if file_type.is_symlink() {
            return Err(FixtureInventoryError::SymlinkEntry(path));
        }
        if file_type.is_dir() {
            collect_manifest_paths(&path, output)?;
        } else if file_type.is_file()
            && path
                .extension()
                .is_some_and(|extension| extension == "yaml")
        {
            output.push(path);
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixtureInventoryError {
    Io {
        operation: &'static str,
        path: PathBuf,
        detail: String,
    },
    SymlinkEntry(PathBuf),
    OutsideFixtureRoot(PathBuf),
    Load(FixtureLoadError),
    DuplicateFixtureId(String),
    EmptyInventory,
}

impl fmt::Display for FixtureInventoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                operation,
                path,
                detail,
            } => write!(
                formatter,
                "{operation} `{}` failed: {detail}",
                path.display()
            ),
            Self::SymlinkEntry(path) => write!(
                formatter,
                "fixture manifest tree may not contain symlink entry `{}`",
                path.display()
            ),
            Self::OutsideFixtureRoot(path) => write!(
                formatter,
                "discovered fixture manifest `{}` is outside the configured fixture root",
                path.display()
            ),
            Self::Load(error) => error.fmt(formatter),
            Self::DuplicateFixtureId(fixture_id) => write!(
                formatter,
                "fixture inventory contains duplicate fixture ID `{fixture_id}`"
            ),
            Self::EmptyInventory => {
                formatter.write_str("fixture inventory contains no YAML manifests")
            }
        }
    }
}

impl std::error::Error for FixtureInventoryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Load(error) => Some(error),
            _ => None,
        }
    }
}
