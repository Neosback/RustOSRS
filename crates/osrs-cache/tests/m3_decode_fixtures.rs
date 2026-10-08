use osrs_cache::decode::{
    ArchiveFileProvenance, DecodedLocation, EncodedTerrainOverlay, EncodedTileHeight, DecoderContext,
    decode_floor_overlay, decode_floor_underlay, decode_locations, decode_object_definition,
    decode_sequence_definition, decode_terrain, decode_texture_definition, decode_varbit, decode_varp,
};
use osrs_cache::profile::TargetProfile;
use osrs_core::coords::{RegionCoord, RegionTile, SourcePlane};
use osrs_core::definitions::{ObjectMorphs, ObjectModels, Rgb24, SequenceRange, TypedObjectModel};
use osrs_core::floor_color::{OverlayHsl, UnderlayHsl};
use osrs_core::ids::{
    FloorOverlayId, FloorUnderlayId, ItemId, ModelId, ObjectId, SequenceId, SkeletalAnimationId,
    SpriteId, TextureId, VarbitId, VarpId,
};

const TARGET_PROFILE_YAML: &str =
    include_str!("../../../profiles/osrs-live-241-2026-09-30-openrs2-2727.yaml");
const FIXTURES: &str = include_str!("../../../reference-fixtures/decode/m3-p0.txt");
const TERRAIN_TILE_COUNT: usize = 4 * 64 * 64;

#[derive(Debug)]
struct Fixture {
    name: String,
    decoder: String,
    id: u32,
    index: u8,
    group: u32,
    file: u32,
    bytes: Vec<u8>,
}

fn context() -> Result<DecoderContext, Box<dyn std::error::Error>> {
    let profile = TargetProfile::from_yaml_str(TARGET_PROFILE_YAML)?;
    Ok(DecoderContext::from_profile(&profile)?)
}

fn fixtures() -> Result<Vec<Fixture>, Box<dyn std::error::Error>> {
    let mut fixtures = Vec::new();
    for (line_number, raw) in FIXTURES.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts = line.split('|').collect::<Vec<_>>();
        if parts.len() != 7 {
            return Err(format!("fixture line {} has {} fields", line_number + 1, parts.len()).into());
        }
        let bytes = parts[6]
            .split_ascii_whitespace()
            .map(|token| u8::from_str_radix(token, 16))
            .collect::<Result<Vec<_>, _>>()?;
        fixtures.push(Fixture {
            name: parts[0].to_owned(),
            decoder: parts[1].to_owned(),
            id: parts[2].parse()?,
            index: parts[3].parse()?,
            group: parts[4].parse()?,
            file: parts[5].parse()?,
            bytes,
        });
    }
    Ok(fixtures)
}

#[test]
fn checked_in_m3_p0_vectors_match_canonical_outputs() -> Result<(), Box<dyn std::error::Error>> {
    let context = context()?;
    let fixtures = fixtures()?;
    assert_eq!(fixtures.len(), 10, "fixture inventory changed without updating this gate");

    for fixture in fixtures {
        let source = ArchiveFileProvenance::new(fixture.index, fixture.group, Some(fixture.file));
        match fixture.decoder.as_str() {
            "object" if fixture.name == "object_extended_model" => {
                let decoded = decode_object_definition(
                    ObjectId::new(fixture.id),
                    &fixture.bytes,
                    &context,
                    &source,
                )?;
                assert_eq!(
                    decoded.models,
                    Some(ObjectModels::Typed(vec![TypedObjectModel {
                        model_id: ModelId::new(0x1234_5678),
                        loc_type: osrs_core::definitions::LocType::new(22),
                    }]))
                );
            }
            "object" if fixture.name == "object_morph_fallback" => {
                let decoded = decode_object_definition(
                    ObjectId::new(fixture.id),
                    &fixture.bytes,
                    &context,
                    &source,
                )?;
                assert_eq!(
                    decoded.morphs,
                    Some(ObjectMorphs {
                        transform_varbit: None,
                        transform_varp: Some(VarpId::new(9)),
                        transforms: vec![None],
                        fallback: Some(ObjectId::new(200)),
                    })
                );
            }
            "underlay" => {
                let decoded = decode_floor_underlay(
                    FloorUnderlayId::new(fixture.id),
                    &fixture.bytes,
                    &context,
                    &source,
                )?;
                assert_eq!(decoded.rgb.get(), 0x12_34_56);
                assert_eq!(
                    decoded.hsl,
                    UnderlayHsl {
                        weighted_hue: 39,
                        saturation: 167,
                        lightness: 52,
                        hue_multiplier: 68,
                    }
                );
            }
            "overlay" => {
                let decoded = decode_floor_overlay(
                    FloorOverlayId::new(fixture.id),
                    &fixture.bytes,
                    &context,
                    &source,
                )?;
                assert_eq!(decoded.primary_rgb.get(), 0x12_34_56);
                assert_eq!(decoded.texture, Some(TextureId::new(7)));
                assert!(!decoded.hide_underlay);
                assert_eq!(decoded.secondary_rgb.map(Rgb24::get), Some(0xab_cd_ef));
                assert_eq!(
                    decoded.primary_hsl,
                    OverlayHsl {
                        hue: 149,
                        saturation: 167,
                        lightness: 52,
                    }
                );
                assert_eq!(
                    decoded.secondary_hsl,
                    Some(OverlayHsl {
                        hue: 149,
                        saturation: 170,
                        lightness: 205,
                    })
                );
            }
            "varbit" => {
                let decoded = decode_varbit(
                    VarbitId::new(fixture.id),
                    &fixture.bytes,
                    &context,
                    &source,
                )?;
                assert_eq!(decoded.base_varp, VarpId::new(0x1234));
                assert_eq!((decoded.start_bit, decoded.end_bit), (3, 7));
            }
            "varp" => {
                let decoded = decode_varp(
                    VarpId::new(fixture.id),
                    &fixture.bytes,
                    &context,
                    &source,
                )?;
                assert_eq!(decoded.client_type, 0xabcd);
            }
            "texture" => {
                let decoded = decode_texture_definition(
                    TextureId::new(fixture.id),
                    &fixture.bytes,
                    &context,
                    &source,
                )?;
                assert_eq!(decoded.source_sprites, vec![SpriteId::new(0x1234)]);
                assert_eq!(decoded.average_rgb, 0xabcd);
                assert!(decoded.opaque);
                assert_eq!((decoded.animation_direction, decoded.animation_speed), (3, 7));
            }
            "sequence" => {
                let decoded = decode_sequence_definition(
                    SequenceId::new(fixture.id),
                    &fixture.bytes,
                    &context,
                    &source,
                )?;
                assert_eq!(decoded.left_hand_item, Some(ItemId::new(4151)));
                assert_eq!(decoded.right_hand_item, Some(ItemId::new(65535)));
                assert_eq!(decoded.skeletal_animation, Some(SkeletalAnimationId::new(900)));
                assert_eq!(decoded.skeletal_range, Some(SequenceRange { start: 10, end: 20 }));
                let mask = decoded.skeletal_mask.ok_or("skeletal fixture lost opcode-17 mask")?;
                assert!(mask[0] && mask[7] && mask[255]);
            }
            "terrain" => {
                let mut bytes = fixture.bytes;
                for _ in 1..TERRAIN_TILE_COUNT {
                    bytes.extend_from_slice(&0u16.to_be_bytes());
                }
                let decoded = decode_terrain(
                    &bytes,
                    &context,
                    &source,
                    RegionCoord::new(50, 50),
                )?;
                assert_eq!(decoded.len(), TERRAIN_TILE_COUNT);
                let first = decoded.tiles()[0];
                assert_eq!(first.tile, RegionTile::new(0, 0).ok_or("invalid fixture tile")?);
                assert_eq!(first.source_plane, SourcePlane::new(0).ok_or("invalid fixture plane")?);
                assert_eq!(first.height, EncodedTileHeight::Explicit(7));
                assert_eq!(
                    first.overlay,
                    Some(EncodedTerrainOverlay {
                        encoded_id: 0x8123,
                        shape: 1,
                        rotation: 1,
                    })
                );
                assert_eq!(first.settings, 3);
                assert_eq!(first.underlay_id, 8);
            }
            "locations" => {
                let decoded = decode_locations(
                    &fixture.bytes,
                    &context,
                    &source,
                    RegionCoord::new(50, 50),
                )?;
                assert_eq!(decoded.len(), 1);
                assert_eq!(
                    decoded.locations()[0],
                    DecodedLocation {
                        object_id: ObjectId::new(65_536),
                        tile: RegionTile::new(10, 20).ok_or("invalid fixture tile")?,
                        source_plane: SourcePlane::new(2).ok_or("invalid fixture plane")?,
                        loc_type: 10,
                        orientation: 3,
                    }
                );
            }
            _ => return Err(format!("unhandled M3 fixture {}", fixture.name).into()),
        }
    }

    Ok(())
}
