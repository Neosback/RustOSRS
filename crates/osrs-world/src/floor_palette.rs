//! Display colours of every floor underlay and overlay, for editor palettes.
//!
//! Only reads the two config groups and the texture definitions' average colour, so it is cheap next
//! to [`crate::WorldDefinitions::open`]. Ids are the values stored in map files (definition id + 1);
//! `0` means "none" and is not listed.

use crate::WorldError;
use osrs_cache::{
    decode::{
        DecoderContext, decode_floor_overlay, decode_floor_underlay, decode_terrain,
        decode_texture_definition,
    },
    transport::CacheRepository,
};
use osrs_core::{
    color_palette::build_color_palette,
    coords::RegionCoord,
    ids::{FloorOverlayId, FloorUnderlayId, TextureId},
};

const CONFIG_INDEX: u8 = 2;
const UNDERLAY_GROUP: u32 = 1;
const OVERLAY_GROUP: u32 = 4;
const TEXTURE_INDEX: u8 = 9;

/// One underlay as a palette entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnderlaySwatch {
    /// Value stored in the map file.
    pub id: u16,
    pub rgb: [u8; 3],
    /// Tiles of the whole map (all planes) that use it.
    pub uses: u64,
}

/// One overlay as a palette entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OverlaySwatch {
    /// Value stored in the map file.
    pub id: u16,
    pub rgb: [u8; 3],
    /// Raw primary RGB colour from opcode 1 (`primaryRgb`), even when textured.
    pub primary_rgb: [u8; 3],
    /// Secondary/minimap RGB colour from opcode 5 (`secondaryRgb`), when present.
    pub secondary_rgb: Option<[u8; 3]>,
    /// Texture drawn instead of the colour, when any.
    pub texture: Option<u32>,
    /// The overlay hides the underlay beneath it.
    pub hide_underlay: bool,
    /// Tiles of the whole map (all planes) that use it.
    pub uses: u64,
    /// Tiles that use this overlay with a shaped border (`shape in 1..=11`).
    pub shaped_uses: u64,
    /// Top 3 `(underlay_id, tile_count)` co-occurring on the same tile when `shape in 1..=11`.
    pub companion_underlays: [(u16, u32); 3],
}

impl OverlaySwatch {
    /// Most frequent underlay found underneath shaped (`1..=11`) tiles of this overlay.
    pub const fn top_companion_underlay(&self) -> Option<u16> {
        let (id, count) = self.companion_underlays[0];
        if id != 0 && count > 0 { Some(id) } else { None }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FloorPalette {
    pub underlays: Vec<UnderlaySwatch>,
    pub overlays: Vec<OverlaySwatch>,
}

fn split_rgb(rgb: i32) -> [u8; 3] {
    [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8]
}

fn top_three_companions(counts: &std::collections::HashMap<u16, u32>) -> [(u16, u32); 3] {
    let mut pairs: Vec<(u16, u32)> = counts
        .iter()
        .filter(|&(&id, &count)| id != 0 && count > 0)
        .map(|(&id, &count)| (id, count))
        .collect();
    pairs.sort_by_key(|&(id, count)| (std::cmp::Reverse(count), id));
    let mut out = [(0, 0); 3];
    for (slot, pair) in out.iter_mut().zip(pairs) {
        *slot = pair;
    }
    out
}

/// Read the palette from the pinned cache at `cache_dir`.
pub fn load_floor_palette(
    cache: impl Into<osrs_cache::source::CacheSource>,
) -> Result<FloorPalette, WorldError> {
    let profiles = osrs_cache::registry::bundled_profiles()?;
    let (cache, profile) = CacheRepository::open_matching(cache, &profiles)?;
    let context = DecoderContext::from_profile(&profile)?;
    let colors = build_color_palette(0.8);

    let mut palette = FloorPalette::default();
    for (id, file) in cache.read_group_files(CONFIG_INDEX, UNDERLAY_GROUP)? {
        let definition = decode_floor_underlay(
            FloorUnderlayId::new(id),
            &file.bytes,
            &context,
            &file.provenance,
        )
        .map_err(WorldError::Decode)?;
        palette.underlays.push(UnderlaySwatch {
            id: (id + 1) as u16,
            rgb: split_rgb(definition.rgb.get() as i32),
            uses: 0,
        });
    }
    let mut texture_colors = std::collections::HashMap::new();
    for (id, file) in cache.read_group_files(TEXTURE_INDEX, 0)? {
        let definition =
            decode_texture_definition(TextureId::new(id), &file.bytes, &context, &file.provenance)
                .map_err(WorldError::Decode)?;
        let rgb = colors
            .get(usize::from(definition.average_rgb))
            .copied()
            .unwrap_or(0);
        texture_colors.insert(id, split_rgb(rgb));
    }
    for (id, file) in cache.read_group_files(CONFIG_INDEX, OVERLAY_GROUP)? {
        let definition = decode_floor_overlay(
            FloorOverlayId::new(id),
            &file.bytes,
            &context,
            &file.provenance,
        )
        .map_err(WorldError::Decode)?;
        let texture = definition.texture.map(|texture| texture.get());
        let primary_rgb = split_rgb(definition.primary_rgb.get() as i32);
        let secondary_rgb = definition
            .secondary_rgb
            .map(|rgb| split_rgb(rgb.get() as i32));
        let rgb = texture
            .and_then(|texture| texture_colors.get(&texture).copied())
            .unwrap_or(primary_rgb);
        palette.overlays.push(OverlaySwatch {
            id: (id + 1) as u16,
            rgb,
            primary_rgb,
            secondary_rgb,
            texture,
            hide_underlay: definition.hide_underlay,
            uses: 0,
            shaped_uses: 0,
            companion_underlays: [(0, 0); 3],
        });
    }
    // How many tiles of the real map use each floor (a second or so for the whole cache).
    let mut overlay_uses: std::collections::HashMap<u16, u64> = std::collections::HashMap::new();
    let mut overlay_shaped_uses: std::collections::HashMap<u16, u64> =
        std::collections::HashMap::new();
    let mut overlay_companions: std::collections::HashMap<
        u16,
        std::collections::HashMap<u16, u32>,
    > = std::collections::HashMap::new();
    let mut underlay_uses: std::collections::HashMap<u16, u64> = std::collections::HashMap::new();
    for x in 0..=120 {
        for y in 0..=220 {
            let region = RegionCoord::new(x, y);
            let Ok(square) = cache.read_map_square(region) else {
                continue;
            };
            let Ok(terrain) = decode_terrain(
                &square.terrain.bytes,
                &context,
                &square.terrain.provenance,
                region,
            ) else {
                continue;
            };
            for tile in terrain.tiles() {
                if let Some(overlay) = tile.overlay {
                    let ov_id = overlay.encoded_id & 0x7fff;
                    *overlay_uses.entry(ov_id).or_default() += 1;
                    if (1..=11).contains(&overlay.shape) {
                        *overlay_shaped_uses.entry(ov_id).or_default() += 1;
                        if tile.underlay_id != 0 {
                            *overlay_companions
                                .entry(ov_id)
                                .or_default()
                                .entry(tile.underlay_id)
                                .or_default() += 1;
                        }
                    }
                }
                if tile.underlay_id != 0 {
                    *underlay_uses.entry(tile.underlay_id).or_default() += 1;
                }
            }
        }
    }
    for swatch in &mut palette.overlays {
        swatch.uses = overlay_uses.get(&swatch.id).copied().unwrap_or(0);
        swatch.shaped_uses = overlay_shaped_uses.get(&swatch.id).copied().unwrap_or(0);
        if let Some(counts) = overlay_companions.get(&swatch.id) {
            swatch.companion_underlays = top_three_companions(counts);
        }
    }
    for swatch in &mut palette.underlays {
        swatch.uses = underlay_uses.get(&swatch.id).copied().unwrap_or(0);
    }
    palette.underlays.sort_by_key(|swatch| swatch.id);
    palette.overlays.sort_by_key(|swatch| swatch.id);
    Ok(palette)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_three_companions_ranks_by_count_then_lowest_id() {
        let mut counts = std::collections::HashMap::new();
        counts.insert(10, 50);
        counts.insert(3, 120);
        counts.insert(7, 120);
        counts.insert(2, 10);
        let top = top_three_companions(&counts);
        assert_eq!(top, [(3, 120), (7, 120), (10, 50)]);

        let swatch = OverlaySwatch {
            id: 5,
            rgb: [100, 100, 100],
            primary_rgb: [100, 100, 100],
            secondary_rgb: Some([120, 110, 90]),
            texture: None,
            hide_underlay: true,
            uses: 500,
            shaped_uses: 290,
            companion_underlays: top,
        };
        assert_eq!(swatch.top_companion_underlay(), Some(3));
    }
}
