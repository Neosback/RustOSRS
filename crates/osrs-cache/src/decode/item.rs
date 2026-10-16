//! Item config decoding, for the parts that decide how an item looks when worn. Every opcode of
//! the installed revision-240 cache is read (and most are discarded) so a stream must end exactly
//! where it should; layouts follow OpenRune's `ItemCodec` (including its 241 additions). Unknown opcodes fail with provenance.
use super::{ArchiveFileProvenance, BinaryReader, DecodeResult, DecodeSubject, DecoderContext};
use osrs_core::{
    definitions::{DefinitionIdentity, RecolorPair, RetexturePair},
    ids::{ItemId, ModelId},
    player::{ItemDefinition, WearModels},
};

fn narrow(value: u16) -> Option<ModelId> {
    (value != u16::MAX).then(|| ModelId::new(u32::from(value)))
}

fn wide(value: u32) -> Option<ModelId> {
    (value != u32::MAX && value != u32::from(u16::MAX)).then(|| ModelId::new(value))
}

/// Decode one item, requiring complete byte consumption.
pub fn decode_item_definition(
    id: ItemId,
    bytes: &[u8],
    context: &DecoderContext,
    source: &ArchiveFileProvenance,
) -> DecodeResult<ItemDefinition> {
    let mut r =
        BinaryReader::new(bytes, context, source).with_subject(DecodeSubject::Item(id.get()));
    let mut item = ItemDefinition {
        identity: DefinitionIdentity::new(id, context.target_provenance().clone()),
        name: None,
        equip_slot: None,
        hides: [None; 2],
        male: WearModels::default(),
        female: WearModels::default(),
        recolors: Vec::new(),
        retextures: Vec::new(),
    };
    loop {
        let offset = r.offset();
        let op = r.read_u8()?;
        match op {
            0 => break,
            // Plain flags with no payload (stackable, members, untradeable, never stacks, tradeable).
            11 | 15 | 16 | 65 | 160 | 251 => {}
            2 => item.name = Some(r.read_cp1252_string()?),
            3 | 30..=39 => {
                r.read_cp1252_string()?;
            }
            // Two-byte fields: inventory model, zoom, rotations, offsets, weight, category,
            // models for the inventory icon, note links, resizes, ids.
            1 | 4..=8 | 75 | 94 | 95 | 97 | 98 | 99 | 110..=112 | 139 | 140 | 148 | 149 => {
                r.read_u16_be()?;
            }
            12 => {
                r.read_i32_be()?;
            }
            // Models of the item that are shown only during certain animations.
            161 => {
                for _ in 0..r.read_u16_be()? {
                    r.read_u16_be()?;
                }
            }
            13 => item.equip_slot = Some(r.read_u8()?),
            14 => item.hides[0] = Some(r.read_u8()?),
            27 => item.hides[1] = u8::try_from(r.read_i8()?).ok(),
            23 => {
                item.male.models[0] = narrow(r.read_u16_be()?);
                item.male.offset = r.read_u8()?;
            }
            24 => item.male.models[1] = narrow(r.read_u16_be()?),
            78 => item.male.models[2] = narrow(r.read_u16_be()?),
            25 => {
                item.female.models[0] = narrow(r.read_u16_be()?);
                item.female.offset = r.read_u8()?;
            }
            26 => item.female.models[1] = narrow(r.read_u16_be()?),
            79 => item.female.models[2] = narrow(r.read_u16_be()?),
            44 | 51..=54 => {
                r.read_u32_be()?;
            }
            45 => {
                item.male.models[0] = wide(r.read_u32_be()?);
                item.male.offset = r.read_u8()?;
            }
            46 => item.male.models[1] = wide(r.read_u32_be()?),
            47 => item.male.models[2] = wide(r.read_u32_be()?),
            48 => {
                item.female.models[0] = wide(r.read_u32_be()?);
                item.female.offset = r.read_u8()?;
            }
            49 => item.female.models[1] = wide(r.read_u32_be()?),
            50 => item.female.models[2] = wide(r.read_u32_be()?),
            // Wear models for chat heads (u16 forms).
            90..=93 => {
                r.read_u16_be()?;
            }
            40 => {
                item.recolors.clear();
                for _ in 0..r.read_u8()? {
                    item.recolors.push(RecolorPair {
                        from: r.read_u16_be()?,
                        to: r.read_u16_be()?,
                    });
                }
            }
            41 => {
                item.retextures.clear();
                for _ in 0..r.read_u8()? {
                    item.retextures.push(RetexturePair {
                        from: r.read_u16_be()?,
                        to: r.read_u16_be()?,
                    });
                }
            }
            42 | 113..=115 => {
                r.read_u8()?;
            }
            43 => {
                r.read_u8()?;
                while r.read_u8()? != 0 {
                    r.read_cp1252_string()?;
                }
            }
            100..=109 => {
                r.read_u16_be()?;
                r.read_u16_be()?;
            }
            200 => {
                r.read_u8()?;
                while r.read_u8()? != 0 {
                    r.read_cp1252_string()?;
                }
            }
            201 => {
                r.read_u8()?;
                r.read_u16_be()?;
                r.read_u16_be()?;
                r.read_i32_be()?;
                r.read_i32_be()?;
                r.read_cp1252_string()?;
            }
            202 => {
                r.read_u8()?;
                for _ in 0..3 {
                    r.read_u16_be()?;
                }
                r.read_i32_be()?;
                r.read_i32_be()?;
                r.read_cp1252_string()?;
            }
            249 => {
                for _ in 0..r.read_u8()? {
                    let kind = r.read_u8()?;
                    r.read_u24_be()?;
                    match kind {
                        1 => {
                            r.read_cp1252_string()?;
                        }
                        2 => r.skip(8)?,
                        _ => {
                            r.read_i32_be()?;
                        }
                    }
                }
            }
            _ => return Err(r.unsupported_opcode("item", u32::from(op), offset, 1)),
        }
    }
    r.finish()?;
    Ok(item)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::test_support::{source, target_context};

    fn decode(bytes: &[u8]) -> DecodeResult<ItemDefinition> {
        let context = target_context().unwrap_or_else(|e| unreachable!("{e:?}"));
        decode_item_definition(ItemId::new(4151), bytes, &context, &source())
    }

    #[test]
    fn decodes_worn_models_slot_and_hides() {
        let mut b = vec![2];
        b.extend(b"Whip\0");
        b.extend([13, 3, 14, 6, 27, 0xff]); // slot 3, hides slot 6, second hide none
        b.extend([23, 0, 20, 4, 25, 0, 21, 0]); // male model 20 (offset 4), female 21
        b.push(0);
        let item = decode(&b).unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(item.name.as_deref(), Some("Whip"));
        assert_eq!(item.equip_slot, Some(3));
        assert_eq!(item.hides, [Some(6), None]);
        assert_eq!(item.male.models[0], Some(ModelId::new(20)));
        assert_eq!(item.male.offset, 4);
        assert_eq!(item.female.models[0], Some(ModelId::new(21)));
    }

    #[test]
    fn truncated_unknown_and_trailing_input_fail() {
        assert!(decode(&[13]).is_err());
        assert!(decode(&[2, b'a']).is_err());
        assert!(decode(&[250, 0]).is_err());
        assert!(decode(&[0, 1]).is_err());
    }
}
