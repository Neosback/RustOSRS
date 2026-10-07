//! Cache-transport-independent target/profile provenance values.
//!
//! These types carry the identity established by M1 into canonical semantic
//! artifacts without exposing YAML, filesystem, OpenRS2, or `rune-fs` types.

use core::fmt;

pub const PROFILE_DIGEST_ALGORITHM_V1: &str = "rustosrs-target-profile-digest-v1";
pub const CACHE_FINGERPRINT_ALGORITHM_V1: &str = "rustosrs-cache-v1";

/// Exact 256-bit digest value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Digest256([u8; 32]);

impl Digest256 {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }

    pub fn from_lower_hex(value: &str) -> Result<Self, DigestParseError> {
        if value.len() != 64 {
            return Err(DigestParseError::Length { actual: value.len() });
        }

        let bytes = value.as_bytes();
        let mut decoded = [0_u8; 32];
        let mut index = 0;
        while index < decoded.len() {
            let high = lower_hex_nibble(bytes[index * 2])
                .ok_or(DigestParseError::InvalidLowerHex { index: index * 2 })?;
            let low = lower_hex_nibble(bytes[index * 2 + 1])
                .ok_or(DigestParseError::InvalidLowerHex { index: index * 2 + 1 })?;
            decoded[index] = (high << 4) | low;
            index += 1;
        }
        Ok(Self(decoded))
    }
}

impl fmt::Display for Digest256 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DigestParseError {
    Length { actual: usize },
    InvalidLowerHex { index: usize },
}

impl fmt::Display for DigestParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length { actual } => write!(
                formatter,
                "digest must contain exactly 64 lowercase hexadecimal characters; got {actual}"
            ),
            Self::InvalidLowerHex { index } => write!(
                formatter,
                "digest contains a non-lowercase-hexadecimal character at byte index {index}"
            ),
        }
    }
}

impl std::error::Error for DigestParseError {}

/// `rustosrs-target-profile-digest-v1` value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProfileDigest(Digest256);

impl ProfileDigest {
    pub const ALGORITHM: &'static str = PROFILE_DIGEST_ALGORITHM_V1;

    pub const fn from_digest(digest: Digest256) -> Self {
        Self(digest)
    }

    pub fn from_lower_hex(value: &str) -> Result<Self, DigestParseError> {
        Digest256::from_lower_hex(value).map(Self)
    }

    pub const fn digest(self) -> Digest256 {
        self.0
    }
}

impl fmt::Display for ProfileDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// `rustosrs-cache-v1` logical cache fingerprint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CacheFingerprint(Digest256);

impl CacheFingerprint {
    pub const ALGORITHM: &'static str = CACHE_FINGERPRINT_ALGORITHM_V1;

    pub const fn from_digest(digest: Digest256) -> Self {
        Self(digest)
    }

    pub fn from_lower_hex(value: &str) -> Result<Self, DigestParseError> {
        Digest256::from_lower_hex(value).map(Self)
    }

    pub const fn digest(self) -> Digest256 {
        self.0
    }
}

impl fmt::Display for CacheFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Minimal immutable target identity carried by canonical decoded artifacts.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TargetProvenance {
    profile_id: String,
    profile_digest: ProfileDigest,
    cache_fingerprint: CacheFingerprint,
    decoder_schema_version: u32,
}

impl TargetProvenance {
    pub fn new(
        profile_id: impl Into<String>,
        profile_digest: ProfileDigest,
        cache_fingerprint: CacheFingerprint,
        decoder_schema_version: u32,
    ) -> Result<Self, ProvenanceError> {
        let profile_id = profile_id.into();
        if profile_id.trim().is_empty() {
            return Err(ProvenanceError::EmptyProfileId);
        }
        if decoder_schema_version == 0 {
            return Err(ProvenanceError::ZeroDecoderSchemaVersion);
        }
        Ok(Self {
            profile_id,
            profile_digest,
            cache_fingerprint,
            decoder_schema_version,
        })
    }

    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }

    pub const fn profile_digest(&self) -> ProfileDigest {
        self.profile_digest
    }

    pub const fn cache_fingerprint(&self) -> CacheFingerprint {
        self.cache_fingerprint
    }

    pub const fn decoder_schema_version(&self) -> u32 {
        self.decoder_schema_version
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProvenanceError {
    EmptyProfileId,
    ZeroDecoderSchemaVersion,
}

impl fmt::Display for ProvenanceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyProfileId => formatter.write_str("target profile id must not be empty"),
            Self::ZeroDecoderSchemaVersion => {
                formatter.write_str("decoder schema version must be nonzero")
            }
        }
    }
}

impl std::error::Error for ProvenanceError {}

const fn lower_hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROFILE_DIGEST: &str =
        "cfdefa9ef99eff799fcef4fdf0ec78d9fdcd72d8e5be78e1c154d018ab4575b7";
    const CACHE_FINGERPRINT: &str =
        "ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38";

    #[test]
    fn digest_round_trips_exact_lowercase_hex() -> Result<(), DigestParseError> {
        let digest = Digest256::from_lower_hex(PROFILE_DIGEST)?;
        assert_eq!(digest.to_string(), PROFILE_DIGEST);
        Ok(())
    }

    #[test]
    fn digest_rejects_wrong_length_and_uppercase() {
        assert_eq!(
            Digest256::from_lower_hex("abc"),
            Err(DigestParseError::Length { actual: 3 })
        );

        let mut uppercase = PROFILE_DIGEST.to_owned();
        uppercase.replace_range(0..1, "C");
        assert_eq!(
            Digest256::from_lower_hex(&uppercase),
            Err(DigestParseError::InvalidLowerHex { index: 0 })
        );
    }

    #[test]
    fn typed_digests_cannot_be_interchanged_accidentally() -> Result<(), DigestParseError> {
        let profile = ProfileDigest::from_lower_hex(PROFILE_DIGEST)?;
        let cache = CacheFingerprint::from_lower_hex(CACHE_FINGERPRINT)?;

        assert_eq!(profile.to_string(), PROFILE_DIGEST);
        assert_eq!(cache.to_string(), CACHE_FINGERPRINT);
        assert_eq!(ProfileDigest::ALGORITHM, PROFILE_DIGEST_ALGORITHM_V1);
        assert_eq!(CacheFingerprint::ALGORITHM, CACHE_FINGERPRINT_ALGORITHM_V1);
        Ok(())
    }

    #[test]
    fn target_provenance_preserves_m1_identity() -> Result<(), Box<dyn std::error::Error>> {
        let profile = ProfileDigest::from_lower_hex(PROFILE_DIGEST)?;
        let cache = CacheFingerprint::from_lower_hex(CACHE_FINGERPRINT)?;
        let provenance = TargetProvenance::new(
            "osrs-live-241-2026-09-30-openrs2-2727",
            profile,
            cache,
            1,
        )?;

        assert_eq!(
            provenance.profile_id(),
            "osrs-live-241-2026-09-30-openrs2-2727"
        );
        assert_eq!(provenance.profile_digest(), profile);
        assert_eq!(provenance.cache_fingerprint(), cache);
        assert_eq!(provenance.decoder_schema_version(), 1);
        Ok(())
    }

    #[test]
    fn invalid_target_provenance_is_rejected() -> Result<(), DigestParseError> {
        let profile = ProfileDigest::from_lower_hex(PROFILE_DIGEST)?;
        let cache = CacheFingerprint::from_lower_hex(CACHE_FINGERPRINT)?;

        assert_eq!(
            TargetProvenance::new("  ", profile, cache, 1),
            Err(ProvenanceError::EmptyProfileId)
        );
        assert_eq!(
            TargetProvenance::new("profile", profile, cache, 0),
            Err(ProvenanceError::ZeroDecoderSchemaVersion)
        );
        Ok(())
    }
}
