//! Canonical normalized semantic fixture schemas.
//!
//! M5 fixtures use small typed JSON documents so exact semantic values can be
//! compared without importing cache, renderer, editor, or oracle runtime types.
//! The repository artifacts use JSON syntax; deserialization is intentionally
//! kept inside this development-only crate.

use serde::{Deserialize, Serialize};
use std::{error::Error, fmt, str};

/// Normalized input/output schema version currently understood by RustOSRS.
pub const NORMALIZED_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NormalizedFixtureKind {
    ModelSelection,
    ModelMirror,
    ModelTransform,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizedFixtureInput {
    pub schema_version: u32,
    pub case: NormalizedInputCase,
}

impl NormalizedFixtureInput {
    pub fn parse(bytes: &[u8]) -> Result<Self, NormalizedSchemaError> {
        let text = str::from_utf8(bytes)
            .map_err(|error| NormalizedSchemaError::Utf8(error.to_string()))?;
        let document: Self = serde_yaml_ng::from_str(text)
            .map_err(|error| NormalizedSchemaError::Syntax(error.to_string()))?;
        validate_version(document.schema_version)?;
        Ok(document)
    }

    pub const fn kind(&self) -> NormalizedFixtureKind {
        self.case.kind()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizedFixtureExpected {
    pub schema_version: u32,
    pub case: NormalizedExpectedCase,
}

impl NormalizedFixtureExpected {
    pub fn parse(bytes: &[u8]) -> Result<Self, NormalizedSchemaError> {
        let text = str::from_utf8(bytes)
            .map_err(|error| NormalizedSchemaError::Utf8(error.to_string()))?;
        let document: Self = serde_yaml_ng::from_str(text)
            .map_err(|error| NormalizedSchemaError::Syntax(error.to_string()))?;
        validate_version(document.schema_version)?;
        Ok(document)
    }

    pub const fn kind(&self) -> NormalizedFixtureKind {
        self.case.kind()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum NormalizedInputCase {
    ModelSelection {
        models: NormalizedObjectModels,
        is_rotated: bool,
        requested_type: u8,
        orientation: u8,
    },
    ModelMirror {
        vertices: Vec<NormalizedModelPoint>,
        faces: Vec<NormalizedTriangle>,
    },
    ModelTransform {
        vertices: Vec<NormalizedModelPoint>,
        face_colors: Vec<u16>,
        face_textures: Vec<Option<u32>>,
        requested_type: u8,
        orientation: u8,
        recolors: Vec<NormalizedU16Replacement>,
        retextures: Vec<NormalizedU16Replacement>,
        scale: NormalizedModelScale,
        translation: NormalizedModelTranslation,
    },
}

impl NormalizedInputCase {
    pub const fn kind(&self) -> NormalizedFixtureKind {
        match self {
            Self::ModelSelection { .. } => NormalizedFixtureKind::ModelSelection,
            Self::ModelMirror { .. } => NormalizedFixtureKind::ModelMirror,
            Self::ModelTransform { .. } => NormalizedFixtureKind::ModelTransform,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum NormalizedExpectedCase {
    ModelSelection {
        selection: Option<NormalizedModelSelection>,
    },
    ModelMirror {
        vertices: Vec<NormalizedModelPoint>,
        faces: Vec<NormalizedTriangle>,
    },
    ModelTransform {
        vertices: Vec<NormalizedModelPoint>,
        face_colors: Vec<u16>,
        face_textures: Vec<Option<u32>>,
    },
}

impl NormalizedExpectedCase {
    pub const fn kind(&self) -> NormalizedFixtureKind {
        match self {
            Self::ModelSelection { .. } => NormalizedFixtureKind::ModelSelection,
            Self::ModelMirror { .. } => NormalizedFixtureKind::ModelMirror,
            Self::ModelTransform { .. } => NormalizedFixtureKind::ModelTransform,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "encoding", rename_all = "snake_case", deny_unknown_fields)]
pub enum NormalizedObjectModels {
    Typed { entries: Vec<NormalizedTypedModel> },
    Untyped { model_ids: Vec<u32> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizedTypedModel {
    pub loc_type: u8,
    pub model_id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizedModelSelection {
    pub model_ids: Vec<u32>,
    pub mirror: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizedModelPoint {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizedTriangle {
    pub a: u32,
    pub b: u32,
    pub c: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizedU16Replacement {
    pub from: u16,
    pub to: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizedModelScale {
    pub x: u16,
    pub y: u16,
    pub z: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizedModelTranslation {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

fn validate_version(version: u32) -> Result<(), NormalizedSchemaError> {
    if version == NORMALIZED_SCHEMA_VERSION {
        Ok(())
    } else {
        Err(NormalizedSchemaError::UnsupportedSchemaVersion(version))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NormalizedSchemaError {
    Utf8(String),
    Syntax(String),
    UnsupportedSchemaVersion(u32),
}

impl fmt::Display for NormalizedSchemaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Utf8(detail) => write!(formatter, "normalized fixture is not UTF-8: {detail}"),
            Self::Syntax(detail) => {
                write!(formatter, "invalid normalized fixture document: {detail}")
            }
            Self::UnsupportedSchemaVersion(version) => {
                write!(
                    formatter,
                    "unsupported normalized fixture schema version {version}"
                )
            }
        }
    }
}

impl Error for NormalizedSchemaError {}

#[cfg(test)]
mod tests {
    use super::{NormalizedFixtureInput, NormalizedFixtureKind, NormalizedSchemaError};

    #[test]
    fn parses_typed_model_selection_document() -> Result<(), Box<dyn std::error::Error>> {
        let document = NormalizedFixtureInput::parse(
            br#"{
  "schema_version": 1,
  "case": {
    "kind": "model_selection",
    "models": {
      "encoding": "typed",
      "entries": [{"loc_type": 4, "model_id": 200}]
    },
    "is_rotated": false,
    "requested_type": 4,
    "orientation": 4
  }
}"#,
        )?;
        assert_eq!(document.kind(), NormalizedFixtureKind::ModelSelection);
        Ok(())
    }

    #[test]
    fn rejects_unknown_schema_version() {
        let result = NormalizedFixtureInput::parse(
            br#"{
  "schema_version": 2,
  "case": {
    "kind": "model_mirror",
    "vertices": [],
    "faces": []
  }
}"#,
        );
        assert!(matches!(
            result,
            Err(NormalizedSchemaError::UnsupportedSchemaVersion(2))
        ));
    }

    #[test]
    fn rejects_unknown_fields() {
        let result = NormalizedFixtureInput::parse(
            br#"{
  "schema_version": 1,
  "extra": true,
  "case": {
    "kind": "model_mirror",
    "vertices": [],
    "faces": []
  }
}"#,
        );
        assert!(matches!(result, Err(NormalizedSchemaError::Syntax(_))));
    }
}
