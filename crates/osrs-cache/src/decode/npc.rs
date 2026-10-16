//! NPC config decoding. Layouts are checked against the installed OpenRune cache;
//! unsupported fields fail with archive/definition/offset provenance.
use super::{
    ArchiveFileProvenance, BinaryReader, ByteSpan, DecodeResult, DecodeSubject, DecoderContext,
};
use osrs_core::{
    definitions::{DefinitionIdentity, RecolorPair, RetexturePair},
    entity::{EntityScale, NpcDefinition, NpcMorphs, NpcMovementSequences, NpcRenderFlags},
    ids::{ModelId, NpcId, SequenceId, VarbitId, VarpId},
};

/// Decode a supported modern client NPC config, requiring complete byte consumption.
pub fn decode_npc_definition(
    id: NpcId,
    bytes: &[u8],
    context: &DecoderContext,
    source: &ArchiveFileProvenance,
) -> DecodeResult<NpcDefinition> {
    let mut r = BinaryReader::new(bytes, context, source)
        .with_subject(DecodeSubject::NpcDefinition(id.get()));
    let mut d = NpcDefinition {
        identity: DefinitionIdentity::new(id, context.target_provenance().clone()),
        name: None,
        body_models: Vec::new(),
        head_models: Vec::new(),
        size: 1,
        movement_sequences: NpcMovementSequences::default(),
        recolors: Vec::new(),
        retextures: Vec::new(),
        combat_level: None,
        scale: EntityScale::IDENTITY,
        ambient: 0,
        contrast: 0,
        morphs: None,
        render: NpcRenderFlags::default(),
        actions: std::array::from_fn(|_| None),
        full_recolor: None,
    };
    loop {
        let offset = r.offset();
        let op = r.read_u8()?;
        match op {
            0 => break,
            1 | 60 | 61 | 62 => {
                let n = r.read_u8()?;
                let target = if op == 1 || op == 61 {
                    &mut d.body_models
                } else {
                    &mut d.head_models
                };
                target.clear();
                for _ in 0..n {
                    let value = if op >= 61 {
                        r.read_u32_be()?
                    } else {
                        u32::from(r.read_u16_be()?)
                    };
                    // Null models are not usable sources; preserve their absence.
                    if value != if op >= 61 { u32::MAX } else { 65535 } {
                        target.push(ModelId::new(value));
                    }
                }
            }
            2 => d.name = Some(r.read_cp1252_string()?),
            12 => {
                d.size = u16::from(r.read_u8()?);
                if d.size == 0 {
                    return Err(r.invalid_value(
                        "npc size",
                        "must be positive",
                        ByteSpan::new(offset, 2),
                        Some(u32::from(op)),
                    ));
                }
            }
            13 => d.movement_sequences.idle = sequence(&mut r)?,
            14 => d.movement_sequences.walk = sequence(&mut r)?,
            15 => d.movement_sequences.turn_left = sequence(&mut r)?,
            16 => d.movement_sequences.turn_right = sequence(&mut r)?,
            17 => {
                d.movement_sequences.walk = sequence(&mut r)?;
                d.movement_sequences.walk_back = sequence(&mut r)?;
                d.movement_sequences.walk_left = sequence(&mut r)?;
                d.movement_sequences.walk_right = sequence(&mut r)?;
            }
            18 | 74..=79 => {
                r.read_u16_be()?;
            }
            30..=34 => {
                let text = r.read_cp1252_string()?;
                d.actions[usize::from(op - 30)] =
                    (!text.eq_ignore_ascii_case("hidden")).then_some(text);
            }
            40 => {
                for _ in 0..r.read_u8()? {
                    d.recolors.push(RecolorPair {
                        from: r.read_u16_be()?,
                        to: r.read_u16_be()?,
                    });
                }
            }
            41 => {
                for _ in 0..r.read_u8()? {
                    d.retextures.push(RetexturePair {
                        from: r.read_u16_be()?,
                        to: r.read_u16_be()?,
                    });
                }
            }
            42 => d.full_recolor = Some(r.read_u16_be()?),
            93 => d.render.minimap_visible = false,
            95 => d.combat_level = nullable(r.read_u16_be()?),
            97 => d.scale.horizontal = r.read_u16_be()?,
            98 => d.scale.vertical = r.read_u16_be()?,
            99 => d.render.priority = 1,
            100 => d.ambient = i16::from(r.read_i8()?),
            101 => d.contrast = i16::from(r.read_i8()?) * 5,
            102 => {
                let bits = r.read_u8()?;
                for i in 0..8 {
                    if bits & (1 << i) != 0 {
                        // Nullable large-smart archive id followed by short-smart-minus-one index.
                        if r.peek_u8()? >= 128 {
                            r.read_u32_be()?;
                        } else {
                            r.read_u16_be()?;
                        }
                        r.read_unsigned_short_smart()?;
                    }
                }
            }
            103 => d.render.rotation_speed = r.read_u16_be()?,
            106 | 118 => {
                let transform_varbit =
                    nullable(r.read_u16_be()?).map(|v| VarbitId::new(u32::from(v)));
                let transform_varp = nullable(r.read_u16_be()?).map(|v| VarpId::new(u32::from(v)));
                let fallback = if op == 118 {
                    nullable(r.read_u16_be()?).map(|v| NpcId::new(u32::from(v)))
                } else {
                    None
                };
                let n = usize::from(r.read_u8()?) + 1;
                let mut transforms = Vec::with_capacity(n);
                for _ in 0..n {
                    transforms.push(nullable(r.read_u16_be()?).map(|v| NpcId::new(u32::from(v))));
                }
                d.morphs = Some(NpcMorphs {
                    transform_varbit,
                    transform_varp,
                    transforms,
                    fallback,
                });
            }
            107 => d.render.interactable = false,
            109 => d.render.clickable = false,
            111 => d.render.priority = 2,
            114 => d.movement_sequences.run[0] = sequence(&mut r)?,
            115 => {
                for entry in &mut d.movement_sequences.run {
                    *entry = sequence(&mut r)?;
                }
            }
            116 => d.movement_sequences.crawl[0] = sequence(&mut r)?,
            117 => {
                for entry in &mut d.movement_sequences.crawl {
                    *entry = sequence(&mut r)?;
                }
            }
            122 => d.render.follower = true,
            123 => d.render.low_priority_follower_ops = true,
            124 => d.render.height = Some(r.read_u16_be()?),
            126 => d.render.footprint_size = Some(r.read_u16_be()?),
            129 | 130 => d.render.ready_animation_during_action = true,
            145 => d.render.hide_for_overlap = true,
            146 => d.render.overlap_tint = Some(r.read_u16_be()?),
            147 => d.render.depth_buffer = false,
            148 => r.skip(4)?,
            149 | 151 => r.skip(1)?,
            150 => r.skip(6)?,
            152 => {
                r.skip(6)?;
                let n = usize::from(r.read_u8()?);
                r.skip(n * 2)?;
            }
            249 => {
                for _ in 0..r.read_u8()? {
                    let string = r.read_u8()? == 1;
                    r.read_u24_be()?;
                    if string {
                        r.read_cp1252_string()?;
                    } else {
                        r.read_i32_be()?;
                    }
                }
            }
            252 => {
                r.skip(13)?;
                r.read_cp1252_string()?;
            }
            _ => return Err(r.unsupported_opcode("npc-definition", u32::from(op), offset, 1)),
        }
    }
    r.finish()?;
    Ok(d)
}

fn nullable(value: u16) -> Option<u16> {
    (value != u16::MAX).then_some(value)
}
fn sequence(r: &mut BinaryReader<'_>) -> DecodeResult<Option<SequenceId>> {
    Ok(nullable(r.read_u16_be()?).map(|v| SequenceId::new(u32::from(v))))
}
