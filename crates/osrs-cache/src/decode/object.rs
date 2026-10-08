use super::{ArchiveFileProvenance, BinaryReader, DecodeResult, DecoderContext};
use osrs_core::definitions::{
    DefinitionIdentity, LocType, ModelScale, ModelTranslation, ObjectDefinition, ObjectModels,
    ObjectMorphs, ObjectPlacementFlags, RecolorPair, RetexturePair, TypedObjectModel,
};
use osrs_core::ids::{
    CategoryId, MapIconId, MapSceneId, ModelId, ObjectId, SequenceId, VarbitId, VarpId,
};

const EXTENDED_OBJECT_MODEL_IDS_GATE: &str = "extended_object_model_ids";
const OBJECT_SOUND_LAYOUT_220_PLUS_GATE: &str = "object_sound_layout_220_plus";

/// Decode one canonical object/loc definition for the selected target profile.
///
/// The decoder retains fields owned by [`ObjectDefinition`] and consumes other
/// verified target fields solely to preserve byte alignment. Runtime placement,
/// model selection, morph resolution, sound playback, and post-decode derived
/// values remain owned by later semantic layers.
pub fn decode_object_definition(
    id: ObjectId,
    bytes: &[u8],
    context: &DecoderContext,
    source: &ArchiveFileProvenance,
) -> DecodeResult<ObjectDefinition> {
    let extended_object_model_ids = context.requires_revision_gate(EXTENDED_OBJECT_MODEL_IDS_GATE);
    let object_sound_layout_220_plus =
        context.requires_revision_gate(OBJECT_SOUND_LAYOUT_220_PLUS_GATE);
    let mut reader = BinaryReader::new(bytes, context, source);

    let mut name = None;
    let mut models = None;
    let mut size_x = 1_u16;
    let mut size_y = 1_u16;
    let mut interact_type = 2_u8;
    let mut blocks_projectiles = true;
    let mut clipped = true;
    let mut model_clipped = false;
    let mut obstructs_ground = false;
    let mut solid = false;
    let mut decoration_displacement = 16_u16;
    let mut support_items = None;
    let mut is_rotated = false;
    let mut non_flat_shading = false;
    let mut contour_clip = None;
    let mut animation = None;
    let mut ambient = 0_i16;
    let mut contrast = 0_i16;
    let mut scale = ModelScale::IDENTITY;
    let mut translation = ModelTranslation::ZERO;
    let mut recolors = Vec::new();
    let mut retextures = Vec::new();
    let mut morphs = None;
    let mut map_scene = None;
    let mut map_icon = None;
    let mut category = None;
    let mut actions: [Option<String>; 5] = std::array::from_fn(|_| None);

    loop {
        let opcode_offset = reader.offset();
        let opcode = reader.read_u8()?;
        match opcode {
            0 => break,
            1 => models = decode_typed_models_u16(&mut reader)?,
            2 => name = Some(reader.read_cp1252_string()?),
            5 => models = decode_untyped_models_u16(&mut reader)?,
            6 => {
                require_extended_model_ids(
                    &reader,
                    extended_object_model_ids,
                    opcode,
                    opcode_offset,
                )?;
                models = decode_typed_models_u32(&mut reader)?;
            }
            7 => {
                require_extended_model_ids(
                    &reader,
                    extended_object_model_ids,
                    opcode,
                    opcode_offset,
                )?;
                models = decode_untyped_models_u32(&mut reader)?;
            }
            14 => size_x = u16::from(reader.read_u8()?),
            15 => size_y = u16::from(reader.read_u8()?),
            17 => {
                interact_type = 0;
                blocks_projectiles = false;
            }
            18 => blocks_projectiles = false,
            19 => {
                // Target `int1` is a client interaction hint whose postDecode
                // inference is not part of the current canonical definition.
                reader.skip(1)?;
            }
            21 => contour_clip = Some(0),
            22 => non_flat_shading = true,
            23 => model_clipped = true,
            24 => animation = nullable_sequence_id(reader.read_u16_be()?),
            27 => interact_type = 1,
            28 => decoration_displacement = u16::from(reader.read_u8()?),
            29 => ambient = i16::from(reader.read_i8()?),
            39 => contrast = i16::from(reader.read_i8()?) * 25,
            30..=34 => {
                let action = reader.read_cp1252_string()?;
                actions[usize::from(opcode - 30)] = if action.eq_ignore_ascii_case("Hidden") {
                    None
                } else {
                    Some(action)
                };
            }
            40 => recolors = decode_recolors(&mut reader)?,
            41 => retextures = decode_retextures(&mut reader)?,
            61 => category = Some(CategoryId::new(u32::from(reader.read_u16_be()?))),
            62 => is_rotated = true,
            64 => clipped = false,
            65 => scale.x = reader.read_u16_be()?,
            66 => scale.y = reader.read_u16_be()?,
            67 => scale.z = reader.read_u16_be()?,
            68 => map_scene = Some(MapSceneId::new(u32::from(reader.read_u16_be()?))),
            69 => {
                // Client clip-mask metadata is not currently canonical state.
                reader.skip(1)?;
            }
            70 => translation.x = i32::from(reader.read_i16_be()?),
            71 => translation.y = i32::from(reader.read_i16_be()?),
            72 => translation.z = i32::from(reader.read_i16_be()?),
            73 => obstructs_ground = true,
            74 => solid = true,
            75 => support_items = Some(reader.read_u8()?),
            77 | 92 => morphs = Some(decode_morphs(&mut reader, opcode == 92)?),
            78 => consume_ambient_sound(&mut reader, object_sound_layout_220_plus)?,
            79 => consume_random_sounds(&mut reader, object_sound_layout_220_plus)?,
            81 => contour_clip = Some(u32::from(reader.read_u8()?) * 256),
            82 => map_icon = Some(MapIconId::new(u32::from(reader.read_u16_be()?))),
            89 | 90 | 94 => {
                // Verified target booleans/no-op metadata with no canonical
                // consumer in M2's ObjectDefinition.
            }
            91 => {
                reader.skip(1)?;
            }
            93 => {
                reader.skip(1)?;
                reader.skip(2)?;
                reader.skip(1)?;
                reader.skip(2)?;
            }
            95 => {
                reader.skip(1)?;
            }
            249 => consume_parameters(&mut reader)?,
            _ => {
                return Err(reader.unsupported_opcode(
                    "object-definition",
                    u32::from(opcode),
                    opcode_offset,
                    1,
                ));
            }
        }
    }

    reader.finish()?;
    Ok(ObjectDefinition {
        identity: DefinitionIdentity::new(id, context.target_provenance().clone()),
        name,
        models,
        size_x,
        size_y,
        placement: ObjectPlacementFlags {
            interact_type,
            blocks_projectiles,
            clipped,
            model_clipped,
            obstructs_ground,
            solid,
        },
        decoration_displacement,
        support_items,
        is_rotated,
        non_flat_shading,
        contour_clip,
        animation,
        ambient,
        contrast,
        scale,
        translation,
        recolors,
        retextures,
        morphs,
        map_scene,
        map_icon,
        category,
        actions,
    })
}

fn require_extended_model_ids(
    reader: &BinaryReader<'_>,
    enabled: bool,
    opcode: u8,
    opcode_offset: usize,
) -> DecodeResult<()> {
    if enabled {
        Ok(())
    } else {
        Err(reader.unsupported_opcode(
            "object-definition-extended-model-ids",
            u32::from(opcode),
            opcode_offset,
            1,
        ))
    }
}

fn decode_typed_models_u16(reader: &mut BinaryReader<'_>) -> DecodeResult<Option<ObjectModels>> {
    let count = usize::from(reader.read_u8()?);
    if count == 0 {
        return Ok(None);
    }

    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        entries.push(TypedObjectModel {
            model_id: ModelId::new(u32::from(reader.read_u16_be()?)),
            loc_type: LocType::new(reader.read_u8()?),
        });
    }
    Ok(Some(ObjectModels::Typed(entries)))
}

fn decode_untyped_models_u16(reader: &mut BinaryReader<'_>) -> DecodeResult<Option<ObjectModels>> {
    let count = usize::from(reader.read_u8()?);
    if count == 0 {
        return Ok(None);
    }

    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        entries.push(ModelId::new(u32::from(reader.read_u16_be()?)));
    }
    Ok(Some(ObjectModels::Untyped(entries)))
}

fn decode_typed_models_u32(reader: &mut BinaryReader<'_>) -> DecodeResult<Option<ObjectModels>> {
    let count = usize::from(reader.read_u8()?);
    if count == 0 {
        return Ok(None);
    }

    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        entries.push(TypedObjectModel {
            model_id: ModelId::new(reader.read_u32_be()?),
            loc_type: LocType::new(reader.read_u8()?),
        });
    }
    Ok(Some(ObjectModels::Typed(entries)))
}

fn decode_untyped_models_u32(reader: &mut BinaryReader<'_>) -> DecodeResult<Option<ObjectModels>> {
    let count = usize::from(reader.read_u8()?);
    if count == 0 {
        return Ok(None);
    }

    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        entries.push(ModelId::new(reader.read_u32_be()?));
    }
    Ok(Some(ObjectModels::Untyped(entries)))
}

fn decode_recolors(reader: &mut BinaryReader<'_>) -> DecodeResult<Vec<RecolorPair>> {
    let count = usize::from(reader.read_u8()?);
    let mut pairs = Vec::with_capacity(count);
    for _ in 0..count {
        pairs.push(RecolorPair {
            from: reader.read_u16_be()?,
            to: reader.read_u16_be()?,
        });
    }
    Ok(pairs)
}

fn decode_retextures(reader: &mut BinaryReader<'_>) -> DecodeResult<Vec<RetexturePair>> {
    let count = usize::from(reader.read_u8()?);
    let mut pairs = Vec::with_capacity(count);
    for _ in 0..count {
        pairs.push(RetexturePair {
            from: reader.read_u16_be()?,
            to: reader.read_u16_be()?,
        });
    }
    Ok(pairs)
}

fn decode_morphs(reader: &mut BinaryReader<'_>, explicit_fallback: bool) -> DecodeResult<ObjectMorphs> {
    let transform_varbit = nullable_varbit_id(reader.read_u16_be()?);
    let transform_varp = nullable_varp_id(reader.read_u16_be()?);
    let fallback = if explicit_fallback {
        nullable_object_id(reader.read_u16_be()?)
    } else {
        None
    };

    let count = usize::from(reader.read_u8()?) + 1;
    let mut transforms = Vec::with_capacity(count);
    for _ in 0..count {
        transforms.push(nullable_object_id(reader.read_u16_be()?));
    }

    Ok(ObjectMorphs {
        transform_varbit,
        transform_varp,
        transforms,
        fallback,
    })
}

fn consume_ambient_sound(reader: &mut BinaryReader<'_>, post_220: bool) -> DecodeResult<()> {
    reader.skip(2)?;
    reader.skip(1)?;
    if post_220 {
        reader.skip(1)?;
    }
    Ok(())
}

fn consume_random_sounds(reader: &mut BinaryReader<'_>, post_220: bool) -> DecodeResult<()> {
    reader.skip(2)?;
    reader.skip(2)?;
    reader.skip(1)?;
    if post_220 {
        reader.skip(1)?;
    }
    let count = usize::from(reader.read_u8()?);
    reader.skip(count * 2)?;
    Ok(())
}

fn consume_parameters(reader: &mut BinaryReader<'_>) -> DecodeResult<()> {
    let count = usize::from(reader.read_u8()?);
    for _ in 0..count {
        let is_string = reader.read_u8()? == 1;
        reader.skip(3)?;
        if is_string {
            let _ = reader.read_cp1252_string()?;
        } else {
            reader.skip(4)?;
        }
    }
    Ok(())
}

fn nullable_sequence_id(value: u16) -> Option<SequenceId> {
    (value != u16::MAX).then(|| SequenceId::new(u32::from(value)))
}

fn nullable_varbit_id(value: u16) -> Option<VarbitId> {
    (value != u16::MAX).then(|| VarbitId::new(u32::from(value)))
}

fn nullable_varp_id(value: u16) -> Option<VarpId> {
    (value != u16::MAX).then(|| VarpId::new(u32::from(value)))
}

fn nullable_object_id(value: u16) -> Option<ObjectId> {
    (value != u16::MAX).then(|| ObjectId::new(u32::from(value)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::{ByteSpan, DecodeErrorKind, test_support};

    #[test]
    fn defaults_match_target_constructor_without_postdecode_inference()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let source = ArchiveFileProvenance::new(2, 6, Some(100));
        let definition =
            decode_object_definition(ObjectId::new(100), &[0], &context, &source)?;

        assert_eq!(definition.identity.id, ObjectId::new(100));
        assert_eq!(definition.identity.provenance, *context.target_provenance());
        assert_eq!(definition.name, None);
        assert_eq!(definition.models, None);
        assert_eq!((definition.size_x, definition.size_y), (1, 1));
        assert_eq!(definition.placement.interact_type, 2);
        assert!(definition.placement.blocks_projectiles);
        assert!(definition.placement.clipped);
        assert!(!definition.placement.model_clipped);
        assert!(!definition.placement.obstructs_ground);
        assert!(!definition.placement.solid);
        assert_eq!(definition.decoration_displacement, 16);
        assert_eq!(definition.support_items, None);
        assert_eq!(definition.contour_clip, None);
        assert_eq!(definition.animation, None);
        assert_eq!(definition.scale, ModelScale::IDENTITY);
        assert_eq!(definition.translation, ModelTranslation::ZERO);
        assert_eq!(definition.actions, std::array::from_fn(|_| None));
        Ok(())
    }

    #[test]
    fn classic_typed_and_untyped_model_tables_remain_distinct()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let source = ArchiveFileProvenance::new(2, 6, Some(101));

        let typed = decode_object_definition(
            ObjectId::new(101),
            &[1, 2, 0x12, 0x34, 10, 0xab, 0xcd, 4, 0],
            &context,
            &source,
        )?;
        assert_eq!(
            typed.models,
            Some(ObjectModels::Typed(vec![
                TypedObjectModel {
                    model_id: ModelId::new(0x1234),
                    loc_type: LocType::new(10),
                },
                TypedObjectModel {
                    model_id: ModelId::new(0xabcd),
                    loc_type: LocType::new(4),
                },
            ]))
        );

        let untyped = decode_object_definition(
            ObjectId::new(102),
            &[5, 2, 0x12, 0x34, 0xab, 0xcd, 0],
            &context,
            &source,
        )?;
        assert_eq!(
            untyped.models,
            Some(ObjectModels::Untyped(vec![
                ModelId::new(0x1234),
                ModelId::new(0xabcd),
            ]))
        );
        Ok(())
    }

    #[test]
    fn extended_model_opcodes_preserve_full_u32_identity()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        assert!(context.requires_revision_gate(EXTENDED_OBJECT_MODEL_IDS_GATE));
        let source = ArchiveFileProvenance::new(2, 6, Some(103));

        let typed = decode_object_definition(
            ObjectId::new(103),
            &[6, 1, 0x12, 0x34, 0x56, 0x78, 22, 0],
            &context,
            &source,
        )?;
        assert_eq!(
            typed.models,
            Some(ObjectModels::Typed(vec![TypedObjectModel {
                model_id: ModelId::new(0x1234_5678),
                loc_type: LocType::new(22),
            }]))
        );

        let untyped = decode_object_definition(
            ObjectId::new(104),
            &[7, 1, 0xfe, 0xdc, 0xba, 0x98, 0],
            &context,
            &source,
        )?;
        assert_eq!(
            untyped.models,
            Some(ObjectModels::Untyped(vec![ModelId::new(0xfedc_ba98)]))
        );
        Ok(())
    }

    #[test]
    fn placement_and_model_build_fields_preserve_target_semantics()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let source = ArchiveFileProvenance::new(2, 6, Some(105));
        let bytes = [
            2, b'D', b'o', b'o', b'r', 0, 14, 2, 15, 3, 17, 22, 23, 28, 24, 29, 0xfe, 39,
            0xfe, 62, 64, 65, 0, 140, 66, 0, 141, 67, 0, 142, 70, 0xff, 0xf6, 71, 0, 20,
            72, 0xff, 0xe2, 73, 74, 75, 2, 21, 24, 0xff, 0xff, 0,
        ];
        let definition =
            decode_object_definition(ObjectId::new(105), &bytes, &context, &source)?;

        assert_eq!(definition.name.as_deref(), Some("Door"));
        assert_eq!((definition.size_x, definition.size_y), (2, 3));
        assert_eq!(definition.placement.interact_type, 0);
        assert!(!definition.placement.blocks_projectiles);
        assert!(!definition.placement.clipped);
        assert!(definition.placement.model_clipped);
        assert!(definition.placement.obstructs_ground);
        assert!(definition.placement.solid);
        assert_eq!(definition.decoration_displacement, 24);
        assert_eq!(definition.support_items, Some(2));
        assert!(definition.is_rotated);
        assert!(definition.non_flat_shading);
        assert_eq!(definition.contour_clip, Some(0));
        assert_eq!(definition.animation, None);
        assert_eq!(definition.ambient, -2);
        assert_eq!(definition.contrast, -50);
        assert_eq!(
            definition.scale,
            ModelScale {
                x: 140,
                y: 141,
                z: 142,
            }
        );
        assert_eq!(
            definition.translation,
            ModelTranslation {
                x: -10,
                y: 20,
                z: -30,
            }
        );
        Ok(())
    }

    #[test]
    fn actions_colors_textures_and_editor_metadata_decode_without_loss()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let source = ArchiveFileProvenance::new(2, 6, Some(106));
        let bytes = [
            30, b'O', b'p', b'e', b'n', 0, 31, b'h', b'I', b'D', b'D', b'E', b'N', 0, 40, 1,
            0, 1, 0, 2, 41, 1, 0, 3, 0, 4, 61, 0, 5, 68, 0, 6, 82, 0, 7, 0,
        ];
        let definition =
            decode_object_definition(ObjectId::new(106), &bytes, &context, &source)?;

        assert_eq!(definition.actions[0].as_deref(), Some("Open"));
        assert_eq!(definition.actions[1], None);
        assert_eq!(definition.recolors, vec![RecolorPair { from: 1, to: 2 }]);
        assert_eq!(definition.retextures, vec![RetexturePair { from: 3, to: 4 }]);
        assert_eq!(definition.category, Some(CategoryId::new(5)));
        assert_eq!(definition.map_scene, Some(MapSceneId::new(6)));
        assert_eq!(definition.map_icon, Some(MapIconId::new(7)));
        Ok(())
    }

    #[test]
    fn morph_opcodes_preserve_null_entries_and_explicit_fallback()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let source = ArchiveFileProvenance::new(2, 6, Some(107));

        let opcode_77 = decode_object_definition(
            ObjectId::new(107),
            &[77, 0, 7, 0, 8, 1, 0, 100, 0xff, 0xff, 0],
            &context,
            &source,
        )?;
        assert_eq!(
            opcode_77.morphs,
            Some(ObjectMorphs {
                transform_varbit: Some(VarbitId::new(7)),
                transform_varp: Some(VarpId::new(8)),
                transforms: vec![Some(ObjectId::new(100)), None],
                fallback: None,
            })
        );

        let opcode_92 = decode_object_definition(
            ObjectId::new(108),
            &[92, 0xff, 0xff, 0, 9, 0, 200, 0, 0xff, 0xff, 0],
            &context,
            &source,
        )?;
        assert_eq!(
            opcode_92.morphs,
            Some(ObjectMorphs {
                transform_varbit: None,
                transform_varp: Some(VarpId::new(9)),
                transforms: vec![None],
                fallback: Some(ObjectId::new(200)),
            })
        );
        Ok(())
    }

    #[test]
    fn target_only_noncanonical_fields_are_consumed_with_exact_layouts()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        assert!(context.requires_revision_gate(OBJECT_SOUND_LAYOUT_220_PLUS_GATE));
        let source = ArchiveFileProvenance::new(2, 6, Some(109));
        let bytes = [
            19, 3, 69, 4, 78, 0, 10, 20, 30, 79, 0, 1, 0, 2, 3, 4, 2, 0, 11, 0, 12, 89,
            90, 91, 5, 93, 6, 0, 7, 8, 0, 9, 94, 95, 10, 249, 2, 1, 0, 0, 1, b'x',
            0, 0, 0, 0, 2, 0, 0, 0, 42, 14, 4, 0,
        ];
        let definition =
            decode_object_definition(ObjectId::new(109), &bytes, &context, &source)?;

        assert_eq!(definition.size_x, 4);
        Ok(())
    }

    #[test]
    fn unknown_opcode_is_typed_and_contextual() -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let source = ArchiveFileProvenance::new(2, 6, Some(999));
        let error = match decode_object_definition(
            ObjectId::new(999),
            &[250, 0],
            &context,
            &source,
        ) {
            Err(error) => error,
            Ok(_) => return Err("unknown object opcode unexpectedly decoded".into()),
        };

        assert_eq!(error.opcode(), Some(250));
        assert_eq!(error.span(), ByteSpan::new(0, 1));
        assert_eq!(error.source_provenance(), &source);
        assert_eq!(
            error.kind(),
            &DecodeErrorKind::UnsupportedOpcode {
                decoder: "object-definition",
                opcode: 250,
            }
        );
        Ok(())
    }
}
