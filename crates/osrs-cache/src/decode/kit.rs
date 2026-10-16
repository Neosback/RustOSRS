//! Identity-kit (player body part) config decoding, checked against the installed OpenRune
//! cache. Layouts follow OpenRune's `IdentityKitCodec`; unknown opcodes fail with provenance.
use super::{ArchiveFileProvenance, BinaryReader, DecodeResult, DecodeSubject, DecoderContext};
use osrs_core::{
    definitions::{DefinitionIdentity, RecolorPair, RetexturePair},
    ids::{KitId, ModelId},
    player::KitDefinition,
};

/// Decode one kit, requiring complete byte consumption.
pub fn decode_kit_definition(
    id: KitId,
    bytes: &[u8],
    context: &DecoderContext,
    source: &ArchiveFileProvenance,
) -> DecodeResult<KitDefinition> {
    let mut r =
        BinaryReader::new(bytes, context, source).with_subject(DecodeSubject::Kit(id.get()));
    let mut kit = KitDefinition {
        identity: DefinitionIdentity::new(id, context.target_provenance().clone()),
        body_part: None,
        models: Vec::new(),
        non_selectable: false,
        recolors: Vec::new(),
        retextures: Vec::new(),
    };
    loop {
        let offset = r.offset();
        let op = r.read_u8()?;
        match op {
            0 => break,
            1 => kit.body_part = Some(r.read_u8()?),
            2 | 5 => {
                let count = r.read_u8()?;
                kit.models.clear();
                for _ in 0..count {
                    let value = if op == 5 {
                        r.read_u32_be()?
                    } else {
                        u32::from(r.read_u16_be()?)
                    };
                    // 65535 is the u16 "none"; the wide form uses the same sentinel.
                    if value != 65_535 && value != u32::MAX {
                        kit.models.push(ModelId::new(value));
                    }
                }
            }
            3 => kit.non_selectable = true,
            40 => {
                kit.recolors.clear();
                for _ in 0..r.read_u8()? {
                    kit.recolors.push(RecolorPair {
                        from: r.read_u16_be()?,
                        to: r.read_u16_be()?,
                    });
                }
            }
            41 => {
                kit.retextures.clear();
                for _ in 0..r.read_u8()? {
                    kit.retextures.push(RetexturePair {
                        from: r.read_u16_be()?,
                        to: r.read_u16_be()?,
                    });
                }
            }
            // Chat-head models (u16, then the wide u32 form): the editor draws no chat heads.
            60..=69 => {
                r.read_u16_be()?;
            }
            70..=79 => {
                r.read_u32_be()?;
            }
            _ => return Err(r.unsupported_opcode("kit", u32::from(op), offset, 1)),
        }
    }
    r.finish()?;
    Ok(kit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::test_support::{source, target_context};

    fn decode(bytes: &[u8]) -> DecodeResult<KitDefinition> {
        let context = target_context().unwrap_or_else(|e| unreachable!("{e:?}"));
        decode_kit_definition(KitId::new(3), bytes, &context, &source())
    }

    #[test]
    fn decodes_part_models_and_recolors() {
        let kit = decode(&[
            1, 4, // body part
            2, 3, 0, 10, 0xff, 0xff, 1, 0x2c, // three u16 models, one is the none sentinel
            40, 1, 0, 5, 0, 9, // one recolor
            60, 0, 7, // chat-head model, ignored
            3, 0,
        ])
        .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(kit.body_part, Some(4));
        assert_eq!(kit.models, vec![ModelId::new(10), ModelId::new(300)]);
        assert_eq!(kit.recolors, vec![RecolorPair { from: 5, to: 9 }]);
        assert!(kit.non_selectable);
    }

    #[test]
    fn truncated_and_unknown_and_trailing_input_fail() {
        assert!(decode(&[1]).is_err());
        assert!(decode(&[2, 1, 0]).is_err());
        assert!(decode(&[200, 0]).is_err());
        assert!(decode(&[0, 7]).is_err());
    }
}
