//! Indexed sprite group decoder (cache index 8).
//!
//! Layout (read from the end): `u16 frame_count`; before it the per-frame metadata block
//! `u16 max_width, u16 max_height, u8 palette_length - 1`, four `u16[frame_count]` arrays
//! (x offset, y offset, width, height) and `(palette_length - 1) * 3` palette bytes. Frame pixel
//! data starts at offset 0: per frame one flags byte (`1` = column-major, `2` = has alpha), the
//! palette indices, then optional alpha bytes. Palette entry `0` is transparent and a palette
//! color of `0` is stored as `1` so it cannot be confused with transparency.

use super::{
    ArchiveFileProvenance, BinaryReader, ByteSpan, DecodeResult, DecodeSubject, DecoderContext,
};

const FLAG_VERTICAL: u8 = 0b01;
const FLAG_ALPHA: u8 = 0b10;

/// One decoded sprite frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedSprite {
    pub max_width: u16,
    pub max_height: u16,
    pub x_offset: u16,
    pub y_offset: u16,
    pub width: u16,
    pub height: u16,
    /// Palette (`palette[0] == 0` is transparent); 24-bit RGB values.
    pub palette: Vec<u32>,
    /// `width * height` palette indices, row-major.
    pub indices: Vec<u8>,
    /// Optional per-pixel alpha, row-major.
    pub alpha: Option<Vec<u8>>,
}

impl IndexedSprite {
    /// Palette indices expanded into the full `max_width x max_height` canvas
    /// (`IndexedSprite.normalize`): the frame sits at its offset, everything else is index `0`.
    pub fn normalized_indices(&self) -> Vec<u8> {
        let (max_width, max_height) = (usize::from(self.max_width), usize::from(self.max_height));
        if usize::from(self.width) == max_width && usize::from(self.height) == max_height {
            return self.indices.clone();
        }
        let mut canvas = vec![0_u8; max_width * max_height];
        let (width, height) = (usize::from(self.width), usize::from(self.height));
        for row in 0..height {
            for column in 0..width {
                let target = (row + usize::from(self.y_offset)) * max_width
                    + column
                    + usize::from(self.x_offset);
                if let Some(slot) = canvas.get_mut(target) {
                    *slot = self.indices[row * width + column];
                }
            }
        }
        canvas
    }
}

/// Decode every frame of a sprite group file.
pub fn decode_sprite_group(
    id: u32,
    bytes: &[u8],
    context: &DecoderContext,
    source: &ArchiveFileProvenance,
) -> DecodeResult<Vec<IndexedSprite>> {
    let mut reader =
        BinaryReader::new(bytes, context, source).with_subject(DecodeSubject::Sprite(id));
    if bytes.len() < 2 {
        return Err(reader.invalid_value(
            "sprite group",
            "group is shorter than its frame-count trailer",
            ByteSpan::new(0, bytes.len()),
            None,
        ));
    }
    reader.seek(bytes.len() - 2)?;
    let frame_count = usize::from(reader.read_u16_be()?);

    let metadata_len = 7 + frame_count * 8;
    if bytes.len() < metadata_len + 2 {
        return Err(reader.invalid_value(
            "sprite metadata",
            "group is shorter than its metadata block",
            ByteSpan::new(0, bytes.len()),
            None,
        ));
    }
    reader.seek(bytes.len() - 2 - metadata_len + 2)?;
    let max_width = reader.read_u16_be()?;
    let max_height = reader.read_u16_be()?;
    let palette_len = usize::from(reader.read_u8()?) + 1;

    let read_array = |reader: &mut BinaryReader<'_>| -> DecodeResult<Vec<u16>> {
        (0..frame_count).map(|_| reader.read_u16_be()).collect()
    };
    let x_offsets = read_array(&mut reader)?;
    let y_offsets = read_array(&mut reader)?;
    let widths = read_array(&mut reader)?;
    let heights = read_array(&mut reader)?;

    let palette_start = bytes.len() - 2 - metadata_len - (palette_len - 1) * 3 + 2;
    reader.seek(palette_start)?;
    let mut palette = vec![0_u32; palette_len];
    for entry in palette.iter_mut().skip(1) {
        let rgb = reader.read_u24_be()?;
        *entry = if rgb == 0 { 1 } else { rgb };
    }

    reader.seek(0)?;
    let mut frames = Vec::with_capacity(frame_count);
    for frame in 0..frame_count {
        let (width, height) = (usize::from(widths[frame]), usize::from(heights[frame]));
        let area = width * height;
        let flags = reader.read_u8()?;
        let mut indices = vec![0_u8; area];
        if flags & FLAG_VERTICAL == 0 {
            indices.copy_from_slice(reader.read_bytes(area)?);
        } else {
            for column in 0..width {
                for row in 0..height {
                    indices[width * row + column] = reader.read_u8()?;
                }
            }
        }
        let alpha = if flags & FLAG_ALPHA != 0 {
            let mut alpha = vec![0_u8; area];
            if flags & FLAG_VERTICAL == 0 {
                alpha.copy_from_slice(reader.read_bytes(area)?);
            } else {
                for column in 0..width {
                    for row in 0..height {
                        alpha[width * row + column] = reader.read_u8()?;
                    }
                }
            }
            Some(alpha)
        } else {
            None
        };
        frames.push(IndexedSprite {
            max_width,
            max_height,
            x_offset: x_offsets[frame],
            y_offset: y_offsets[frame],
            width: widths[frame],
            height: heights[frame],
            palette: palette.clone(),
            indices,
            alpha,
        });
    }
    Ok(frames)
}
