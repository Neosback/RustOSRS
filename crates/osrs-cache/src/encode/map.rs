use crate::decode::{DecodedLocations, DecodedTerrain, EncodedTileHeight};
use std::fmt;

const MAX_SETTINGS: u8 = 32;
const MAX_UNDERLAY_ID: u16 = u16::MAX - 81;
const EXTENDED_SMART_CONTINUATION: u32 = 32_767;
const MAX_SMART: u32 = 32_767;

/// A value that the target map streams cannot represent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MapEncodeError {
    /// Overlay shape is above 11 or rotation above 3 (opcodes 2..=49).
    OverlayShapeOutOfRange { shape: u8, rotation: u8 },
    /// Tile settings must fit opcodes 50..=81 (values 1..=32).
    SettingsOutOfRange { settings: u8 },
    /// Underlay id does not fit the 16-bit opcode space.
    UnderlayOutOfRange { underlay_id: u16 },
    /// The terrain tile list is not the full 4 x 64 x 64 stream in plane/x/y order.
    TileOrder { index: usize },
    /// A packed location position or delta exceeds the stream's limits.
    LocationOutOfRange { detail: &'static str },
}

impl fmt::Display for MapEncodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OverlayShapeOutOfRange { shape, rotation } => write!(
                formatter,
                "overlay shape {shape} / rotation {rotation} is outside the encodable range"
            ),
            Self::SettingsOutOfRange { settings } => {
                write!(formatter, "tile settings {settings} are outside 0..=32")
            }
            Self::UnderlayOutOfRange { underlay_id } => {
                write!(
                    formatter,
                    "underlay id {underlay_id} does not fit its opcode"
                )
            }
            Self::TileOrder { index } => {
                write!(
                    formatter,
                    "terrain tile {index} is out of plane/x/y stream order"
                )
            }
            Self::LocationOutOfRange { detail } => write!(formatter, "location: {detail}"),
        }
    }
}

impl std::error::Error for MapEncodeError {}

const TILE_COUNT: usize = 4 * 64 * 64;

/// Encode a terrain stream: per tile an optional overlay, settings and underlay opcode, then the
/// height terminator, followed by the build-241 trailing flags byte (always `0`). A source square
/// whose flags byte had bit 0 set carries a second tile block the client discards and the decoder
/// does not retain, so it is not re-emitted.
pub fn encode_terrain(terrain: &DecodedTerrain) -> Result<Vec<u8>, MapEncodeError> {
    let tiles = terrain.tiles();
    if tiles.len() != TILE_COUNT {
        return Err(MapEncodeError::TileOrder { index: tiles.len() });
    }
    let mut out = Vec::with_capacity(TILE_COUNT * 2 + 64);
    for (index, tile) in tiles.iter().enumerate() {
        let expected = (
            (index / 4096) as u8,
            ((index / 64) % 64) as u8,
            (index % 64) as u8,
        );
        if (
            tile.source_plane.index().get(),
            tile.tile.x(),
            tile.tile.y(),
        ) != expected
        {
            return Err(MapEncodeError::TileOrder { index });
        }
        if let Some(overlay) = tile.overlay {
            if overlay.shape > 11 || overlay.rotation > 3 {
                return Err(MapEncodeError::OverlayShapeOutOfRange {
                    shape: overlay.shape,
                    rotation: overlay.rotation,
                });
            }
            let opcode = 2 + u16::from(overlay.shape) * 4 + u16::from(overlay.rotation);
            out.extend_from_slice(&opcode.to_be_bytes());
            out.extend_from_slice(&overlay.encoded_id.to_be_bytes());
        }
        if tile.settings != 0 {
            if tile.settings > MAX_SETTINGS {
                return Err(MapEncodeError::SettingsOutOfRange {
                    settings: tile.settings,
                });
            }
            out.extend_from_slice(&(49 + u16::from(tile.settings)).to_be_bytes());
        }
        if tile.underlay_id != 0 {
            if tile.underlay_id > MAX_UNDERLAY_ID {
                return Err(MapEncodeError::UnderlayOutOfRange {
                    underlay_id: tile.underlay_id,
                });
            }
            out.extend_from_slice(&(81 + tile.underlay_id).to_be_bytes());
        }
        match tile.height {
            EncodedTileHeight::Default => out.extend_from_slice(&0u16.to_be_bytes()),
            EncodedTileHeight::Explicit(height) => {
                out.extend_from_slice(&1u16.to_be_bytes());
                out.push(height);
            }
        }
    }
    out.push(0);
    Ok(out)
}

fn push_unsigned_short_smart(out: &mut Vec<u8>, value: u32) -> Result<(), MapEncodeError> {
    if value > MAX_SMART {
        return Err(MapEncodeError::LocationOutOfRange {
            detail: "smart value exceeds 32767",
        });
    }
    if value < 128 {
        out.push(value as u8);
    } else {
        out.extend_from_slice(&((value + 32_768) as u16).to_be_bytes());
    }
    Ok(())
}

fn push_extended_smart(out: &mut Vec<u8>, mut value: u32) -> Result<(), MapEncodeError> {
    while value >= EXTENDED_SMART_CONTINUATION {
        push_unsigned_short_smart(out, EXTENDED_SMART_CONTINUATION)?;
        value -= EXTENDED_SMART_CONTINUATION;
    }
    push_unsigned_short_smart(out, value)
}

/// Encode a location stream: objects ascending by id, each with its packed positions ascending
/// and delta-coded. Entries sharing an id and tile keep their relative order.
pub fn encode_locations(locations: &DecodedLocations) -> Result<Vec<u8>, MapEncodeError> {
    let mut entries: Vec<(u32, u32, u8)> = locations
        .locations()
        .iter()
        .map(|loc| {
            let packed = (u32::from(loc.source_plane.index().get()) << 12)
                | (u32::from(loc.tile.x()) << 6)
                | u32::from(loc.tile.y());
            (
                loc.object_id.get(),
                packed,
                (loc.loc_type << 2) | (loc.orientation & 3),
            )
        })
        .collect();
    if locations.locations().iter().any(|loc| loc.loc_type > 63) {
        return Err(MapEncodeError::LocationOutOfRange {
            detail: "loc type exceeds 63",
        });
    }
    // Stable sort: equal (id, position) keep their input order.
    entries.sort_by_key(|&(id, packed, _)| (id, packed));

    let mut out = Vec::with_capacity(entries.len() * 3 + 2);
    let mut previous_id = -1i64;
    let mut index = 0;
    while index < entries.len() {
        let id = entries[index].0;
        let delta = i64::from(id) - previous_id;
        push_extended_smart(&mut out, delta as u32)?;
        previous_id = i64::from(id);
        let mut previous_position = 0u32;
        while index < entries.len() && entries[index].0 == id {
            let (_, packed, attributes) = entries[index];
            push_unsigned_short_smart(&mut out, packed - previous_position + 1)?;
            out.push(attributes);
            previous_position = packed;
            index += 1;
        }
        push_unsigned_short_smart(&mut out, 0)?;
    }
    push_unsigned_short_smart(&mut out, 0)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::{
        ArchiveFileProvenance, DecodedLocation, DecoderContext, decode_locations, decode_terrain,
        test_support,
    };
    use osrs_core::coords::{RegionCoord, RegionTile, SourcePlane};
    use osrs_core::ids::ObjectId;

    fn context() -> Result<DecoderContext, Box<dyn std::error::Error>> {
        Ok(test_support::target_context()?)
    }

    fn source() -> ArchiveFileProvenance {
        ArchiveFileProvenance::new(5, 0, Some(0))
    }

    fn loc(id: u32, x: u8, y: u8, plane: u8, loc_type: u8, orientation: u8) -> DecodedLocation {
        DecodedLocation {
            object_id: ObjectId::new(id),
            tile: RegionTile::new(x, y).unwrap_or_else(|| unreachable!("valid tile")),
            source_plane: SourcePlane::new(plane).unwrap_or_else(|| unreachable!("valid plane")),
            loc_type,
            orientation,
        }
    }

    #[test]
    fn terrain_with_every_opcode_round_trips_to_identical_bytes()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut bytes = Vec::new();
        // First tile: overlay (shape 11, rotation 3, id with the high bit), settings 32,
        // underlay 8, explicit height 7.
        bytes.extend_from_slice(&(2u16 + 11 * 4 + 3).to_be_bytes());
        bytes.extend_from_slice(&0x8123u16.to_be_bytes());
        bytes.extend_from_slice(&81u16.to_be_bytes());
        bytes.extend_from_slice(&89u16.to_be_bytes());
        bytes.extend_from_slice(&1u16.to_be_bytes());
        bytes.push(7);
        // Second tile: settings 1 only.
        bytes.extend_from_slice(&50u16.to_be_bytes());
        bytes.extend_from_slice(&0u16.to_be_bytes());
        for _ in 2..TILE_COUNT {
            bytes.extend_from_slice(&0u16.to_be_bytes());
        }
        bytes.push(0);
        let region = RegionCoord::new(50, 50);
        let decoded = decode_terrain(&bytes, &context()?, &source(), region)?;
        assert_eq!(encode_terrain(&decoded)?, bytes);
        Ok(())
    }

    #[test]
    fn out_of_range_terrain_values_are_rejected_not_truncated()
    -> Result<(), Box<dyn std::error::Error>> {
        let bytes = vec![0u8; TILE_COUNT * 2];
        let region = RegionCoord::new(0, 0);
        let decoded = decode_terrain(&bytes, &context()?, &source(), region)?;
        let mut tiles = decoded.tiles().to_vec();
        tiles[0].settings = 33;
        let broken = DecodedTerrain::from_parts(region, tiles, 0);
        assert_eq!(
            encode_terrain(&broken),
            Err(MapEncodeError::SettingsOutOfRange { settings: 33 })
        );
        Ok(())
    }

    #[test]
    fn locations_round_trip_with_large_id_gaps_and_shared_tiles()
    -> Result<(), Box<dyn std::error::Error>> {
        let region = RegionCoord::new(50, 50);
        let locations = DecodedLocations::with_locations(
            region,
            vec![
                loc(5, 1, 1, 0, 10, 2),
                loc(5, 1, 1, 0, 22, 0),
                loc(5, 63, 63, 3, 0, 3),
                loc(70_000, 0, 0, 0, 10, 1),
                loc(0, 10, 20, 1, 4, 0),
            ],
        );
        let bytes = encode_locations(&locations)?;
        let decoded = decode_locations(&bytes, &context()?, &source(), region)?;
        let mut expected = locations.locations().to_vec();
        expected.sort_by_key(|l| {
            (
                l.object_id.get(),
                u32::from(l.source_plane.index().get()) << 12
                    | u32::from(l.tile.x()) << 6
                    | u32::from(l.tile.y()),
            )
        });
        assert_eq!(decoded.locations(), expected.as_slice());
        // Canonical: encoding the decoded value is byte-stable.
        assert_eq!(encode_locations(&decoded)?, bytes);
        Ok(())
    }

    #[test]
    fn an_empty_location_stream_is_one_terminator() -> Result<(), Box<dyn std::error::Error>> {
        let region = RegionCoord::new(1, 1);
        let bytes = encode_locations(&DecodedLocations::with_locations(region, Vec::new()))?;
        assert_eq!(bytes, vec![0]);
        assert!(decode_locations(&bytes, &context()?, &source(), region)?.is_empty());
        Ok(())
    }
}
