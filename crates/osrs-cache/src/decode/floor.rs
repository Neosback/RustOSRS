use super::{
    ArchiveFileProvenance, BinaryReader, ByteSpan, DecodeResult, DecodeSubject, DecoderContext,
};
use osrs_core::definitions::{
    DefinitionIdentity, FloorOverlayDefinition, FloorUnderlayDefinition, Rgb24,
};
use osrs_core::floor_color::{OverlayHsl, UnderlayHsl};
use osrs_core::ids::{FloorOverlayId, FloorUnderlayId, TextureId};

/// Decode a floor-underlay definition using the build-241-compatible opcode
/// contract verified against the pinned primary source.
pub fn decode_floor_underlay(
    id: FloorUnderlayId,
    bytes: &[u8],
    context: &DecoderContext,
    source: &ArchiveFileProvenance,
) -> DecodeResult<FloorUnderlayDefinition> {
    let mut reader = BinaryReader::new(bytes, context, source)
        .with_subject(DecodeSubject::FloorUnderlay(id.get()));
    let mut rgb = 0_u32;

    loop {
        let opcode_offset = reader.offset();
        let opcode = reader.read_u8()?;
        match opcode {
            0 => break,
            1 => rgb = reader.read_u24_be()?,
            _ => {
                return Err(reader.unsupported_opcode(
                    "floor-underlay-definition",
                    u32::from(opcode),
                    opcode_offset,
                    1,
                ));
            }
        }
    }

    let rgb = rgb24(&reader, "underlay rgb", rgb, None)?;
    reader.finish()?;
    Ok(FloorUnderlayDefinition {
        identity: DefinitionIdentity::new(id, context.target_provenance().clone()),
        rgb,
        hsl: UnderlayHsl::from_rgb(rgb),
    })
}

/// Decode a floor-overlay definition using the pinned primary target contract.
///
/// Opcode 8 is a target no-op. Opcode 9 consumes one unsigned byte in the
/// pinned source, but that byte is not retained as canonical overlay state.
pub fn decode_floor_overlay(
    id: FloorOverlayId,
    bytes: &[u8],
    context: &DecoderContext,
    source: &ArchiveFileProvenance,
) -> DecodeResult<FloorOverlayDefinition> {
    let mut reader = BinaryReader::new(bytes, context, source)
        .with_subject(DecodeSubject::FloorOverlay(id.get()));
    let mut primary_rgb = 0_u32;
    let mut texture = None;
    let mut hide_underlay = true;
    let mut secondary_rgb = None;

    loop {
        let opcode_offset = reader.offset();
        let opcode = reader.read_u8()?;
        match opcode {
            0 => break,
            1 => primary_rgb = reader.read_u24_be()?,
            2 => texture = Some(TextureId::new(u32::from(reader.read_u8()?))),
            5 => hide_underlay = false,
            7 => secondary_rgb = Some(reader.read_u24_be()?),
            8 => {}
            9 => {
                reader.skip(1)?;
            }
            _ => {
                return Err(reader.unsupported_opcode(
                    "floor-overlay-definition",
                    u32::from(opcode),
                    opcode_offset,
                    1,
                ));
            }
        }
    }

    let primary_rgb = rgb24(&reader, "overlay primary rgb", primary_rgb, Some(1))?;
    let secondary_rgb = secondary_rgb
        .map(|value| rgb24(&reader, "overlay secondary rgb", value, Some(7)))
        .transpose()?;
    reader.finish()?;

    // Match the target postDecode ordering: secondary is derived first and
    // copied aside, then primary is derived and becomes the active HSL state.
    let secondary_hsl = secondary_rgb.map(OverlayHsl::from_rgb);
    let primary_hsl = OverlayHsl::from_rgb(primary_rgb);

    Ok(FloorOverlayDefinition {
        identity: DefinitionIdentity::new(id, context.target_provenance().clone()),
        primary_rgb,
        texture,
        hide_underlay,
        secondary_rgb,
        primary_hsl,
        secondary_hsl,
    })
}

fn rgb24(
    reader: &BinaryReader<'_>,
    field: &'static str,
    value: u32,
    opcode: Option<u32>,
) -> DecodeResult<Rgb24> {
    match Rgb24::new(value) {
        Some(rgb) => Ok(rgb),
        None => Err(reader.invalid_value(
            field,
            format!("value {value:#x} exceeds 24-bit RGB width"),
            ByteSpan::new(reader.offset(), 0),
            opcode,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::{DecodeErrorKind, test_support};

    #[test]
    fn underlay_preserves_rgb_postdecode_hsl_and_target_provenance()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let source = ArchiveFileProvenance::new(2, 1, Some(42));
        let definition = decode_floor_underlay(
            FloorUnderlayId::new(42),
            &[1, 0x12, 0x34, 0x56, 0],
            &context,
            &source,
        )?;

        assert_eq!(definition.identity.id, FloorUnderlayId::new(42));
        assert_eq!(&definition.identity.provenance, context.target_provenance());
        assert_eq!(definition.rgb.get(), 0x12_34_56);
        assert_eq!(
            definition.hsl,
            UnderlayHsl {
                weighted_hue: 39,
                saturation: 167,
                lightness: 52,
                hue_multiplier: 68,
            }
        );
        Ok(())
    }

    #[test]
    fn underlay_default_rgb_has_minimum_hue_multiplier() -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let source = ArchiveFileProvenance::new(2, 1, Some(7));
        let definition = decode_floor_underlay(FloorUnderlayId::new(7), &[0], &context, &source)?;

        assert_eq!(definition.rgb.get(), 0);
        assert_eq!(definition.hsl.hue_multiplier, 1);
        assert_eq!(definition.hsl.saturation, 0);
        assert_eq!(definition.hsl.lightness, 0);
        Ok(())
    }

    #[test]
    fn overlay_decodes_target_opcodes_and_preserves_postdecode_order()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let source = ArchiveFileProvenance::new(2, 4, Some(11));
        let bytes = [
            1, 0x12, 0x34, 0x56, 2, 7, 5, 7, 0xab, 0xcd, 0xef, 8, 9, 0x42, 0,
        ];
        let definition = decode_floor_overlay(FloorOverlayId::new(11), &bytes, &context, &source)?;

        assert_eq!(definition.identity.id, FloorOverlayId::new(11));
        assert_eq!(definition.primary_rgb.get(), 0x12_34_56);
        assert_eq!(definition.texture, Some(TextureId::new(7)));
        assert!(!definition.hide_underlay);
        assert_eq!(definition.secondary_rgb.map(Rgb24::get), Some(0xab_cd_ef));
        assert_eq!(
            definition.primary_hsl,
            OverlayHsl {
                hue: 149,
                saturation: 167,
                lightness: 52,
            }
        );
        assert_eq!(
            definition.secondary_hsl,
            Some(OverlayHsl {
                hue: 149,
                saturation: 170,
                lightness: 205,
            })
        );
        Ok(())
    }

    #[test]
    fn overlay_defaults_match_target_constructor_and_postdecode_semantics()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let source = ArchiveFileProvenance::new(2, 4, Some(12));
        let definition = decode_floor_overlay(FloorOverlayId::new(12), &[0], &context, &source)?;

        assert_eq!(definition.primary_rgb.get(), 0);
        assert_eq!(definition.texture, None);
        assert!(definition.hide_underlay);
        assert_eq!(definition.secondary_rgb, None);
        assert_eq!(
            definition.primary_hsl,
            OverlayHsl::from_rgb(definition.primary_rgb)
        );
        assert_eq!(definition.secondary_hsl, None);
        Ok(())
    }

    #[test]
    fn overlay_opcode_nine_consumes_one_nonsemantic_byte() -> Result<(), Box<dyn std::error::Error>>
    {
        let context = test_support::target_context()?;
        let source = ArchiveFileProvenance::new(2, 4, Some(13));
        let definition = decode_floor_overlay(
            FloorOverlayId::new(13),
            &[9, 0xaa, 1, 0x01, 0x02, 0x03, 0],
            &context,
            &source,
        )?;

        assert_eq!(definition.primary_rgb.get(), 0x01_02_03);
        assert_eq!(definition.texture, None);
        Ok(())
    }

    #[test]
    fn unknown_floor_opcode_is_typed_and_contextual() -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let source = ArchiveFileProvenance::new(2, 4, Some(99));
        let error = match decode_floor_overlay(FloorOverlayId::new(99), &[99, 0], &context, &source)
        {
            Err(error) => error,
            Ok(_) => return Err("unknown overlay opcode unexpectedly decoded".into()),
        };

        assert_eq!(error.opcode(), Some(99));
        assert_eq!(error.span(), ByteSpan::new(0, 1));
        assert_eq!(error.source_provenance(), &source);
        assert_eq!(
            error.kind(),
            &DecodeErrorKind::UnsupportedOpcode {
                decoder: "floor-overlay-definition",
                opcode: 99,
            }
        );
        Ok(())
    }
}
