use super::{ArchiveFileProvenance, BinaryReader, ByteSpan, DecodeResult, DecoderContext};
use osrs_core::definitions::{DefinitionIdentity, TextureDefinition};
use osrs_core::ids::{SpriteId, TextureId};

const TEXTURE_LAYOUT_233_PLUS_GATE: &str = "texture_layout_233_plus";

/// Decode the build-241 texture definition layout.
///
/// Revision 233+ stores exactly one sprite/file id followed by average RGB,
/// a transparency flag, and animation metadata. Historical multi-sprite
/// combine arrays do not exist in this target layout and remain empty.
pub fn decode_texture_definition(
    id: TextureId,
    bytes: &[u8],
    context: &DecoderContext,
    source: &ArchiveFileProvenance,
) -> DecodeResult<TextureDefinition> {
    let mut reader = BinaryReader::new(bytes, context, source);
    if !context.requires_revision_gate(TEXTURE_LAYOUT_233_PLUS_GATE) {
        return Err(reader.invalid_value(
            "texture layout",
            "target profile does not require texture_layout_233_plus",
            ByteSpan::new(0, 0),
            None,
        ));
    }

    let file_id = reader.read_u16_be()?;
    let average_rgb = reader.read_u16_be()?;
    let is_transparent = reader.read_u8()? == 1;
    let animation_direction = reader.read_u8()?;
    let animation_speed = reader.read_u8()?;
    reader.finish()?;

    Ok(TextureDefinition {
        identity: DefinitionIdentity::new(id, context.target_provenance().clone()),
        average_rgb,
        opaque: !is_transparent,
        source_sprites: vec![SpriteId::new(u32::from(file_id))],
        combine_modes: Vec::new(),
        combine_directions: Vec::new(),
        color_transforms: Vec::new(),
        animation_direction,
        animation_speed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::test_support;

    #[test]
    fn target_layout_preserves_single_sprite_and_animation_metadata()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        assert!(context.requires_revision_gate(TEXTURE_LAYOUT_233_PLUS_GATE));
        let source = ArchiveFileProvenance::new(9, 0, Some(17));
        let texture = decode_texture_definition(
            TextureId::new(17),
            &[0x12, 0x34, 0xab, 0xcd, 0, 3, 7],
            &context,
            &source,
        )?;

        assert_eq!(texture.identity.id, TextureId::new(17));
        assert_eq!(texture.identity.provenance, *context.target_provenance());
        assert_eq!(texture.average_rgb, 0xabcd);
        assert!(texture.opaque);
        assert_eq!(texture.source_sprites, vec![SpriteId::new(0x1234)]);
        assert!(texture.combine_modes.is_empty());
        assert!(texture.combine_directions.is_empty());
        assert!(texture.color_transforms.is_empty());
        assert_eq!(texture.animation_direction, 3);
        assert_eq!(texture.animation_speed, 7);
        Ok(())
    }

    #[test]
    fn target_transparency_flag_inverts_into_canonical_opacity()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let source = ArchiveFileProvenance::new(9, 0, Some(18));
        let texture = decode_texture_definition(
            TextureId::new(18),
            &[0, 9, 0, 10, 1, 0, 0],
            &context,
            &source,
        )?;

        assert!(!texture.opaque);
        assert_eq!(texture.source_sprites, vec![SpriteId::new(9)]);
        Ok(())
    }

    #[test]
    fn target_layout_rejects_trailing_historical_texture_payload()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let source = ArchiveFileProvenance::new(9, 0, Some(19));
        let error = decode_texture_definition(
            TextureId::new(19),
            &[0, 1, 0, 2, 0, 0, 0, 99],
            &context,
            &source,
        )
        .expect_err("target texture layout must reject trailing bytes");

        assert_eq!(error.span(), ByteSpan::new(7, 1));
        Ok(())
    }
}
