//! Display colours of every floor underlay and overlay, for editor palettes.
//!
//! Only reads the two config groups and the texture definitions' average colour, so it is cheap next
//! to [`crate::WorldDefinitions::open`]. Ids are the values stored in map files (definition id + 1);
//! `0` means "none" and is not listed.

use crate::WorldError;
use osrs_cache::{
    decode::{
        DecoderContext, decode_floor_overlay, decode_floor_underlay, decode_texture_definition,
    },
    profile::TargetProfile,
    transport::CacheRepository,
};
use osrs_core::{
    color_palette::build_color_palette,
    ids::{FloorOverlayId, FloorUnderlayId, TextureId},
};
use std::path::Path;

const PROFILE_YAML: &str =
    include_str!("../../../profiles/osrs-live-241-2026-09-30-openrs2-2727.yaml");
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
}

/// One overlay as a palette entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OverlaySwatch {
    /// Value stored in the map file.
    pub id: u16,
    pub rgb: [u8; 3],
    /// Texture drawn instead of the colour, when any.
    pub texture: Option<u32>,
    /// The overlay hides the underlay beneath it.
    pub hide_underlay: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FloorPalette {
    pub underlays: Vec<UnderlaySwatch>,
    pub overlays: Vec<OverlaySwatch>,
}

fn split_rgb(rgb: i32) -> [u8; 3] {
    [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8]
}

/// Read the palette from the pinned cache at `cache_dir`.
pub fn load_floor_palette(cache_dir: impl AsRef<Path>) -> Result<FloorPalette, WorldError> {
    let profile = TargetProfile::from_yaml_str(PROFILE_YAML)?;
    let cache = CacheRepository::open(cache_dir, &profile)?;
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
        let rgb = texture
            .and_then(|texture| texture_colors.get(&texture).copied())
            .unwrap_or_else(|| split_rgb(definition.primary_rgb.get() as i32));
        palette.overlays.push(OverlaySwatch {
            id: (id + 1) as u16,
            rgb,
            texture,
            hide_underlay: definition.hide_underlay,
        });
    }
    palette.underlays.sort_by_key(|swatch| swatch.id);
    palette.overlays.sort_by_key(|swatch| swatch.id);
    Ok(palette)
}
