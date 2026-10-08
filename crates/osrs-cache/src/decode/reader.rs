use super::{
    ArchiveFileProvenance, ByteSpan, DecodeError, DecodeErrorKind, DecodeResult, DecodeSubject,
    DecoderContext,
};

const CP1252_ASCII_EXTENSION: [char; 32] = [
    '€', '\0', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\0', 'Ž', '\0', '\0', '‘',
    '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\0', 'ž', 'Ÿ',
];

/// Bounds-checked big-endian reader used by revision-aware cache decoders.
///
/// The reader never panics on malformed/truncated input. Every failure carries
/// the decoder target and logical archive/file provenance supplied at creation.
pub struct BinaryReader<'a> {
    bytes: &'a [u8],
    offset: usize,
    context: &'a DecoderContext,
    source: &'a ArchiveFileProvenance,
    subject: Option<DecodeSubject>,
}

impl<'a> BinaryReader<'a> {
    pub const fn new(
        bytes: &'a [u8],
        context: &'a DecoderContext,
        source: &'a ArchiveFileProvenance,
    ) -> Self {
        Self {
            bytes,
            offset: 0,
            context,
            source,
            subject: None,
        }
    }

    pub fn with_subject(mut self, subject: DecodeSubject) -> Self {
        self.subject = Some(subject);
        self
    }

    pub const fn offset(&self) -> usize {
        self.offset
    }

    pub const fn len(&self) -> usize {
        self.bytes.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub const fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }

    pub fn peek_u8(&self) -> DecodeResult<u8> {
        self.bytes.get(self.offset).copied().ok_or_else(|| {
            self.error_at(
                self.offset,
                1,
                None,
                DecodeErrorKind::UnexpectedEof {
                    requested: 1,
                    remaining: 0,
                },
            )
        })
    }

    pub fn read_u8(&mut self) -> DecodeResult<u8> {
        let bytes = self.read_bytes(1)?;
        Ok(bytes[0])
    }

    pub fn read_i8(&mut self) -> DecodeResult<i8> {
        self.read_u8().map(|value| value as i8)
    }

    pub fn read_u16_be(&mut self) -> DecodeResult<u16> {
        let bytes = self.read_bytes(2)?;
        Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
    }

    pub fn read_i16_be(&mut self) -> DecodeResult<i16> {
        let bytes = self.read_bytes(2)?;
        Ok(i16::from_be_bytes([bytes[0], bytes[1]]))
    }

    pub fn read_u24_be(&mut self) -> DecodeResult<u32> {
        let bytes = self.read_bytes(3)?;
        Ok((u32::from(bytes[0]) << 16) | (u32::from(bytes[1]) << 8) | u32::from(bytes[2]))
    }

    pub fn read_u32_be(&mut self) -> DecodeResult<u32> {
        let bytes = self.read_bytes(4)?;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub fn read_i32_be(&mut self) -> DecodeResult<i32> {
        let bytes = self.read_bytes(4)?;
        Ok(i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub fn read_bytes(&mut self, length: usize) -> DecodeResult<&'a [u8]> {
        let start = self.offset;
        let Some(end) = start.checked_add(length) else {
            return Err(self.error_at(
                start,
                length,
                None,
                DecodeErrorKind::LimitExceeded {
                    field: "byte range",
                    requested: length,
                    limit: self.remaining(),
                },
            ));
        };
        let Some(bytes) = self.bytes.get(start..end) else {
            return Err(self.error_at(
                start,
                length,
                None,
                DecodeErrorKind::UnexpectedEof {
                    requested: length,
                    remaining: self.remaining(),
                },
            ));
        };
        self.offset = end;
        Ok(bytes)
    }

    pub fn skip(&mut self, length: usize) -> DecodeResult<()> {
        self.read_bytes(length).map(|_| ())
    }

    /// RuneScape unsigned-short-smart primitive.
    ///
    /// Values below 128 occupy one byte. Larger values occupy two bytes with
    /// 32768 subtracted from the encoded unsigned short.
    pub fn read_unsigned_short_smart(&mut self) -> DecodeResult<u16> {
        if self.peek_u8()? < 128 {
            self.read_u8().map(u16::from)
        } else {
            self.read_u16_be().map(|value| value - 32_768)
        }
    }

    /// Read a NUL-terminated byte field without allocating.
    ///
    /// `max_length` bounds the payload length, excluding the terminator.
    pub fn read_null_terminated_bytes(
        &mut self,
        field: &'static str,
        max_length: usize,
    ) -> DecodeResult<&'a [u8]> {
        let start = self.offset;
        let remaining = &self.bytes[start..];
        let terminator = remaining.iter().position(|byte| *byte == 0);

        match terminator {
            Some(length) if length <= max_length => {
                let payload = &remaining[..length];
                self.offset = start + length + 1;
                Ok(payload)
            }
            Some(length) => Err(self.error_at(
                start,
                length,
                None,
                DecodeErrorKind::LimitExceeded {
                    field,
                    requested: length,
                    limit: max_length,
                },
            )),
            None if remaining.len() > max_length => Err(self.error_at(
                start,
                remaining.len(),
                None,
                DecodeErrorKind::LimitExceeded {
                    field,
                    requested: remaining.len(),
                    limit: max_length,
                },
            )),
            None => Err(self.error_at(
                start,
                remaining.len(),
                None,
                DecodeErrorKind::UnterminatedField { field },
            )),
        }
    }

    /// Read the client's NUL-terminated CP1252-like cache string format.
    ///
    /// Bytes 128..159 use the pinned client's extension table. Undefined
    /// extension entries decode to `?`, matching the target client. Other
    /// nonzero bytes map directly to their same-valued Unicode scalar.
    pub fn read_cp1252_string(&mut self) -> DecodeResult<String> {
        let max_length = self.remaining();
        let bytes = self.read_null_terminated_bytes("cp1252 string", max_length)?;
        let mut decoded = String::with_capacity(bytes.len());

        for &byte in bytes {
            let character = if (128..160).contains(&byte) {
                let extension = CP1252_ASCII_EXTENSION[usize::from(byte - 128)];
                if extension == '\0' { '?' } else { extension }
            } else {
                char::from(byte)
            };
            decoded.push(character);
        }

        Ok(decoded)
    }

    pub fn finish(self) -> DecodeResult<()> {
        if self.remaining() == 0 {
            return Ok(());
        }
        Err(self.error_at(
            self.offset,
            self.remaining(),
            None,
            DecodeErrorKind::TrailingBytes {
                remaining: self.remaining(),
            },
        ))
    }

    /// Construct an unsupported-opcode error at the byte where the opcode was
    /// read. Decoders should use this rather than logging/skipping unknown data.
    pub fn unsupported_opcode(
        &self,
        decoder: &'static str,
        opcode: u32,
        opcode_offset: usize,
        encoded_length: usize,
    ) -> DecodeError {
        self.error_at(
            opcode_offset,
            encoded_length,
            Some(opcode),
            DecodeErrorKind::UnsupportedOpcode { decoder, opcode },
        )
    }

    pub fn invalid_value(
        &self,
        field: &'static str,
        detail: impl Into<String>,
        span: ByteSpan,
        opcode: Option<u32>,
    ) -> DecodeError {
        self.error_at(
            span.offset(),
            span.length(),
            opcode,
            DecodeErrorKind::InvalidValue {
                field,
                detail: detail.into(),
            },
        )
    }

    fn error_at(
        &self,
        offset: usize,
        length: usize,
        opcode: Option<u32>,
        kind: DecodeErrorKind,
    ) -> DecodeError {
        DecodeError::new(
            self.context,
            self.source,
            self.subject.clone(),
            ByteSpan::new(offset, length),
            opcode,
            kind,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::test_support;

    #[test]
    fn reader_decodes_big_endian_primitives_and_smart_values()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let source = test_support::source();
        let bytes = [
            0x7f, 0xff, 0x80, 0x01, 0x02, 0x03, 0x89, 0xab, 0xcd, 0xef, 0x81, 0x00,
        ];
        let mut reader = BinaryReader::new(&bytes, &context, &source);

        assert_eq!(reader.read_u8()?, 0x7f);
        assert_eq!(reader.read_i8()?, -1);
        assert_eq!(reader.read_u16_be()?, 0x8001);
        assert_eq!(reader.read_u24_be()?, 0x020389);
        assert_eq!(reader.read_u32_be()?, 0xabcdef81);
        assert_eq!(reader.read_unsigned_short_smart()?, 0);
        reader.finish()?;
        Ok(())
    }

    #[test]
    fn reader_smart_uses_one_or_two_byte_forms() -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let source = test_support::source();
        let bytes = [127, 0x80, 0x80, 0xff, 0xff];
        let mut reader = BinaryReader::new(&bytes, &context, &source);

        assert_eq!(reader.read_unsigned_short_smart()?, 127);
        assert_eq!(reader.read_unsigned_short_smart()?, 128);
        assert_eq!(reader.read_unsigned_short_smart()?, 32_767);
        reader.finish()?;
        Ok(())
    }

    #[test]
    fn truncated_reads_return_contextual_error_without_advancing()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let source = ArchiveFileProvenance::new(2, 6, Some(17));
        let bytes = [0xaa, 0xbb, 0xcc];
        let mut reader = BinaryReader::new(&bytes, &context, &source);

        assert_eq!(reader.read_u8()?, 0xaa);
        let error = match reader.read_u32_be() {
            Err(error) => error,
            Ok(_) => return Err("truncated read unexpectedly succeeded".into()),
        };
        assert_eq!(reader.offset(), 1);
        assert_eq!(error.span(), ByteSpan::new(1, 4));
        assert_eq!(error.source_provenance().file_id(), Some(17));
        assert_eq!(
            error.kind(),
            &DecodeErrorKind::UnexpectedEof {
                requested: 4,
                remaining: 2
            }
        );
        Ok(())
    }

    #[test]
    fn null_terminated_fields_are_bounded_and_zero_copy() -> Result<(), Box<dyn std::error::Error>>
    {
        let context = test_support::target_context()?;
        let source = test_support::source();
        let bytes = b"door\0rest";
        let mut reader = BinaryReader::new(bytes, &context, &source);

        assert_eq!(reader.read_null_terminated_bytes("name", 16)?, b"door");
        assert_eq!(reader.offset(), 5);
        assert_eq!(reader.read_bytes(4)?, b"rest");
        reader.finish()?;
        Ok(())
    }

    #[test]
    fn cp1252_strings_match_target_extension_and_undefined_byte_behavior()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let source = test_support::source();
        let bytes = [b'A', 128, 129, 130, 159, 255, 0];
        let mut reader = BinaryReader::new(&bytes, &context, &source);

        assert_eq!(reader.read_cp1252_string()?, "A€?‚Ÿÿ");
        reader.finish()?;
        Ok(())
    }

    #[test]
    fn null_terminated_field_limit_and_missing_terminator_are_typed()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let source = test_support::source();

        let mut too_long = BinaryReader::new(b"abcd\0", &context, &source);
        let limit_error = match too_long.read_null_terminated_bytes("name", 3) {
            Err(error) => error,
            Ok(_) => return Err("over-limit field unexpectedly succeeded".into()),
        };
        assert_eq!(
            limit_error.kind(),
            &DecodeErrorKind::LimitExceeded {
                field: "name",
                requested: 4,
                limit: 3
            }
        );
        assert_eq!(too_long.offset(), 0);

        let mut unterminated = BinaryReader::new(b"abc", &context, &source);
        let terminator_error = match unterminated.read_null_terminated_bytes("name", 3) {
            Err(error) => error,
            Ok(_) => return Err("unterminated field unexpectedly succeeded".into()),
        };
        assert_eq!(
            terminator_error.kind(),
            &DecodeErrorKind::UnterminatedField { field: "name" }
        );
        assert_eq!(unterminated.offset(), 0);
        Ok(())
    }

    #[test]
    fn unsupported_opcode_error_carries_opcode_and_byte_span()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let source = test_support::source();
        let bytes = [92];
        let reader = BinaryReader::new(&bytes, &context, &source)
            .with_subject(DecodeSubject::ObjectDefinition(17));
        let error = reader.unsupported_opcode("object-definition", 92, 0, 1);

        assert_eq!(error.opcode(), Some(92));
        assert_eq!(error.subject(), Some(&DecodeSubject::ObjectDefinition(17)));
        assert_eq!(error.span(), ByteSpan::new(0, 1));
        assert_eq!(
            error.kind(),
            &DecodeErrorKind::UnsupportedOpcode {
                decoder: "object-definition",
                opcode: 92
            }
        );
        Ok(())
    }
}
