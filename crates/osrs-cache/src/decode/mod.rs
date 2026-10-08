//! Revision-aware RustOSRS decoder infrastructure.
//!
//! This module owns cache-byte decoding mechanics and provenance. It must not
//! leak `rune-fs` transport types or move scene/render/editor semantics into the
//! cache layer.

mod context;
mod error;
mod reader;

pub use context::{DecoderContext, DecoderContextError};
pub use error::{
    ArchiveFileProvenance, ByteSpan, DecodeError, DecodeErrorKind, DecodeResult,
    XteaKeyProvenance, XteaProvenanceError,
};
pub use reader::BinaryReader;

#[cfg(test)]
pub(crate) mod test_support {
    use super::{ArchiveFileProvenance, DecoderContext};
    use crate::profile::{TargetProfile, TargetProfileError};

    const TARGET_PROFILE_YAML: &str =
        include_str!("../../../../profiles/osrs-live-241-2026-09-30-openrs2-2727.yaml");

    pub(crate) fn target_context() -> Result<DecoderContext, DecoderContextError> {
        let profile = TargetProfile::from_yaml_str(TARGET_PROFILE_YAML)
            .map_err(DecoderContextError::InvalidProfile)?;
        DecoderContext::from_profile(&profile)
    }

    pub(crate) fn source() -> ArchiveFileProvenance {
        ArchiveFileProvenance::new(2, 6, Some(0))
    }

    #[allow(dead_code)]
    pub(crate) fn target_profile() -> Result<TargetProfile, TargetProfileError> {
        TargetProfile::from_yaml_str(TARGET_PROFILE_YAML)
    }
}
