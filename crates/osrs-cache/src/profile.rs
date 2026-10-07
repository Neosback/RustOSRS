//! Target-cache profile parsing and validation.
//!
//! A target profile identifies the reproducible cache snapshot and the
//! revision-sensitive decode contract. It is cache input metadata, not an OSRS
//! semantic definition and not editor configuration.

use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub const TARGET_PROFILE_SCHEMA_V1: &str = "rustosrs-target-profile/v1";
pub const TARGET_PROFILE_DIGEST_V1: &str = "rustosrs-target-profile-digest-v1";
pub const CACHE_FINGERPRINT_V1: &str = "rustosrs-cache-v1";

#[derive(Debug, Clone, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TargetProfile {
    pub schema: String,
    pub id: String,
    pub game_family: String,
    pub profile_schema_version: u32,
    pub decoder_schema_version: u32,
    pub cache_source: CacheSource,
    pub cache_fingerprint: CacheFingerprint,
    #[serde(default)]
    pub transport_evidence: Option<TransportEvidence>,
    pub xtea: XteaProfile,
    pub semantic_sources: BTreeMap<String, SemanticSource>,
    pub revision_gates: BTreeMap<String, RevisionGate>,
    pub policy: BTreeMap<String, bool>,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CacheSource {
    pub provider: String,
    pub cache_id: u64,
    pub uri: String,
    pub game: String,
    pub environment: String,
    pub language: String,
    pub build: u32,
    pub timestamp: String,
    pub format: String,
    pub logical_archive_slots_present: u32,
    pub logical_archive_slots_total: u32,
    #[serde(default)]
    pub empty_logical_indices: Vec<u32>,
    pub physical_content_indices: u32,
    pub groups_present: u64,
    pub groups_total: u64,
    pub reported_size_mib: u64,
    pub xtea_keys_present: u64,
    pub xtea_keys_total: u64,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CacheFingerprint {
    pub algorithm: String,
    pub value: Option<String>,
    pub state: String,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TransportEvidence {
    pub dependency_candidate: String,
    pub groups_enumerated: u64,
    pub representative_numeric_group_decompression: String,
    pub map_index_name_hashes_present: bool,
    pub note: String,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct XteaProfile {
    pub ownership: String,
    pub provider_id: String,
    pub key_set_id: String,
    pub persist_raw_keys: bool,
    pub artifact_key_fingerprint_algorithm: String,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SemanticSource {
    #[serde(default)]
    pub repository: Option<String>,
    #[serde(default)]
    pub commit: Option<String>,
    #[serde(default)]
    pub tree: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RevisionGate {
    pub state: String,
    #[serde(default)]
    pub spec: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug)]
pub enum TargetProfileError {
    Yaml(serde_yaml_ng::Error),
    Validation(String),
}

impl fmt::Display for TargetProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Yaml(error) => write!(f, "target profile YAML is invalid: {error}"),
            Self::Validation(message) => write!(f, "target profile validation failed: {message}"),
        }
    }
}

impl std::error::Error for TargetProfileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Yaml(error) => Some(error),
            Self::Validation(_) => None,
        }
    }
}

impl From<serde_yaml_ng::Error> for TargetProfileError {
    fn from(value: serde_yaml_ng::Error) -> Self {
        Self::Yaml(value)
    }
}

impl TargetProfile {
    /// Parse and validate a runtime-usable v1 target profile.
    pub fn from_yaml_str(yaml: &str) -> Result<Self, TargetProfileError> {
        let profile: Self = serde_yaml_ng::from_str(yaml)?;
        profile.validate()?;
        Ok(profile)
    }

    /// Validate invariants required before a profile may drive cache decoding.
    pub fn validate(&self) -> Result<(), TargetProfileError> {
        if self.schema != TARGET_PROFILE_SCHEMA_V1 {
            return validation_error(format!(
                "unsupported schema {:?}; expected {TARGET_PROFILE_SCHEMA_V1}",
                self.schema
            ));
        }
        if self.profile_schema_version != 1 {
            return validation_error(format!(
                "unsupported profile_schema_version {}",
                self.profile_schema_version
            ));
        }
        if self.decoder_schema_version == 0 {
            return validation_error("decoder_schema_version must be nonzero");
        }
        if self.id.trim().is_empty() {
            return validation_error("profile id must not be empty");
        }
        if self.game_family != "osrs" {
            return validation_error(format!(
                "unsupported game_family {:?}; expected osrs",
                self.game_family
            ));
        }

        self.validate_cache_source()?;
        self.validate_fingerprint()?;
        self.validate_xtea()?;
        self.validate_semantic_sources()?;
        self.validate_revision_gates()?;
        self.validate_policy()?;
        Ok(())
    }

    /// Canonical semantic identity digest, independent of YAML formatting,
    /// comments, map insertion order, and line endings.
    pub fn identity_digest_v1(&self) -> [u8; 32] {
        let mut digest = Sha256::new();
        digest.update(TARGET_PROFILE_DIGEST_V1.as_bytes());
        digest.update([0]);

        digest_field(&mut digest, "schema", &self.schema);
        digest_field(&mut digest, "id", &self.id);
        digest_field(&mut digest, "game_family", &self.game_family);
        digest_u64(
            &mut digest,
            "profile_schema_version",
            u64::from(self.profile_schema_version),
        );
        digest_u64(
            &mut digest,
            "decoder_schema_version",
            u64::from(self.decoder_schema_version),
        );

        let source = &self.cache_source;
        digest_field(&mut digest, "cache_source.provider", &source.provider);
        digest_u64(&mut digest, "cache_source.cache_id", source.cache_id);
        digest_field(&mut digest, "cache_source.uri", &source.uri);
        digest_field(&mut digest, "cache_source.game", &source.game);
        digest_field(&mut digest, "cache_source.environment", &source.environment);
        digest_field(&mut digest, "cache_source.language", &source.language);
        digest_u64(&mut digest, "cache_source.build", u64::from(source.build));
        digest_field(&mut digest, "cache_source.timestamp", &source.timestamp);
        digest_field(&mut digest, "cache_source.format", &source.format);
        digest_u64(
            &mut digest,
            "cache_source.logical_archive_slots_present",
            u64::from(source.logical_archive_slots_present),
        );
        digest_u64(
            &mut digest,
            "cache_source.logical_archive_slots_total",
            u64::from(source.logical_archive_slots_total),
        );
        let mut empty_indices = source.empty_logical_indices.clone();
        empty_indices.sort_unstable();
        for index in empty_indices {
            digest_u64(
                &mut digest,
                "cache_source.empty_logical_index",
                u64::from(index),
            );
        }
        digest_u64(
            &mut digest,
            "cache_source.physical_content_indices",
            u64::from(source.physical_content_indices),
        );
        digest_u64(
            &mut digest,
            "cache_source.groups_present",
            source.groups_present,
        );
        digest_u64(
            &mut digest,
            "cache_source.groups_total",
            source.groups_total,
        );
        digest_u64(
            &mut digest,
            "cache_source.xtea_keys_present",
            source.xtea_keys_present,
        );
        digest_u64(
            &mut digest,
            "cache_source.xtea_keys_total",
            source.xtea_keys_total,
        );

        digest_field(
            &mut digest,
            "cache_fingerprint.algorithm",
            &self.cache_fingerprint.algorithm,
        );
        digest_field(
            &mut digest,
            "cache_fingerprint.value",
            self.cache_fingerprint.value.as_deref().unwrap_or(""),
        );

        digest_field(&mut digest, "xtea.ownership", &self.xtea.ownership);
        digest_field(&mut digest, "xtea.provider_id", &self.xtea.provider_id);
        digest_field(&mut digest, "xtea.key_set_id", &self.xtea.key_set_id);
        digest_field(
            &mut digest,
            "xtea.artifact_key_fingerprint_algorithm",
            &self.xtea.artifact_key_fingerprint_algorithm,
        );

        for (name, source) in &self.semantic_sources {
            digest_field(&mut digest, "semantic_source.name", name);
            digest_field(
                &mut digest,
                "semantic_source.repository",
                source.repository.as_deref().unwrap_or(""),
            );
            digest_field(
                &mut digest,
                "semantic_source.commit",
                source.commit.as_deref().unwrap_or(""),
            );
            digest_field(
                &mut digest,
                "semantic_source.tree",
                source.tree.as_deref().unwrap_or(""),
            );
        }

        for (name, gate) in &self.revision_gates {
            digest_field(&mut digest, "revision_gate.name", name);
            digest_field(&mut digest, "revision_gate.state", &gate.state);
            digest_field(
                &mut digest,
                "revision_gate.spec",
                gate.spec.as_deref().unwrap_or(""),
            );
        }

        for (name, value) in &self.policy {
            digest_field(&mut digest, "policy.name", name);
            digest_field(
                &mut digest,
                "policy.value",
                if *value { "true" } else { "false" },
            );
        }

        digest.finalize().into()
    }

    pub fn identity_digest_hex_v1(&self) -> String {
        bytes_to_hex(self.identity_digest_v1())
    }

    fn validate_cache_source(&self) -> Result<(), TargetProfileError> {
        let source = &self.cache_source;
        for (name, value) in [
            ("cache_source.provider", source.provider.as_str()),
            ("cache_source.uri", source.uri.as_str()),
            ("cache_source.game", source.game.as_str()),
            ("cache_source.environment", source.environment.as_str()),
            ("cache_source.language", source.language.as_str()),
            ("cache_source.timestamp", source.timestamp.as_str()),
            ("cache_source.format", source.format.as_str()),
        ] {
            if value.trim().is_empty() {
                return validation_error(format!("{name} must not be empty"));
            }
        }
        if source.logical_archive_slots_present > source.logical_archive_slots_total {
            return validation_error(
                "logical_archive_slots_present must not exceed logical_archive_slots_total",
            );
        }
        if source.groups_present > source.groups_total {
            return validation_error("groups_present must not exceed groups_total");
        }
        if source.xtea_keys_present > source.xtea_keys_total {
            return validation_error("xtea_keys_present must not exceed xtea_keys_total");
        }

        let unique_empty: BTreeSet<_> = source.empty_logical_indices.iter().copied().collect();
        if unique_empty.len() != source.empty_logical_indices.len() {
            return validation_error("empty_logical_indices must not contain duplicates");
        }
        if source
            .empty_logical_indices
            .iter()
            .any(|index| *index >= source.logical_archive_slots_total)
        {
            return validation_error(
                "empty_logical_indices must be within logical archive slot range",
            );
        }
        let represented = source
            .physical_content_indices
            .checked_add(source.empty_logical_indices.len() as u32)
            .ok_or_else(|| {
                TargetProfileError::Validation(
                    "logical/physical archive counts overflowed".to_owned(),
                )
            })?;
        if represented != source.logical_archive_slots_present {
            return validation_error(format!(
                "physical_content_indices ({}) + empty logical indices ({}) must equal logical_archive_slots_present ({})",
                source.physical_content_indices,
                source.empty_logical_indices.len(),
                source.logical_archive_slots_present
            ));
        }
        Ok(())
    }

    fn validate_fingerprint(&self) -> Result<(), TargetProfileError> {
        if self.cache_fingerprint.algorithm != CACHE_FINGERPRINT_V1 {
            return validation_error(format!(
                "unsupported cache fingerprint algorithm {:?}",
                self.cache_fingerprint.algorithm
            ));
        }
        let value = self.cache_fingerprint.value.as_deref().ok_or_else(|| {
            TargetProfileError::Validation("cache fingerprint is required".into())
        })?;
        if !is_lower_hex_sha256(value) {
            return validation_error(
                "cache fingerprint must be exactly 64 lowercase hexadecimal characters",
            );
        }
        if self.cache_fingerprint.state.trim().is_empty() {
            return validation_error("cache fingerprint state must not be empty");
        }
        Ok(())
    }

    fn validate_xtea(&self) -> Result<(), TargetProfileError> {
        if self.xtea.persist_raw_keys {
            return validation_error("target profiles must never persist raw XTEA keys");
        }
        for (name, value) in [
            ("xtea.ownership", self.xtea.ownership.as_str()),
            ("xtea.provider_id", self.xtea.provider_id.as_str()),
            ("xtea.key_set_id", self.xtea.key_set_id.as_str()),
            (
                "xtea.artifact_key_fingerprint_algorithm",
                self.xtea.artifact_key_fingerprint_algorithm.as_str(),
            ),
        ] {
            if value.trim().is_empty() {
                return validation_error(format!("{name} must not be empty"));
            }
        }
        Ok(())
    }

    fn validate_semantic_sources(&self) -> Result<(), TargetProfileError> {
        if self.semantic_sources.is_empty() {
            return validation_error("semantic_sources must not be empty");
        }
        for (name, source) in &self.semantic_sources {
            let has_commit = source
                .commit
                .as_ref()
                .is_some_and(|value| !value.is_empty());
            let has_tree = source.tree.as_ref().is_some_and(|value| !value.is_empty());
            if !has_commit && !has_tree {
                return validation_error(format!(
                    "semantic source {name:?} must pin a commit or tree"
                ));
            }
            if source.status.trim().is_empty() {
                return validation_error(format!("semantic source {name:?} must include a status"));
            }
        }
        Ok(())
    }

    fn validate_revision_gates(&self) -> Result<(), TargetProfileError> {
        for required in [
            "extended_object_model_ids",
            "object_sound_layout_220_plus",
            "sequence_layout_226_plus",
            "texture_layout_233_plus",
            "terrain_color_builder",
        ] {
            if !self.revision_gates.contains_key(required) {
                return validation_error(format!("required revision gate {required:?} is missing"));
            }
        }
        for (name, gate) in &self.revision_gates {
            if gate.state.trim().is_empty() {
                return validation_error(format!("revision gate {name:?} must include a state"));
            }
        }
        Ok(())
    }

    fn validate_policy(&self) -> Result<(), TargetProfileError> {
        for required_false in [
            "bundled_revision_180_cache_is_target",
            "fixed_texture_count_is_semantic_truth",
            "local_absolute_path_is_identity",
            "external_definition_structs_cross_osrs_cache_boundary",
        ] {
            match self.policy.get(required_false) {
                Some(false) => {}
                Some(true) => {
                    return validation_error(format!(
                        "policy {required_false:?} must remain false"
                    ));
                }
                None => {
                    return validation_error(format!(
                        "required policy assertion {required_false:?} is missing"
                    ));
                }
            }
        }
        Ok(())
    }
}

fn validation_error<T>(message: impl Into<String>) -> Result<T, TargetProfileError> {
    Err(TargetProfileError::Validation(message.into()))
}

fn digest_field(digest: &mut Sha256, name: &str, value: &str) {
    digest.update((name.len() as u64).to_be_bytes());
    digest.update(name.as_bytes());
    digest.update((value.len() as u64).to_be_bytes());
    digest.update(value.as_bytes());
}

fn digest_u64(digest: &mut Sha256, name: &str, value: u64) {
    digest_field(digest, name, &value.to_string());
}

fn is_lower_hex_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn bytes_to_hex(bytes: [u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    use std::io;
    use std::path::Path;

    const TARGET_PROFILE_PATH: &str = "../../profiles/osrs-live-241-2026-09-30-openrs2-2727.yaml";
    const TARGET_CACHE_FINGERPRINT: &str =
        "ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38";
    const TARGET_PROFILE_DIGEST: &str =
        "cfdefa9ef99eff799fcef4fdf0ec78d9fdcd72d8e5be78e1c154d018ab4575b7";

    type TestResult = Result<(), Box<dyn Error>>;

    fn target_profile_yaml() -> Result<String, io::Error> {
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(TARGET_PROFILE_PATH))
    }

    fn rejected_profile(
        result: Result<TargetProfile, TargetProfileError>,
    ) -> Result<TargetProfileError, io::Error> {
        match result {
            Err(error) => Ok(error),
            Ok(_) => Err(io::Error::other("profile unexpectedly validated")),
        }
    }

    #[test]
    fn committed_build_241_profile_parses_and_validates() -> TestResult {
        let yaml = target_profile_yaml()?;
        let profile = TargetProfile::from_yaml_str(&yaml)?;

        assert_eq!(profile.id, "osrs-live-241-2026-09-30-openrs2-2727");
        assert_eq!(profile.cache_source.build, 241);
        assert_eq!(profile.cache_source.physical_content_indices, 23);
        assert_eq!(profile.cache_source.groups_present, 117_584);
        assert_eq!(
            profile.cache_fingerprint.value.as_deref(),
            Some(TARGET_CACHE_FINGERPRINT)
        );
        assert_eq!(profile.identity_digest_hex_v1(), TARGET_PROFILE_DIGEST);
        Ok(())
    }

    #[test]
    fn identity_digest_ignores_yaml_comments_and_line_endings() -> TestResult {
        let yaml = target_profile_yaml()?;
        let baseline = TargetProfile::from_yaml_str(&yaml)?.identity_digest_v1();
        let reformatted = format!(
            "# formatting-only comment\r\n{}",
            yaml.replace('\n', "\r\n")
        );
        let changed = TargetProfile::from_yaml_str(&reformatted)?.identity_digest_v1();

        assert_eq!(baseline, changed);
        Ok(())
    }

    #[test]
    fn unknown_profile_field_is_rejected() -> TestResult {
        let yaml = format!("{}\nunexpected_field: true\n", target_profile_yaml()?);
        let error = rejected_profile(TargetProfile::from_yaml_str(&yaml))?;
        assert!(error.to_string().contains("unknown field"));
        Ok(())
    }

    #[test]
    fn invalid_cache_fingerprint_is_rejected() -> TestResult {
        let yaml = target_profile_yaml()?.replace(TARGET_CACHE_FINGERPRINT, "ABC");
        let error = rejected_profile(TargetProfile::from_yaml_str(&yaml))?;
        assert!(error.to_string().contains("64 lowercase hexadecimal"));
        Ok(())
    }

    #[test]
    fn raw_xtea_key_persistence_is_rejected() -> TestResult {
        let yaml =
            target_profile_yaml()?.replace("persist_raw_keys: false", "persist_raw_keys: true");
        let error = rejected_profile(TargetProfile::from_yaml_str(&yaml))?;
        assert!(error.to_string().contains("never persist raw XTEA keys"));
        Ok(())
    }

    #[test]
    fn logical_and_physical_index_count_mismatch_is_rejected() -> TestResult {
        let yaml = target_profile_yaml()?.replace(
            "physical_content_indices: 23",
            "physical_content_indices: 24",
        );
        let error = rejected_profile(TargetProfile::from_yaml_str(&yaml))?;
        assert!(
            error
                .to_string()
                .contains("must equal logical_archive_slots_present")
        );
        Ok(())
    }
}
