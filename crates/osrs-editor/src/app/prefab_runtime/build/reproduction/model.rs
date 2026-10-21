//! Closed reproduction envelope. Canonical transport text stays intact until its strict decoder.
use super::*;
use osrs_core::provenance::{CacheFingerprint, ProfileDigest, TargetProvenance};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::app) struct Source {
    pub profile_id: String,
    pub profile_digest: String,
    pub cache_fingerprint: String,
    pub decoder_schema_version: u32,
    pub build: u32,
}
impl Source {
    pub(in crate::app) fn new(target: &TargetProvenance, build: u32) -> Self {
        Self {
            profile_id: target.profile_id().into(),
            profile_digest: target.profile_digest().to_string(),
            cache_fingerprint: target.cache_fingerprint().to_string(),
            decoder_schema_version: target.decoder_schema_version(),
            build,
        }
    }
    pub(super) fn validate(&self) -> Result<(), Error> {
        let profile = ProfileDigest::from_lower_hex(&self.profile_digest)
            .map_err(|e| Error::Input(e.to_string()))?;
        let cache = CacheFingerprint::from_lower_hex(&self.cache_fingerprint)
            .map_err(|e| Error::Input(e.to_string()))?;
        TargetProvenance::new(
            self.profile_id.clone(),
            profile,
            cache,
            self.decoder_schema_version,
        )
        .map_err(|e| Error::Input(format!("Source identity: {e:?}")))?;
        if self.build == 0 {
            return Err(Error::Input("Source build is missing".into()));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::app) struct BuildIdentity {
    pub package_version: String,
    pub workspace_source_digest: String,
    pub target: String,
    pub profile: String,
    pub opt_level: String,
    pub rustc: String,
    pub cargo_features: String,
    pub rustflags: String,
}
impl BuildIdentity {
    pub(super) fn current() -> Self {
        Self {
            package_version: env!("CARGO_PKG_VERSION").into(),
            workspace_source_digest: env!("RUSTOSRS_SOURCE_SHA256").into(),
            target: env!("RUSTOSRS_BUILD_TARGET").into(),
            profile: env!("RUSTOSRS_BUILD_PROFILE").into(),
            opt_level: env!("RUSTOSRS_BUILD_OPT_LEVEL").into(),
            rustc: env!("RUSTOSRS_RUSTC_VERSION").into(),
            cargo_features: env!("RUSTOSRS_BUILD_FEATURES").into(),
            rustflags: env!("RUSTOSRS_BUILD_RUSTFLAGS").into(),
        }
    }
    pub(super) fn validate(&self) -> Result<(), Error> {
        digest(&self.workspace_source_digest)?;
        if [
            &self.package_version,
            &self.target,
            &self.profile,
            &self.opt_level,
            &self.rustc,
        ]
        .iter()
        .any(|s| s.trim().is_empty())
        {
            return Err(Error::Input("Incomplete editor build identity".into()));
        }
        Ok(())
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Inputs {
    pub source: Source,
    pub editor_build: BuildIdentity,
    pub source_generation: u64,
    pub semantic_revision: u64,
    pub origin: [i32; 2],
    pub plane: u8,
    pub request: BuildForm,
    pub candidate: BuildForm,
    pub kit_json: String,
    pub footprints: Vec<super::super::footprints::Record>,
    pub initial_map_json: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Bundle {
    pub format: String,
    pub feature: String,
    pub planner_contract: String,
    pub inputs_digest: String,
    pub inputs: Inputs,
    pub expected_building_digest: String,
    pub expected_patch_digest: String,
    pub expected_result_digest: String,
}
#[derive(Debug, Serialize)]
pub(in crate::app) struct Verification {
    pub status: &'static str,
    pub feature: &'static str,
    pub patch_digest: String,
    pub result_digest: String,
    pub operation_count: usize,
    pub captured_build: BuildIdentity,
    pub verifier_build: BuildIdentity,
    pub same_recorded_build_context: bool,
    pub source: Source,
}
