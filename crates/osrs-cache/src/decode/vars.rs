use super::{ArchiveFileProvenance, BinaryReader, DecodeResult, DecoderContext};
use osrs_core::definitions::{DefinitionIdentity, VarbitDefinition, VarpDefinition};
use osrs_core::ids::{VarbitId, VarpId};

/// Decode a varbit definition.
///
/// The encoded `end_bit` is inclusive in the reference bit-mask convention, so
/// a range `start_bit=3, end_bit=7` spans five bits. M8 owns runtime state
/// selection; M3 preserves the exact decoded endpoints.
pub fn decode_varbit(
    id: VarbitId,
    bytes: &[u8],
    context: &DecoderContext,
    source: &ArchiveFileProvenance,
) -> DecodeResult<VarbitDefinition> {
    let mut reader = BinaryReader::new(bytes, context, source);
    let mut base_varp = VarpId::new(0);
    let mut start_bit = 0_u8;
    let mut end_bit = 0_u8;

    loop {
        let opcode_offset = reader.offset();
        let opcode = reader.read_u8()?;
        match opcode {
            0 => break,
            1 => {
                base_varp = VarpId::new(u32::from(reader.read_u16_be()?));
                start_bit = reader.read_u8()?;
                end_bit = reader.read_u8()?;
            }
            _ => {
                return Err(reader.unsupported_opcode(
                    "varbit-definition",
                    u32::from(opcode),
                    opcode_offset,
                    1,
                ));
            }
        }
    }

    reader.finish()?;
    Ok(VarbitDefinition {
        identity: DefinitionIdentity::new(id, context.target_provenance().clone()),
        base_varp,
        start_bit,
        end_bit,
    })
}

/// Decode a varp definition.
pub fn decode_varp(
    id: VarpId,
    bytes: &[u8],
    context: &DecoderContext,
    source: &ArchiveFileProvenance,
) -> DecodeResult<VarpDefinition> {
    let mut reader = BinaryReader::new(bytes, context, source);
    let mut client_type = 0_u16;

    loop {
        let opcode_offset = reader.offset();
        let opcode = reader.read_u8()?;
        match opcode {
            0 => break,
            5 => client_type = reader.read_u16_be()?,
            _ => {
                return Err(reader.unsupported_opcode(
                    "varp-definition",
                    u32::from(opcode),
                    opcode_offset,
                    1,
                ));
            }
        }
    }

    reader.finish()?;
    Ok(VarpDefinition {
        identity: DefinitionIdentity::new(id, context.target_provenance().clone()),
        client_type,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::{ByteSpan, DecodeErrorKind, test_support};

    #[test]
    fn varbit_decodes_backing_varp_and_inclusive_bit_range()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let source = ArchiveFileProvenance::new(2, 14, Some(123));
        let definition = decode_varbit(
            VarbitId::new(123),
            &[1, 0x12, 0x34, 3, 7, 0],
            &context,
            &source,
        )?;

        assert_eq!(definition.identity.id, VarbitId::new(123));
        assert_eq!(&definition.identity.provenance, context.target_provenance());
        assert_eq!(definition.base_varp, VarpId::new(0x1234));
        assert_eq!(definition.start_bit, 3);
        assert_eq!(definition.end_bit, 7);
        assert_eq!(definition.end_bit - definition.start_bit + 1, 5);
        Ok(())
    }

    #[test]
    fn varbit_defaults_match_reference_zero_initialization()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let source = ArchiveFileProvenance::new(2, 14, Some(124));
        let definition = decode_varbit(VarbitId::new(124), &[0], &context, &source)?;

        assert_eq!(definition.base_varp, VarpId::new(0));
        assert_eq!(definition.start_bit, 0);
        assert_eq!(definition.end_bit, 0);
        Ok(())
    }

    #[test]
    fn varp_decodes_client_type_and_default() -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let source = ArchiveFileProvenance::new(2, 16, Some(33));
        let decoded = decode_varp(VarpId::new(33), &[5, 0xab, 0xcd, 0], &context, &source)?;
        let defaulted = decode_varp(VarpId::new(34), &[0], &context, &source)?;

        assert_eq!(decoded.client_type, 0xabcd);
        assert_eq!(decoded.identity.id, VarpId::new(33));
        assert_eq!(defaulted.client_type, 0);
        Ok(())
    }

    #[test]
    fn unknown_var_opcode_is_typed_and_contextual() -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let source = ArchiveFileProvenance::new(2, 14, Some(999));
        let error = match decode_varbit(VarbitId::new(999), &[2, 0], &context, &source) {
            Err(error) => error,
            Ok(_) => return Err("unknown varbit opcode unexpectedly decoded".into()),
        };

        assert_eq!(error.opcode(), Some(2));
        assert_eq!(error.span(), ByteSpan::new(0, 1));
        assert_eq!(error.source_provenance(), &source);
        assert_eq!(
            error.kind(),
            &DecodeErrorKind::UnsupportedOpcode {
                decoder: "varbit-definition",
                opcode: 2,
            }
        );
        Ok(())
    }
}
