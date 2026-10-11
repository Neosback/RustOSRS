//! The client's bitmap fonts (`AbstractFont` / `Font`), enough to draw world-map labels exactly:
//! the glyph sprites come from cache index 8, the advances and kerning from index 13.

use crate::WorldDefinitions;
use osrs_cache::decode::decode_sprite_group;

const SPRITE_INDEX: u8 = 8;
const FONT_METRICS_INDEX: u8 = 13;

/// One glyph: its offset from the pen, size and lit pixels.
#[derive(Debug, Clone, Default)]
struct Glyph {
    left: i32,
    top: i32,
    width: i32,
    height: i32,
    pixels: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct BitmapFont {
    advances: [i32; 256],
    ascent: i32,
    kerning: Option<Vec<i8>>,
    glyphs: Vec<Glyph>,
    max_ascent: i32,
    max_descent: i32,
}

/// A rendered label: RGBA pixels and where its top-left sits relative to the anchor point (the
/// label's centre x, its top y).
#[derive(Debug, Clone)]
pub struct LabelImage {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
    pub offset_x: i32,
    pub offset_y: i32,
}

/// `Sound.charToByteCp1252` for the characters labels use.
fn cp1252(character: char) -> usize {
    let code = u32::from(character);
    let byte = match code {
        0x20AC => 0x80,
        0x201A => 0x82,
        0x0192 => 0x83,
        0x201E => 0x84,
        0x2026 => 0x85,
        0x2020 => 0x86,
        0x2021 => 0x87,
        0x02C6 => 0x88,
        0x2030 => 0x89,
        0x0160 => 0x8A,
        0x2039 => 0x8B,
        0x0152 => 0x8C,
        0x017D => 0x8E,
        0x2018 => 0x91,
        0x2019 => 0x92,
        0x201C => 0x93,
        0x201D => 0x94,
        0x2022 => 0x95,
        0x2013 => 0x96,
        0x2014 => 0x97,
        0x02DC => 0x98,
        0x2122 => 0x99,
        0x0161 => 0x9A,
        0x203A => 0x9B,
        0x0153 => 0x9C,
        0x017E => 0x9E,
        0x0178 => 0x9F,
        0..=255 => code,
        _ => 63,
    };
    (byte & 0xFF) as usize
}

impl BitmapFont {
    /// Load the font whose group id is `group` (metrics in index 13, glyphs in index 8).
    pub fn load(definitions: &WorldDefinitions, group: u32) -> Option<Self> {
        let cache = definitions.cache();
        let metrics = cache.read_file(FONT_METRICS_INDEX, group, 0, None).ok()?;
        let sprites = cache.read_file(SPRITE_INDEX, group, 0, None).ok()?;
        let frames = decode_sprite_group(
            group,
            &sprites.bytes,
            definitions.context(),
            &sprites.provenance,
        )
        .ok()?;
        let mut glyphs = vec![Glyph::default(); 256];
        for (index, frame) in frames.iter().enumerate().take(256) {
            glyphs[index] = Glyph {
                left: i32::from(frame.x_offset),
                top: i32::from(frame.y_offset),
                width: i32::from(frame.width),
                height: i32::from(frame.height),
                pixels: frame.indices.clone(),
            };
        }
        let mut font = Self::read_metrics(&metrics.bytes)?;
        let (mut min_top, mut max_bottom) = (i32::MAX, i32::MIN);
        for glyph in &glyphs {
            if glyph.top < min_top && glyph.height != 0 {
                min_top = glyph.top;
            }
            max_bottom = max_bottom.max(glyph.top + glyph.height);
        }
        font.max_ascent = font.ascent - min_top;
        font.max_descent = max_bottom - font.ascent;
        font.glyphs = glyphs;
        Some(font)
    }

    /// `AbstractFont.readMetrics`.
    fn read_metrics(bytes: &[u8]) -> Option<Self> {
        let mut advances = [0_i32; 256];
        if bytes.len() == 257 {
            for (slot, byte) in advances.iter_mut().zip(bytes) {
                *slot = i32::from(*byte);
            }
            return Some(Self {
                advances,
                ascent: i32::from(bytes[256]),
                kerning: None,
                glyphs: Vec::new(),
                max_ascent: 0,
                max_descent: 0,
            });
        }
        let mut at = 0;
        let next = |at: &mut usize| -> Option<u8> {
            let byte = *bytes.get(*at)?;
            *at += 1;
            Some(byte)
        };
        for slot in &mut advances {
            *slot = i32::from(next(&mut at)?);
        }
        let mut heights = [0_i32; 256];
        let mut tops = [0_i32; 256];
        for slot in &mut heights {
            *slot = i32::from(next(&mut at)?);
        }
        for slot in &mut tops {
            *slot = i32::from(next(&mut at)?);
        }
        let mut left_profile: Vec<Vec<i8>> = Vec::with_capacity(256);
        for height in heights {
            let mut running = 0_i8;
            let mut row = Vec::with_capacity(height as usize);
            for _ in 0..height {
                running = running.wrapping_add(next(&mut at)? as i8);
                row.push(running);
            }
            left_profile.push(row);
        }
        let mut right_profile: Vec<Vec<i8>> = Vec::with_capacity(256);
        for height in heights {
            let mut running = 0_i8;
            let mut row = Vec::with_capacity(height as usize);
            for _ in 0..height {
                running = running.wrapping_add(next(&mut at)? as i8);
                row.push(running);
            }
            right_profile.push(row);
        }
        let mut kerning = vec![0_i8; 65_536];
        for first in 0..256_usize {
            if first == 32 || first == 160 {
                continue;
            }
            for second in 0..256_usize {
                if second == 32 || second == 160 {
                    continue;
                }
                kerning[second + (first << 8)] = kerning_between(
                    &left_profile,
                    &right_profile,
                    &tops,
                    &advances,
                    &heights,
                    first,
                    second,
                ) as i8;
            }
        }
        Some(Self {
            advances,
            ascent: tops[32] + heights[32],
            kerning: Some(kerning),
            glyphs: Vec::new(),
            max_ascent: 0,
            max_descent: 0,
        })
    }

    /// `AbstractFont.stringWidth` for one tag-free line.
    pub fn string_width(&self, line: &str) -> i32 {
        let mut width = 0;
        let mut previous: Option<usize> = None;
        for character in line.chars() {
            let code = cp1252(if character == '\u{a0}' {
                ' '
            } else {
                character
            });
            width += self.advances[code];
            if let (Some(kerning), Some(previous)) = (&self.kerning, previous) {
                width += i32::from(kerning[code + (previous << 8)]);
            }
            previous = Some(code);
        }
        width
    }

    /// `AbstractFont.lineCount`/`breakLines` for labels: lines split at `<br>`; other tags drop.
    pub fn lines(text: &str) -> Vec<String> {
        text.split("<br>").map(strip_tags).collect()
    }

    /// Draw a label as `WorldMapRegion.method6722` does: centred lines starting at the anchor's y,
    /// colour with a one-pixel black shadow.
    pub fn render_label(&self, text: &str, colour: u32) -> Option<LabelImage> {
        let lines = Self::lines(text);
        if lines.is_empty() {
            return None;
        }
        let widest = lines.iter().map(|line| self.string_width(line)).max()?;
        let line_height = (self.ascent / 2).max(1);
        // Canvas generous enough for glyph overhang; the anchor is the box (x - widest / 2, y).
        let pad = 3;
        let width = (widest + 2 * pad) as usize;
        let height = (self.max_ascent
            + self.max_descent
            + line_height * (lines.len() as i32 - 1)
            + 2 * pad
            + 2) as usize;
        let mut rgba = vec![0_u8; width * height * 4];
        let shadow = [0_u8, 0, 0, 255];
        let fill = [(colour >> 16) as u8, (colour >> 8) as u8, colour as u8, 255];
        let mut baseline = self.max_ascent;
        for line in &lines {
            let mut pen = (widest - self.string_width(line)) / 2;
            let mut previous: Option<usize> = None;
            for character in line.chars() {
                let code = cp1252(if character == '\u{a0}' {
                    ' '
                } else {
                    character
                });
                if let (Some(kerning), Some(previous)) = (&self.kerning, previous) {
                    pen += i32::from(kerning[code + (previous << 8)]);
                }
                if code != 32 {
                    let glyph = &self.glyphs[code];
                    for pass in [(1, 1, shadow), (0, 0, fill)] {
                        for row in 0..glyph.height {
                            for column in 0..glyph.width {
                                if glyph.pixels[(row * glyph.width + column) as usize] == 0 {
                                    continue;
                                }
                                let x = pad + pen + glyph.left + column + pass.0;
                                let y = pad + baseline - self.ascent + glyph.top + row + pass.1;
                                if x >= 0 && y >= 0 && (x as usize) < width && (y as usize) < height
                                {
                                    let at = (y as usize * width + x as usize) * 4;
                                    rgba[at..at + 4].copy_from_slice(&pass.2);
                                }
                            }
                        }
                    }
                }
                pen += self.advances[code];
                previous = Some(code);
            }
            baseline += line_height;
        }
        Some(LabelImage {
            width,
            height,
            rgba,
            offset_x: -(widest / 2) - pad,
            offset_y: -pad,
        })
    }
}

/// Remove `<...>` markup (colour and style tags), keeping `<lt>` / `<gt>` escapes.
fn strip_tags(line: &str) -> String {
    let mut out = String::new();
    let mut tag: Option<String> = None;
    for character in line.chars() {
        match (&mut tag, character) {
            (None, '<') => tag = Some(String::new()),
            (Some(name), '>') => {
                match name.as_str() {
                    "lt" => out.push('<'),
                    "gt" => out.push('>'),
                    _ => {}
                }
                tag = None;
            }
            (Some(name), other) => name.push(other),
            (None, other) => out.push(other),
        }
    }
    out
}

/// `AbstractFont.method9718`: the tightest gap between two glyphs' profiles.
fn kerning_between(
    left_profile: &[Vec<i8>],
    right_profile: &[Vec<i8>],
    tops: &[i32; 256],
    advances: &[i32; 256],
    heights: &[i32; 256],
    first: usize,
    second: usize,
) -> i32 {
    let first_top = tops[first];
    let first_bottom = first_top + heights[first];
    let second_top = tops[second];
    let second_bottom = second_top + heights[second];
    let start = first_top.max(second_top);
    let end = first_bottom.min(second_bottom);
    let mut gap = advances[first].min(advances[second]);
    let first_right = &right_profile[first];
    let second_left = &left_profile[second];
    let a = (start - first_top) as usize;
    let b = (start - second_top) as usize;
    let rows = (end - start).max(0) as usize;
    for (right, left) in first_right[a..].iter().zip(&second_left[b..]).take(rows) {
        gap = gap.min(i32::from(*right) + i32::from(*left));
    }
    -gap
}
