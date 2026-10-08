use crate::profile::{TargetProfile, TargetProfileError};
use osrs_core::provenance::{
    CacheFingerprint, Digest256, DigestParseError, ProfileDigest, ProvenanceError, TargetProvenance,
};
use std::collections::BTreeMap;
use std::fmt;

/// Validated target identity and revision gates used by all RustOSRS decoders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecoderContext {
    target: TargetProvenance,
    build: u32,
    revision_gates: BTreeMap<String, String>,
}

impl DecoderContext {
    /// Build a decoder context from an already-declared target profile.
    ///
    /// The profile is revalidated here because decoder construction is a trust
    /// boundary. Canonical target provenance is derived once and then cloned
    /// into decode failures/artifacts without exposing YAML or transport types.
    pub fn from_profile(profile: &TargetProfile) -> Result<Self, DecoderContextError> {
        profile
            .validate()
            .map_err(DecoderContextError::InvalidProfile)?;

        let profile_digest =
            ProfileDigest::from_digest(Digest256::from_bytes(profile.identity_digest_v1()));
        let cache_fingerprint = profile
            .cache_fingerprint
            .value
            .as_deref()
            .ok_or(DecoderContextError::MissingCacheFingerprint)
            .and_then(|value| {
                CacheFingerprint::from_lower_hex(value)
                    .map_err(DecoderContextError::InvalidCacheFingerprint)
            })?;
        let target = TargetProvenance::new(
            profile.id.clone(),
            profile_digest,
            cache_fingerprint,
            profile.decoder_schema_version,
        )
        .map_err(DecoderContextError::InvalidTargetProvenance)?;

        let revision_gates = profile
            .revision_gates
            .iter()
            .map(|(name, gate)| (name.clone(), gate.state.clone()))
            .collect();

        Ok(Self {
            target,
            build: profile.cache_source.build,
            revision_gates,
        })
    }

    pub fn target_provenance(&self) -> &TargetProvenance {
        &self.target
    }

    pub const fn build(&self) -> u32 {
        self.build
    }

    /// Raw validated gate state from the target profile.
    pub fn revision_gate_state(&self, name: &str) -> Option<&str> {
        self.revision_gates.get(name).map(String::as_str)
    }

    pub fn requires_revision_gate(&self, name: &str) -> bool {
        self.revision_gate_state(name) == Some("required")
    }

    pub fn blocks_revision_gate(&self, name: &str) -> bool {
        self.revision_gate_state(name) == Some("blocked")
    }
}

#[derive(Debug)]
pub enum DecoderContextError {
    InvalidProfile(TargetProfileError),
    MissingCacheFingerprint,
    InvalidCacheFingerprint(DigestParseError),
    InvalidTargetProvenance(ProvenanceError),
}

impl fmt::Display for DecoderContextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProfile(error) => {
                write!(formatter, "invalid decoder target profile: {error}")
            }
            Self::MissingCacheFingerprint => {
                formatter.write_str("decoder target profile is missing its cache fingerprint")
            }
            Self::InvalidCacheFingerprint(error) => {
                write!(formatter, "decoder cache fingerprint is invalid: {error}")
            }
            Self::InvalidTargetProvenance(error) => {
                write!(formatter, "decoder target provenance is invalid: {error}")
            }
        }
    }
}

impl std::error::Error for DecoderContextError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidProfile(error) => Some(error),
            Self::InvalidCacheFingerprint(error) => Some(error),
            Self::InvalidTargetProvenance(error) => Some(error),
            Self::MissingCacheFingerprint => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::test_support;

    const TARGET_PROFILE_DIGEST: &str =
        "cfdefa9ef99eff799fcef4fdf0ec78d9fdcd72d8e5be78e1c154d018ab4575b7";
    const TARGET_CACHE_FINGERPRINT: &str =
        "ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38";

    #[test]
    fn target_profile_becomes_canonical_decoder_context() -> Result<(), Box<dyn std::error::Error>>
    {
        let context = test_support::target_context()?;
        let target = context.target_provenance();

        assert_eq!(context.build(), 241);
        assert_eq!(target.profile_id(), "osrs-live-241-2026-09-30-openrs2-2727");
        assert_eq!(target.profile_digest().to_string(), TARGET_PROFILE_DIGEST);
        assert_eq!(
            target.cache_fingerprint().to_string(),
            TARGET_CACHE_FINGERPRINT
        );
        assert_eq!(target.decoder_schema_version(), 1);
        assert!(context.requires_revision_gate("extended_object_model_ids"));
        assert!(context.requires_revision_gate("texture_layout_233_plus"));
        assert!(context.blocks_revision_gate("terrain_color_builder"));
        assert_eq!(context.revision_gate_state("not-a-real-gate"), None);
        Ok(())
    }
}
