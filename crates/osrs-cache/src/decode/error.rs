use super::DecoderContext;
use osrs_core::provenance::TargetProvenance;
use std::fmt;

/// Identifies the exact logical archive/file bytes being decoded.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArchiveFileProvenance {
    index_id: u8,
    group_id: u32,
    file_id: Option<u32>,
    xtea: Option<XteaKeyProvenance>,
}

impl ArchiveFileProvenance {
    pub const fn new(index_id: u8, group_id: u32, file_id: Option<u32>) -> Self {
        Self {
            index_id,
            group_id,
            file_id,
            xtea: None,
        }
    }

    pub fn with_xtea(mut self, xtea: XteaKeyProvenance) -> Self {
        self.xtea = Some(xtea);
        self
    }

    pub const fn index_id(&self) -> u8 {
        self.index_id
    }

    pub const fn group_id(&self) -> u32 {
        self.group_id
    }

    pub const fn file_id(&self) -> Option<u32> {
        self.file_id
    }

    pub fn xtea(&self) -> Option<&XteaKeyProvenance> {
        self.xtea.as_ref()
    }
}

/// XTEA provenance stores identity/fingerprint only, never raw key material.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct XteaKeyProvenance {
    provider_id: String,
    key_fingerprint: String,
}

impl XteaKeyProvenance {
    pub fn new(
        provider_id: impl Into<String>,
        key_fingerprint: impl Into<String>,
    ) -> Result<Self, XteaProvenanceError> {
        let provider_id = provider_id.into();
        let key_fingerprint = key_fingerprint.into();
        if provider_id.trim().is_empty() {
            return Err(XteaProvenanceError::EmptyProviderId);
        }
        if key_fingerprint.trim().is_empty() {
            return Err(XteaProvenanceError::EmptyKeyFingerprint);
        }
        Ok(Self {
            provider_id,
            key_fingerprint,
        })
    }

    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    pub fn key_fingerprint(&self) -> &str {
        &self.key_fingerprint
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XteaProvenanceError {
    EmptyProviderId,
    EmptyKeyFingerprint,
}

impl fmt::Display for XteaProvenanceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyProviderId => formatter.write_str("XTEA provider id must not be empty"),
            Self::EmptyKeyFingerprint => {
                formatter.write_str("XTEA key fingerprint must not be empty")
            }
        }
    }
}

impl std::error::Error for XteaProvenanceError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ByteSpan {
    offset: usize,
    length: usize,
}

impl ByteSpan {
    pub const fn new(offset: usize, length: usize) -> Self {
        Self { offset, length }
    }

    pub const fn offset(self) -> usize {
        self.offset
    }

    pub const fn length(self) -> usize {
        self.length
    }

    pub const fn is_empty(self) -> bool {
        self.length == 0
    }
}

/// Stable RustOSRS-owned categories for malformed/unsupported cache data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeErrorKind {
    UnexpectedEof {
        requested: usize,
        remaining: usize,
    },
    UnsupportedOpcode {
        decoder: &'static str,
        opcode: u32,
    },
    InvalidValue {
        field: &'static str,
        detail: String,
    },
    LimitExceeded {
        field: &'static str,
        requested: usize,
        limit: usize,
    },
    UnterminatedField {
        field: &'static str,
    },
    TrailingBytes {
        remaining: usize,
    },
}

impl fmt::Display for DecodeErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEof {
                requested,
                remaining,
            } => write!(
                formatter,
                "unexpected end of input: requested {requested} byte(s), {remaining} remaining"
            ),
            Self::UnsupportedOpcode { decoder, opcode } => {
                write!(formatter, "unsupported {decoder} opcode {opcode}")
            }
            Self::InvalidValue { field, detail } => {
                write!(formatter, "invalid {field}: {detail}")
            }
            Self::LimitExceeded {
                field,
                requested,
                limit,
            } => write!(
                formatter,
                "{field} exceeds decoder limit: requested {requested}, limit {limit}"
            ),
            Self::UnterminatedField { field } => {
                write!(formatter, "unterminated {field}")
            }
            Self::TrailingBytes { remaining } => {
                write!(formatter, "decoder left {remaining} trailing byte(s)")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DecodeErrorContext {
    target: TargetProvenance,
    build: u32,
    source: ArchiveFileProvenance,
    span: ByteSpan,
    opcode: Option<u32>,
}

/// Contextual decode failure carrying target/cache/archive/file identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeError {
    kind: DecodeErrorKind,
    context: Box<DecodeErrorContext>,
}

impl DecodeError {
    pub(crate) fn new(
        decoder_context: &DecoderContext,
        source: &ArchiveFileProvenance,
        span: ByteSpan,
        opcode: Option<u32>,
        kind: DecodeErrorKind,
    ) -> Self {
        Self {
            kind,
            context: Box::new(DecodeErrorContext {
                target: decoder_context.target_provenance().clone(),
                build: decoder_context.build(),
                source: source.clone(),
                span,
                opcode,
            }),
        }
    }

    pub fn kind(&self) -> &DecodeErrorKind {
        &self.kind
    }

    pub fn target_provenance(&self) -> &TargetProvenance {
        &self.context.target
    }

    pub const fn build(&self) -> u32 {
        self.context.build
    }

    pub fn source_provenance(&self) -> &ArchiveFileProvenance {
        &self.context.source
    }

    pub const fn span(&self) -> ByteSpan {
        self.context.span
    }

    pub const fn opcode(&self) -> Option<u32> {
        self.context.opcode
    }
}

impl fmt::Display for DecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let target = &self.context.target;
        let source = &self.context.source;
        write!(
            formatter,
            "{} [profile={}, build={}, index={}, group={}",
            self.kind,
            target.profile_id(),
            self.context.build,
            source.index_id(),
            source.group_id()
        )?;
        if let Some(file_id) = source.file_id() {
            write!(formatter, ", file={file_id}")?;
        }
        write!(
            formatter,
            ", offset={}, length={}",
            self.context.span.offset(),
            self.context.span.length()
        )?;
        if let Some(opcode) = self.context.opcode {
            write!(formatter, ", opcode={opcode}")?;
        }
        if let Some(xtea) = source.xtea() {
            write!(
                formatter,
                ", xtea_provider={}, xtea_key_fingerprint={}",
                xtea.provider_id(),
                xtea.key_fingerprint()
            )?;
        }
        formatter.write_str("]")
    }
}

impl std::error::Error for DecodeError {}

pub type DecodeResult<T> = Result<T, DecodeError>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::test_support;

    #[test]
    fn decode_error_preserves_target_archive_and_opcode_context(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let xtea = XteaKeyProvenance::new("openrs2-cache-2727", "sha256:example")?;
        let source = ArchiveFileProvenance::new(5, 12_345, Some(7)).with_xtea(xtea);
        let error = DecodeError::new(
            &context,
            &source,
            ByteSpan::new(19, 1),
            Some(92),
            DecodeErrorKind::UnsupportedOpcode {
                decoder: "object-definition",
                opcode: 92,
            },
        );

        assert_eq!(error.build(), 241);
        assert_eq!(error.source_provenance(), &source);
        assert_eq!(error.span(), ByteSpan::new(19, 1));
        assert_eq!(error.opcode(), Some(92));
        assert_eq!(
            error.target_provenance().profile_id(),
            "osrs-live-241-2026-09-30-openrs2-2727"
        );
        let rendered = error.to_string();
        assert!(rendered.contains("index=5"));
        assert!(rendered.contains("group=12345"));
        assert!(rendered.contains("file=7"));
        assert!(rendered.contains("opcode=92"));
        assert!(rendered.contains("xtea_provider=openrs2-cache-2727"));
        Ok(())
    }

    #[test]
    fn xtea_provenance_rejects_missing_identity() {
        assert_eq!(
            XteaKeyProvenance::new("", "fingerprint"),
            Err(XteaProvenanceError::EmptyProviderId)
        );
        assert_eq!(
            XteaKeyProvenance::new("provider", ""),
            Err(XteaProvenanceError::EmptyKeyFingerprint)
        );
    }
}
