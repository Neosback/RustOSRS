use super::{ArchiveFileProvenance, BinaryReader, ByteSpan, DecodeResult, DecoderContext};
use osrs_core::definitions::{DefinitionIdentity, SequenceDefinition, SequenceRange};
use osrs_core::ids::{FrameId, ItemId, SequenceId, SkeletalAnimationId};

const SEQUENCE_LAYOUT_226_PLUS_GATE: &str = "sequence_layout_226_plus";

/// Decode one build-241 sequence definition.
///
/// The target moved cached/skeletal animation metadata beginning with the
/// revision-226 layout. Non-canonical fields are consumed at their exact target
/// widths so following semantic opcodes remain aligned.
pub fn decode_sequence_definition(
    id: SequenceId,
    bytes: &[u8],
    context: &DecoderContext,
    source: &ArchiveFileProvenance,
) -> DecodeResult<SequenceDefinition> {
    let mut reader = BinaryReader::new(bytes, context, source);
    if !context.requires_revision_gate(SEQUENCE_LAYOUT_226_PLUS_GATE) {
        return Err(reader.invalid_value(
            "sequence layout",
            "target profile does not require sequence_layout_226_plus",
            ByteSpan::new(0, 0),
            None,
        ));
    }

    let mut frame_ids = Vec::new();
    let mut frame_delays = Vec::new();
    let mut frame_step = None;
    let mut interleave = None;
    let mut max_loops = 99_u16;
    let mut precedence_animating = None;
    let mut priority = None;
    let mut reply_mode = 2_u8;
    let mut left_hand_item = None;
    let mut right_hand_item = None;
    let mut skeletal_animation = None;
    let mut skeletal_range = None;
    let mut skeletal_mask = None;

    loop {
        let opcode_offset = reader.offset();
        let opcode = reader.read_u8()?;
        match opcode {
            0 => break,
            1 => {
                let count = usize::from(reader.read_u16_be()?);
                frame_delays.clear();
                frame_delays.reserve(count);
                for _ in 0..count {
                    frame_delays.push(reader.read_u16_be()?);
                }

                let mut lows = Vec::with_capacity(count);
                for _ in 0..count {
                    lows.push(reader.read_u16_be()?);
                }

                frame_ids.clear();
                frame_ids.reserve(count);
                for low in lows {
                    let high = reader.read_u16_be()?;
                    frame_ids.push(FrameId::new((u32::from(high) << 16) | u32::from(low)));
                }
            }
            2 => frame_step = Some(reader.read_u16_be()?),
            3 => {
                let count = usize::from(reader.read_u8()?);
                let mut entries = Vec::with_capacity(count);
                for _ in 0..count {
                    entries.push(u16::from(reader.read_u8()?));
                }
                // The client appends an internal 9,999,999 sentinel. It is not
                // encoded cache data and therefore is not canonical state.
                interleave = Some(entries);
            }
            4 => {
                // Client stretch flag is not part of the current canonical
                // sequence inputs.
            }
            5 => {
                // Forced priority is client animation scheduling metadata not
                // currently owned by SequenceDefinition.
                reader.skip(1)?;
            }
            6 => left_hand_item = Some(ItemId::new(u32::from(reader.read_u16_be()?))),
            7 => right_hand_item = Some(ItemId::new(u32::from(reader.read_u16_be()?))),
            8 => max_loops = u16::from(reader.read_u8()?),
            9 => precedence_animating = Some(reader.read_u8()?),
            10 => priority = Some(reader.read_u8()?),
            11 => reply_mode = reader.read_u8()?,
            12 => {
                let count = usize::from(reader.read_u8()?);
                // Chat-frame ids use the same split-low/split-high u32 layout,
                // but they are not current canonical editor inputs.
                reader.skip(count * 2)?;
                reader.skip(count * 2)?;
            }
            13 => {
                let encoded = reader.read_i32_be()?;
                skeletal_animation = (encoded >= 0)
                    .then(|| SkeletalAnimationId::new(encoded as u32));
            }
            14 => {
                // Build 241 skeletal event map. Each entry is:
                // key:u16 + resource:u16 + four u8 fields.
                let count = usize::from(reader.read_u16_be()?);
                reader.skip(count * 8)?;
            }
            15 => {
                skeletal_range = Some(SequenceRange {
                    start: reader.read_u16_be()?,
                    end: reader.read_u16_be()?,
                });
            }
            16 => {
                // Signed animation-height offset, retained by the client but
                // not currently a canonical sequence input.
                let _ = reader.read_i8()?;
            }
            17 => {
                let mut mask = vec![false; 256];
                let count = usize::from(reader.read_u8()?);
                for _ in 0..count {
                    let index = usize::from(reader.read_u8()?);
                    mask[index] = true;
                }
                skeletal_mask = Some(mask);
            }
            19 => {
                // Target boolean flag with no payload and no current canonical
                // consumer. Opcode 18 is intentionally not accepted because
                // the pinned build-241 client has no decode branch for it.
            }
            _ => {
                return Err(reader.unsupported_opcode(
                    "sequence-definition",
                    u32::from(opcode),
                    opcode_offset,
                    1,
                ));
            }
        }
    }

    reader.finish()?;

    let has_blend_structure = interleave.is_some() || skeletal_mask.is_some();
    let precedence_animating = precedence_animating.unwrap_or(if has_blend_structure { 2 } else { 0 });
    let priority = priority.unwrap_or(if has_blend_structure { 2 } else { 0 });

    Ok(SequenceDefinition {
        identity: DefinitionIdentity::new(id, context.target_provenance().clone()),
        frame_ids,
        frame_delays,
        frame_step,
        interleave,
        max_loops,
        precedence_animating,
        priority,
        reply_mode,
        left_hand_item,
        right_hand_item,
        skeletal_animation,
        skeletal_range,
        skeletal_mask,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::{DecodeErrorKind, test_support};

    #[test]
    fn defaults_match_target_constructor_and_postdecode()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        assert!(context.requires_revision_gate(SEQUENCE_LAYOUT_226_PLUS_GATE));
        let source = ArchiveFileProvenance::new(2, 12, Some(1));
        let sequence = decode_sequence_definition(SequenceId::new(1), &[0], &context, &source)?;

        assert_eq!(sequence.identity.id, SequenceId::new(1));
        assert_eq!(sequence.identity.provenance, *context.target_provenance());
        assert!(sequence.frame_ids.is_empty());
        assert!(sequence.frame_delays.is_empty());
        assert_eq!(sequence.frame_step, None);
        assert_eq!(sequence.interleave, None);
        assert_eq!(sequence.max_loops, 99);
        assert_eq!(sequence.precedence_animating, 0);
        assert_eq!(sequence.priority, 0);
        assert_eq!(sequence.reply_mode, 2);
        assert_eq!(sequence.left_hand_item, None);
        assert_eq!(sequence.right_hand_item, None);
        assert_eq!(sequence.skeletal_animation, None);
        assert_eq!(sequence.skeletal_range, None);
        assert_eq!(sequence.skeletal_mask, None);
        Ok(())
    }

    #[test]
    fn frame_layout_combines_high_and_low_halves_without_reordering_delays()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let source = ArchiveFileProvenance::new(2, 12, Some(2));
        let bytes = [
            1, 0, 2, // count
            0, 5, 0, 9, // delays
            0, 0x34, 0, 0x78, // low halves
            0, 0x12, 0, 0x56, // high halves
            2, 0, 2, 0,
        ];
        let sequence = decode_sequence_definition(SequenceId::new(2), &bytes, &context, &source)?;

        assert_eq!(sequence.frame_delays, vec![5, 9]);
        assert_eq!(
            sequence.frame_ids,
            vec![FrameId::new(0x0012_0034), FrameId::new(0x0056_0078)]
        );
        assert_eq!(sequence.frame_step, Some(2));
        Ok(())
    }

    #[test]
    fn interleave_excludes_client_only_sentinel_and_drives_postdecode_defaults()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let source = ArchiveFileProvenance::new(2, 12, Some(3));
        let sequence = decode_sequence_definition(
            SequenceId::new(3),
            &[3, 3, 1, 4, 7, 0],
            &context,
            &source,
        )?;

        assert_eq!(sequence.interleave, Some(vec![1, 4, 7]));
        assert_eq!(sequence.precedence_animating, 2);
        assert_eq!(sequence.priority, 2);
        Ok(())
    }

    #[test]
    fn explicit_priority_fields_override_postdecode_inference()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let source = ArchiveFileProvenance::new(2, 12, Some(4));
        let sequence = decode_sequence_definition(
            SequenceId::new(4),
            &[3, 1, 2, 9, 5, 10, 6, 8, 7, 11, 1, 0],
            &context,
            &source,
        )?;

        assert_eq!(sequence.precedence_animating, 5);
        assert_eq!(sequence.priority, 6);
        assert_eq!(sequence.max_loops, 7);
        assert_eq!(sequence.reply_mode, 1);
        Ok(())
    }

    #[test]
    fn equipment_and_skeletal_inputs_preserve_target_values()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let source = ArchiveFileProvenance::new(2, 12, Some(5));
        let bytes = [
            6, 0x10, 0x37, 7, 0xff, 0xff, 13, 0, 0, 3, 0x84, 15, 0, 10, 0, 20, 17, 3, 0, 7,
            255, 0,
        ];
        let sequence = decode_sequence_definition(SequenceId::new(5), &bytes, &context, &source)?;

        assert_eq!(sequence.left_hand_item, Some(ItemId::new(4151)));
        assert_eq!(sequence.right_hand_item, Some(ItemId::new(65535)));
        assert_eq!(
            sequence.skeletal_animation,
            Some(SkeletalAnimationId::new(900))
        );
        assert_eq!(
            sequence.skeletal_range,
            Some(SequenceRange { start: 10, end: 20 })
        );
        let mask = sequence.skeletal_mask.expect("opcode 17 should create a mask");
        assert_eq!(mask.len(), 256);
        assert!(mask[0]);
        assert!(mask[7]);
        assert!(mask[255]);
        assert_eq!(sequence.precedence_animating, 2);
        assert_eq!(sequence.priority, 2);
        Ok(())
    }

    #[test]
    fn negative_cached_model_id_is_semantic_absence()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let source = ArchiveFileProvenance::new(2, 12, Some(6));
        let sequence = decode_sequence_definition(
            SequenceId::new(6),
            &[13, 0xff, 0xff, 0xff, 0xff, 0],
            &context,
            &source,
        )?;

        assert_eq!(sequence.skeletal_animation, None);
        Ok(())
    }

    #[test]
    fn noncanonical_target_fields_are_consumed_at_exact_widths()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let source = ArchiveFileProvenance::new(2, 12, Some(7));
        let bytes = [
            4, 5, 9, 12, 1, 0, 1, 0, 2, // chat frame id
            14, 0, 1, 0, 3, 0, 4, 5, 6, 7, 8, // event-map entry
            16, 0xfe, 19, 8, 12, 0,
        ];
        let sequence = decode_sequence_definition(SequenceId::new(7), &bytes, &context, &source)?;

        assert_eq!(sequence.max_loops, 12);
        Ok(())
    }

    #[test]
    fn opcode_18_is_not_silently_imported_from_broader_secondary_codec()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let source = ArchiveFileProvenance::new(2, 12, Some(8));
        let error = decode_sequence_definition(
            SequenceId::new(8),
            &[18, b'n', b'a', b'm', b'e', 0, 0],
            &context,
            &source,
        )
        .expect_err("pinned build-241 client has no opcode 18 sequence branch");

        assert_eq!(error.opcode(), Some(18));
        assert_eq!(error.span(), ByteSpan::new(0, 1));
        assert_eq!(
            error.kind(),
            &DecodeErrorKind::UnsupportedOpcode {
                decoder: "sequence-definition",
                opcode: 18,
            }
        );
        Ok(())
    }
}
