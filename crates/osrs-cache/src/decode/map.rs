use super::{ArchiveFileProvenance, BinaryReader, ByteSpan, DecodeResult, DecoderContext};
use osrs_core::coords::{RegionCoord, RegionTile, SourcePlane};
use osrs_core::ids::ObjectId;
use std::fmt;

/// Logical cache index containing terrain/location map-square data.
pub const MAP_INDEX_ID: u8 = 5;
/// Revision at which the pinned OpenRune tooling switches to numeric map groups.
pub const MODERN_MAP_LAYOUT_MIN_BUILD: u32 = 237;
/// File containing terrain tile opcodes in a revision-237+ map-square group.
pub const TERRAIN_FILE_ID: u32 = 0;
/// File containing loc smart/delta data in a revision-237+ map-square group.
pub const LOCATION_FILE_ID: u32 = 1;

const REGION_AXIS_LIMIT: i32 = 255;
const SOURCE_PLANE_COUNT: u8 = 4;
const REGION_AXIS_TILES: u8 = 64;
const TERRAIN_TILE_COUNT: usize = 4 * 64 * 64;
const MAX_PACKED_LOCATION_POSITION: u32 = (4 * 64 * 64) - 1;
const EXTENDED_SMART_CONTINUATION: u32 = 32_767;

/// Exact target-aware map-square archive/file identities.
///
/// Build 241 does not publish `mX_Y` / `lX_Y` archive name hashes. For the
/// revision-237+ layout the map-square identity is numeric and both streams are
/// files in the same group: file 0 is terrain and file 1 is locations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapSquareFiles {
    region: RegionCoord,
    group_id: u32,
    terrain: ArchiveFileProvenance,
    locations: ArchiveFileProvenance,
}

impl MapSquareFiles {
    pub const fn region(&self) -> RegionCoord {
        self.region
    }

    pub const fn group_id(&self) -> u32 {
        self.group_id
    }

    pub const fn terrain(&self) -> &ArchiveFileProvenance {
        &self.terrain
    }

    pub const fn locations(&self) -> &ArchiveFileProvenance {
        &self.locations
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MapSquareResolutionError {
    UnsupportedBuild {
        profile_id: String,
        build: u32,
        minimum_build: u32,
    },
    RegionOutOfRange {
        profile_id: String,
        build: u32,
        region: RegionCoord,
    },
}

impl fmt::Display for MapSquareResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedBuild {
                profile_id,
                build,
                minimum_build,
            } => write!(
                formatter,
                "target profile {profile_id} build {build} does not use the revision-{minimum_build}+ numeric map-group layout"
            ),
            Self::RegionOutOfRange {
                profile_id,
                build,
                region,
            } => write!(
                formatter,
                "map-square region ({}, {}) is outside the 0..=255 packed region range for target profile {profile_id} build {build}",
                region.x, region.y
            ),
        }
    }
}

impl std::error::Error for MapSquareResolutionError {}

/// Resolve one map square to the numeric revision-237+ index/group/file layout.
///
/// This deliberately fails for older builds instead of falling back to the
/// historical archive-name convention. Revision 237+ packs region X/Y into one
/// group id and stores terrain/location data as files 0/1 respectively.
pub fn resolve_map_square(
    context: &DecoderContext,
    region: RegionCoord,
) -> Result<MapSquareFiles, MapSquareResolutionError> {
    let profile_id = context.target_provenance().profile_id();
    let build = context.build();

    if build < MODERN_MAP_LAYOUT_MIN_BUILD {
        return Err(MapSquareResolutionError::UnsupportedBuild {
            profile_id: profile_id.to_owned(),
            build,
            minimum_build: MODERN_MAP_LAYOUT_MIN_BUILD,
        });
    }

    if !(0..=REGION_AXIS_LIMIT).contains(&region.x) || !(0..=REGION_AXIS_LIMIT).contains(&region.y)
    {
        return Err(MapSquareResolutionError::RegionOutOfRange {
            profile_id: profile_id.to_owned(),
            build,
            region,
        });
    }

    let group_id = ((region.x as u32) << 8) | region.y as u32;
    Ok(MapSquareFiles {
        region,
        group_id,
        terrain: ArchiveFileProvenance::new(MAP_INDEX_ID, group_id, Some(TERRAIN_FILE_ID)),
        locations: ArchiveFileProvenance::new(MAP_INDEX_ID, group_id, Some(LOCATION_FILE_ID)),
    })
}

/// Height opcode state preserved directly from the terrain byte stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodedTileHeight {
    /// Opcode 0: no explicit source height byte was present.
    Default,
    /// Opcode 1: the source height byte, without scene-level interpretation.
    Explicit(u8),
}

/// Overlay payload carried by terrain opcodes 2..=49.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncodedTerrainOverlay {
    /// Raw unsigned 16-bit identifier bits stored by the target stream.
    pub encoded_id: u16,
    pub shape: u8,
    pub rotation: u8,
}

/// Raw semantic inputs for one encoded terrain tile.
///
/// Region identity belongs to the containing [`DecodedTerrain`]. This value
/// keeps the local tile coordinate and encoded/source plane distinct from any
/// later scene/storage/collision/render-level interpretation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncodedTerrainTile {
    pub tile: RegionTile,
    pub source_plane: SourcePlane,
    pub height: EncodedTileHeight,
    pub overlay: Option<EncodedTerrainOverlay>,
    pub settings: u8,
    /// Raw one-based underlay value encoded as `opcode - 81`; zero means absent.
    pub underlay_id: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedTerrain {
    region: RegionCoord,
    tiles: Vec<EncodedTerrainTile>,
}

impl DecodedTerrain {
    pub const fn region(&self) -> RegionCoord {
        self.region
    }

    pub fn tiles(&self) -> &[EncodedTerrainTile] {
        &self.tiles
    }

    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }
}

/// Decode the build-241 terrain stream without applying scene semantics.
///
/// The target stream traverses plane -> local X -> local Y. Each tile consumes
/// unsigned-16-bit attributes until opcode 0 (default height) or opcode 1
/// (explicit height). No bridge/plane adjustment or terrain-color construction
/// occurs here.
pub fn decode_terrain(
    bytes: &[u8],
    context: &DecoderContext,
    source: &ArchiveFileProvenance,
    region: RegionCoord,
) -> DecodeResult<DecodedTerrain> {
    let mut reader = BinaryReader::new(bytes, context, source);
    let mut tiles = Vec::with_capacity(TERRAIN_TILE_COUNT);

    for plane_value in 0..SOURCE_PLANE_COUNT {
        let source_plane = match SourcePlane::new(plane_value) {
            Some(plane) => plane,
            None => {
                return Err(reader.invalid_value(
                    "terrain source plane",
                    format!("plane {plane_value} is outside the canonical range"),
                    ByteSpan::new(reader.offset(), 0),
                    None,
                ));
            }
        };

        for x in 0..REGION_AXIS_TILES {
            for y in 0..REGION_AXIS_TILES {
                let tile = match RegionTile::new(x, y) {
                    Some(tile) => tile,
                    None => {
                        return Err(reader.invalid_value(
                            "terrain region-local tile",
                            format!("tile ({x}, {y}) is outside the region"),
                            ByteSpan::new(reader.offset(), 0),
                            None,
                        ));
                    }
                };

                let mut height = EncodedTileHeight::Default;
                let mut overlay = None;
                let mut settings = 0u8;
                let mut underlay_id = 0u16;

                loop {
                    let opcode = reader.read_u16_be()?;
                    match opcode {
                        0 => break,
                        1 => {
                            height = EncodedTileHeight::Explicit(reader.read_u8()?);
                            break;
                        }
                        2..=49 => {
                            let encoded_id = reader.read_u16_be()?;
                            overlay = Some(EncodedTerrainOverlay {
                                encoded_id,
                                shape: ((opcode - 2) / 4) as u8,
                                rotation: ((opcode - 2) & 3) as u8,
                            });
                        }
                        50..=81 => {
                            settings = (opcode - 49) as u8;
                        }
                        _ => {
                            underlay_id = opcode - 81;
                        }
                    }
                }

                tiles.push(EncodedTerrainTile {
                    tile,
                    source_plane,
                    height,
                    overlay,
                    settings,
                    underlay_id,
                });
            }
        }
    }

    reader.finish()?;
    Ok(DecodedTerrain { region, tiles })
}

/// One location entry decoded from a region-local loc stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodedLocation {
    pub object_id: ObjectId,
    pub tile: RegionTile,
    pub source_plane: SourcePlane,
    pub loc_type: u8,
    pub orientation: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedLocations {
    region: RegionCoord,
    locations: Vec<DecodedLocation>,
}

impl DecodedLocations {
    pub const fn region(&self) -> RegionCoord {
        self.region
    }

    pub fn locations(&self) -> &[DecodedLocation] {
        &self.locations
    }

    pub fn len(&self) -> usize {
        self.locations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.locations.is_empty()
    }
}

/// Decode a location stream using extended-smart object deltas and
/// unsigned-short-smart packed-position deltas.
pub fn decode_locations(
    bytes: &[u8],
    context: &DecoderContext,
    source: &ArchiveFileProvenance,
    region: RegionCoord,
) -> DecodeResult<DecodedLocations> {
    let mut reader = BinaryReader::new(bytes, context, source);
    let mut locations = Vec::new();
    let mut object_id = -1i64;

    loop {
        let id_delta = read_extended_smart(&mut reader)?;
        if id_delta == 0 {
            break;
        }

        let next_object_id = match object_id.checked_add(i64::from(id_delta)) {
            Some(value) if value <= i64::from(u32::MAX) => value,
            _ => {
                return Err(reader.invalid_value(
                    "location object id",
                    "smart/delta object id exceeds u32 canonical identity",
                    ByteSpan::new(reader.offset(), 0),
                    None,
                ));
            }
        };
        object_id = next_object_id;
        let canonical_object_id = ObjectId::new(object_id as u32);

        let mut packed_position = 0u32;
        loop {
            let position_delta = u32::from(reader.read_unsigned_short_smart()?);
            if position_delta == 0 {
                break;
            }

            let increment = position_delta - 1;
            packed_position = match packed_position.checked_add(increment) {
                Some(value) => value,
                None => {
                    return Err(reader.invalid_value(
                        "location packed position",
                        "smart/delta packed position overflowed u32",
                        ByteSpan::new(reader.offset(), 0),
                        None,
                    ));
                }
            };

            if packed_position > MAX_PACKED_LOCATION_POSITION {
                return Err(reader.invalid_value(
                    "location packed position",
                    format!("packed position {packed_position} exceeds four 64x64 source planes"),
                    ByteSpan::new(reader.offset(), 0),
                    None,
                ));
            }

            let local_y = (packed_position & 63) as u8;
            let local_x = ((packed_position >> 6) & 63) as u8;
            let plane_value = (packed_position >> 12) as u8;
            let tile = match RegionTile::new(local_x, local_y) {
                Some(tile) => tile,
                None => {
                    return Err(reader.invalid_value(
                        "location region-local tile",
                        format!("tile ({local_x}, {local_y}) is outside the region"),
                        ByteSpan::new(reader.offset(), 0),
                        None,
                    ));
                }
            };
            let source_plane = match SourcePlane::new(plane_value) {
                Some(plane) => plane,
                None => {
                    return Err(reader.invalid_value(
                        "location source plane",
                        format!("plane {plane_value} is outside the canonical range"),
                        ByteSpan::new(reader.offset(), 0),
                        None,
                    ));
                }
            };

            let attributes = reader.read_u8()?;
            locations.push(DecodedLocation {
                object_id: canonical_object_id,
                tile,
                source_plane,
                loc_type: attributes >> 2,
                orientation: attributes & 3,
            });
        }
    }

    reader.finish()?;
    Ok(DecodedLocations { region, locations })
}

fn read_extended_smart(reader: &mut BinaryReader<'_>) -> DecodeResult<u32> {
    let start = reader.offset();
    let mut total = 0u32;

    loop {
        let value = u32::from(reader.read_unsigned_short_smart()?);
        total = match total.checked_add(value) {
            Some(total) => total,
            None => {
                return Err(reader.invalid_value(
                    "location object-id delta",
                    "extended-smart accumulator overflowed u32",
                    ByteSpan::new(start, reader.offset().saturating_sub(start)),
                    None,
                ));
            }
        };

        if value != EXTENDED_SMART_CONTINUATION {
            return Ok(total);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::{DecodeErrorKind, test_support};

    fn push_unsigned_short_smart(bytes: &mut Vec<u8>, value: u16) {
        if value < 128 {
            bytes.push(value as u8);
        } else {
            bytes.extend_from_slice(&(value + 32_768).to_be_bytes());
        }
    }

    fn terrain_fixture() -> Vec<u8> {
        let mut bytes = Vec::with_capacity(TERRAIN_TILE_COUNT * 2 + 16);

        // First tile: overlay(shape 1, rotation 1), settings 3, underlay 8,
        // explicit height 7. The overlay id deliberately exercises all u16 bits.
        bytes.extend_from_slice(&7u16.to_be_bytes());
        bytes.extend_from_slice(&0x8123u16.to_be_bytes());
        bytes.extend_from_slice(&52u16.to_be_bytes());
        bytes.extend_from_slice(&89u16.to_be_bytes());
        bytes.extend_from_slice(&1u16.to_be_bytes());
        bytes.push(7);

        for _ in 1..TERRAIN_TILE_COUNT {
            bytes.extend_from_slice(&0u16.to_be_bytes());
        }
        bytes
    }

    #[test]
    fn build_241_map_square_resolves_to_numeric_group_and_two_files()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let resolved = resolve_map_square(&context, RegionCoord::new(50, 50))?;

        assert_eq!(resolved.group_id(), (50 << 8) | 50);
        assert_eq!(resolved.terrain().index_id(), MAP_INDEX_ID);
        assert_eq!(resolved.terrain().group_id(), resolved.group_id());
        assert_eq!(resolved.terrain().file_id(), Some(TERRAIN_FILE_ID));
        assert_eq!(resolved.locations().group_id(), resolved.group_id());
        assert_eq!(resolved.locations().file_id(), Some(LOCATION_FILE_ID));
        assert_eq!(resolved.terrain().xtea(), None);
        assert_eq!(resolved.locations().xtea(), None);
        Ok(())
    }

    #[test]
    fn numeric_map_square_resolution_rejects_aliasing_region_coordinates()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let error = match resolve_map_square(&context, RegionCoord::new(50, 256)) {
            Err(error) => error,
            Ok(_) => return Err("out-of-range region unexpectedly resolved".into()),
        };

        assert!(matches!(
            error,
            MapSquareResolutionError::RegionOutOfRange {
                build: 241,
                region: RegionCoord { x: 50, y: 256 },
                ..
            }
        ));
        Ok(())
    }

    #[test]
    fn terrain_decoder_preserves_raw_tile_fields_and_exact_traversal()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let source = ArchiveFileProvenance::new(MAP_INDEX_ID, 12_850, Some(TERRAIN_FILE_ID));
        let decoded = decode_terrain(
            &terrain_fixture(),
            &context,
            &source,
            RegionCoord::new(50, 50),
        )?;

        assert_eq!(decoded.region(), RegionCoord::new(50, 50));
        assert_eq!(decoded.len(), TERRAIN_TILE_COUNT);

        let first = decoded.tiles()[0];
        assert_eq!(
            first.tile,
            RegionTile::new(0, 0).ok_or("invalid fixture tile")?
        );
        assert_eq!(
            first.source_plane,
            SourcePlane::new(0).ok_or("invalid fixture plane")?
        );
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

        let second = decoded.tiles()[1];
        assert_eq!(
            second.tile,
            RegionTile::new(0, 1).ok_or("invalid fixture tile")?
        );
        assert_eq!(second.height, EncodedTileHeight::Default);
        assert_eq!(second.overlay, None);
        assert_eq!(second.settings, 0);
        assert_eq!(second.underlay_id, 0);

        let plane_one_first = decoded.tiles()[64 * 64];
        assert_eq!(
            plane_one_first.source_plane,
            SourcePlane::new(1).ok_or("invalid fixture plane")?
        );
        assert_eq!(
            plane_one_first.tile,
            RegionTile::new(0, 0).ok_or("invalid fixture tile")?
        );
        Ok(())
    }

    #[test]
    fn location_decoder_preserves_full_width_ids_plane_local_coords_type_and_orientation()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let source = ArchiveFileProvenance::new(MAP_INDEX_ID, 12_850, Some(LOCATION_FILE_ID));
        let mut bytes = Vec::new();

        // -1 + (32767 + 32767 + 3) = 65536, proving the canonical u32 path.
        push_unsigned_short_smart(&mut bytes, 32_767);
        push_unsigned_short_smart(&mut bytes, 32_767);
        push_unsigned_short_smart(&mut bytes, 3);

        let packed_position = (2u16 << 12) | (10u16 << 6) | 20u16;
        push_unsigned_short_smart(&mut bytes, packed_position + 1);
        bytes.push((10 << 2) | 3);
        push_unsigned_short_smart(&mut bytes, 0); // end positions for object
        push_unsigned_short_smart(&mut bytes, 0); // end object stream

        let decoded = decode_locations(&bytes, &context, &source, RegionCoord::new(50, 50))?;
        assert_eq!(decoded.region(), RegionCoord::new(50, 50));
        assert_eq!(decoded.len(), 1);

        let location = decoded.locations()[0];
        assert_eq!(location.object_id, ObjectId::new(65_536));
        assert_eq!(
            location.tile,
            RegionTile::new(10, 20).ok_or("invalid fixture tile")?
        );
        assert_eq!(
            location.source_plane,
            SourcePlane::new(2).ok_or("invalid fixture plane")?
        );
        assert_eq!(location.loc_type, 10);
        assert_eq!(location.orientation, 3);
        Ok(())
    }

    #[test]
    fn location_decoder_rejects_packed_positions_outside_four_planes()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let source = ArchiveFileProvenance::new(MAP_INDEX_ID, 12_850, Some(LOCATION_FILE_ID));
        let mut bytes = Vec::new();
        push_unsigned_short_smart(&mut bytes, 1); // object id 0
        push_unsigned_short_smart(&mut bytes, 16_385); // packed position becomes 16384 / plane 4

        let error = match decode_locations(&bytes, &context, &source, RegionCoord::new(50, 50)) {
            Err(error) => error,
            Ok(_) => return Err("out-of-range packed position unexpectedly decoded".into()),
        };

        assert!(matches!(
            error.kind(),
            DecodeErrorKind::InvalidValue {
                field: "location packed position",
                ..
            }
        ));
        assert_eq!(error.source_provenance(), &source);
        Ok(())
    }
}
