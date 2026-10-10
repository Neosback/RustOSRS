//! Cache-backed definition access for scene assembly.

use crate::WorldError;
use osrs_cache::{
    decode::{
        ArchiveFileProvenance, DecodedLocations, DecoderContext, decode_floor_overlay,
        decode_floor_underlay, decode_locations, decode_object_definition, decode_sprite_group,
        decode_terrain, decode_texture_definition,
    },
    model_repository::ModelSourceRepository,
    object_model::resolve_object_model,
    profile::TargetProfile,
    transport::{CacheErrorKind, CacheFile, CacheRepository},
};
use osrs_core::{
    coords::RegionCoord,
    definitions::{LocType, ObjectDefinition},
    floor_color::UnderlayHsl,
    ids::{FloorOverlayId, FloorUnderlayId, ObjectId, TextureId},
    model_construction::AssembledModel,
};
use osrs_scene::{
    terrain_build::{FloorLookup, OverlayFloor},
    terrain_load::{RegionTerrain, RegionTerrainTile, RegionTileHeight, RegionTileOverlay},
};
use std::{collections::HashMap, path::Path, sync::Arc};

const PROFILE_YAML: &str =
    include_str!("../../../profiles/osrs-live-241-2026-09-30-openrs2-2727.yaml");
const CONFIG_INDEX: u8 = 2;
const UNDERLAY_GROUP: u32 = 1;
const OVERLAY_GROUP: u32 = 4;
const OBJECT_GROUP: u32 = 6;
const TEXTURE_INDEX: u8 = 9;
const SPRITE_INDEX: u8 = 8;
/// Reference texture edge length (the RuneLite array is 128x128).
pub const TEXTURE_SIZE: usize = 128;

/// Floor and texture inputs for the terrain builder, loaded eagerly (a few hundred small files).
#[derive(Debug, Default)]
pub struct FloorTable {
    underlays: HashMap<u32, UnderlayHsl>,
    overlays: HashMap<u32, OverlayFloor>,
    texture_averages: HashMap<i32, i32>,
}

impl FloorLookup for FloorTable {
    fn underlay(&self, index: u32) -> UnderlayHsl {
        self.underlays.get(&index).copied().unwrap_or(UnderlayHsl {
            weighted_hue: 0,
            saturation: 0,
            lightness: 0,
            hue_multiplier: 1,
        })
    }

    fn overlay(&self, index: u32) -> OverlayFloor {
        self.overlays
            .get(&index)
            .copied()
            .unwrap_or_else(OverlayFloor::missing)
    }

    fn texture_average_rgb(&self, texture_id: i32) -> i32 {
        // The client returns 0 for a texture it does not have.
        self.texture_averages.get(&texture_id).copied().unwrap_or(0)
    }
}

/// One texture image in the reference upload format: 128x128, 8 bits per channel, RGBA.
///
/// Source palette index `0` (and any pixel whose RGB is `0`) is uploaded as all-zero transparent
/// RGBA; every other pixel gets alpha `255` (`TextureManager.convertPixels`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureImage {
    pub texture_id: u32,
    /// `TEXTURE_SIZE * TEXTURE_SIZE` pixels, row-major, bytes `[r, g, b, a]`.
    pub rgba: Vec<[u8; 4]>,
    /// Animation direction (`0` none, `1..=4`) and speed from the texture definition.
    pub animation_direction: u8,
    pub animation_speed: u8,
}

/// All build-241 texture images, indexed by texture id.
#[derive(Debug, Default)]
pub struct TextureTable {
    pub images: Vec<Option<TextureImage>>,
}

impl TextureTable {
    /// Number of array layers needed (highest texture id plus one).
    pub fn layer_count(&self) -> usize {
        self.images.len()
    }

    pub fn image(&self, texture_id: u32) -> Option<&TextureImage> {
        self.images
            .get(texture_id as usize)
            .and_then(Option::as_ref)
    }
}

/// One region's decoded map files.
#[derive(Debug)]
pub struct RegionMap {
    pub terrain: RegionTerrain,
    pub locations: DecodedLocations,
}

/// Definitions and map data read from the pinned build-241 cache.
pub struct WorldDefinitions {
    models: ModelSourceRepository,
    context: DecoderContext,
    floors: FloorTable,
    textures: TextureTable,
    object_files: HashMap<u32, CacheFile>,
    objects: HashMap<u32, Option<Arc<ObjectDefinition>>>,
}

impl WorldDefinitions {
    /// Open the pinned cache at `cache_dir` and load the floor and texture tables.
    pub fn open(cache_dir: impl AsRef<Path>) -> Result<Self, WorldError> {
        let profile = TargetProfile::from_yaml_str(PROFILE_YAML)?;
        let cache = CacheRepository::open(cache_dir, &profile)?;
        let context = DecoderContext::from_profile(&profile)?;

        let mut floors = FloorTable::default();
        for (id, file) in cache.read_group_files(CONFIG_INDEX, UNDERLAY_GROUP)? {
            let definition = decode_floor_underlay(
                FloorUnderlayId::new(id),
                &file.bytes,
                &context,
                &file.provenance,
            )
            .map_err(WorldError::Decode)?;
            floors.underlays.insert(id, definition.hsl);
        }
        for (id, file) in cache.read_group_files(CONFIG_INDEX, OVERLAY_GROUP)? {
            let definition = decode_floor_overlay(
                FloorOverlayId::new(id),
                &file.bytes,
                &context,
                &file.provenance,
            )
            .map_err(WorldError::Decode)?;
            floors.overlays.insert(id, OverlayFloor::from(&definition));
        }
        for (id, file) in cache.read_group_files(TEXTURE_INDEX, 0)? {
            let definition = decode_texture_definition(
                TextureId::new(id),
                &file.bytes,
                &context,
                &file.provenance,
            )
            .map_err(WorldError::Decode)?;
            floors
                .texture_averages
                .insert(id as i32, i32::from(definition.average_rgb));
        }

        let mut textures = TextureTable::default();
        for (id, file) in cache.read_group_files(TEXTURE_INDEX, 0)? {
            let definition = decode_texture_definition(
                TextureId::new(id),
                &file.bytes,
                &context,
                &file.provenance,
            )
            .map_err(WorldError::Decode)?;
            let Some(sprite_id) = definition.source_sprites.first() else {
                continue;
            };
            let sprite_group = cache.read_file(SPRITE_INDEX, sprite_id.get(), 0, None)?;
            let frames = decode_sprite_group(
                sprite_id.get(),
                &sprite_group.bytes,
                &context,
                &sprite_group.provenance,
            )
            .map_err(WorldError::Decode)?;
            let Some(frame) = frames.first() else {
                continue;
            };
            let indices = frame.normalized_indices();
            let source_size = usize::from(frame.max_width);
            let mut rgba = Vec::with_capacity(TEXTURE_SIZE * TEXTURE_SIZE);
            for row in 0..TEXTURE_SIZE {
                for column in 0..TEXTURE_SIZE {
                    // 64x64 sources are doubled with nearest sampling; 128x128 map 1:1.
                    let (source_row, source_column) = if source_size == TEXTURE_SIZE {
                        (row, column)
                    } else {
                        (row >> 1, column >> 1)
                    };
                    let index = indices
                        .get(source_row * source_size + source_column)
                        .copied()
                        .unwrap_or(0);
                    let rgb = frame.palette.get(usize::from(index)).copied().unwrap_or(0);
                    rgba.push(if rgb == 0 {
                        [0, 0, 0, 0]
                    } else {
                        [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, 255]
                    });
                }
            }
            let slot = id as usize;
            if textures.images.len() <= slot {
                textures.images.resize(slot + 1, None);
            }
            textures.images[slot] = Some(TextureImage {
                texture_id: id,
                rgba,
                animation_direction: definition.animation_direction,
                animation_speed: definition.animation_speed,
            });
        }

        let object_files = cache
            .read_group_files(CONFIG_INDEX, OBJECT_GROUP)?
            .into_iter()
            .collect();

        Ok(Self {
            models: ModelSourceRepository::new(cache),
            context,
            floors,
            textures,
            object_files,
            objects: HashMap::new(),
        })
    }

    pub fn cache(&self) -> &CacheRepository {
        self.models.cache_repository()
    }

    /// Select, mirror, combine, and transform the raw models for one object/type/orientation.
    /// `Ok(None)` is the exact semantic "no model" outcome.
    pub fn resolve_model(
        &mut self,
        definition: &ObjectDefinition,
        loc_type: LocType,
        orientation: u8,
    ) -> Result<Option<AssembledModel>, WorldError> {
        Ok(resolve_object_model(
            &mut self.models,
            definition,
            loc_type,
            orientation,
        )?)
    }

    pub fn context(&self) -> &DecoderContext {
        &self.context
    }

    pub fn floors(&self) -> &FloorTable {
        &self.floors
    }

    pub fn textures(&self) -> &TextureTable {
        &self.textures
    }

    /// Decode (and memoize) one object definition. `Ok(None)` means the id has no definition.
    pub fn object(&mut self, id: ObjectId) -> Result<Option<Arc<ObjectDefinition>>, WorldError> {
        if let Some(cached) = self.objects.get(&id.get()) {
            return Ok(cached.clone());
        }
        let definition = match self.object_files.get(&id.get()) {
            Some(file) => Some(Arc::new(
                decode_object_definition(id, &file.bytes, &self.context, &file.provenance)
                    .map_err(WorldError::Decode)?,
            )),
            None => None,
        };
        self.objects.insert(id.get(), definition.clone());
        Ok(definition)
    }

    /// Read and decode one region's terrain and location files, or `None` when the cache has no
    /// map square for it.
    pub fn region(&self, region: RegionCoord) -> Result<Option<RegionMap>, WorldError> {
        let square = match self.cache().read_map_square(region) {
            Ok(square) => square,
            Err(error) if matches!(error.kind(), CacheErrorKind::MapResolution { .. }) => {
                return Ok(None);
            }
            Err(error) => return Err(error.into()),
        };
        let terrain = decode_terrain(
            &square.terrain.bytes,
            &self.context,
            &square.terrain.provenance,
            region,
        )?;
        let locations = decode_locations(
            &square.locations.bytes,
            &self.context,
            &square.locations.provenance,
            region,
        )?;
        let tiles = terrain
            .tiles()
            .iter()
            .map(|tile| RegionTerrainTile {
                height: match tile.height {
                    osrs_cache::decode::EncodedTileHeight::Default => RegionTileHeight::Default,
                    osrs_cache::decode::EncodedTileHeight::Explicit(value) => {
                        RegionTileHeight::Explicit(value)
                    }
                },
                overlay: tile.overlay.map(|overlay| RegionTileOverlay {
                    raw_id: overlay.encoded_id,
                    shape: overlay.shape,
                    rotation: overlay.rotation,
                }),
                settings: (tile.settings != 0).then_some(tile.settings),
                underlay: tile.underlay_id,
            })
            .collect();
        Ok(Some(RegionMap {
            terrain: RegionTerrain::new(tiles)?,
            locations,
        }))
    }

    /// Provenance helper for diagnostics.
    pub fn object_provenance(id: ObjectId) -> ArchiveFileProvenance {
        ArchiveFileProvenance::new(CONFIG_INDEX, OBJECT_GROUP, Some(id.get()))
    }
}
