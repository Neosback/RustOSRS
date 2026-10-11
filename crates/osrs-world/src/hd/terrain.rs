//! Terrain emission for the HD profile: a port of `SceneUploader.uploadTilePaint` and
//! `uploadTileModel` that turns one terrain tile into [`HdFace`]s with blended colours, ground
//! materials, normals and water depth.
//!
//! Water surfaces are not emitted yet (phase 5): water tiles produce their underwater ground, with
//! the water depth of every vertex in the HD vertex data.
//!
//! Adapted from `rs117.hd.renderer.zone.SceneUploader`. Copyright (c) 2018, Adam
//! <Adam@sigterm.info>; (c) 2021, 117 and Hooder (BSD 2-Clause, see `THIRD_PARTY.md`).

use super::{
    HdContext,
    procedural::{
        HIDDEN_HSL, OverrideSlot, ProceduralData, TileFace, TileInput, UNDERWATER_HSL,
        VERTEX_OVERLAY, VERTEX_UNDERLAY, VertexKey, has_vanilla_texture, tile_geometry,
    },
};
use osrs_hd::tiles::{GroundMaterials, TileOverrides, WaterTypes};
use osrs_render::{HD_FLAG_TERRAIN, HD_MATERIAL_OVERLAY, HdFace};

const TILE: i32 = 128;
const JAU_TO_RAD: f32 = std::f32::consts::TAU / 2048.0;

/// Everything the emitter needs about the scene.
pub struct TerrainEmitter<'a> {
    pub ctx: &'a HdContext,
    pub data: &'a ProceduralData,
    /// World tile of scene tile `(0, 0)`.
    pub base: (i32, i32),
}

/// Per-tile inputs not stored in the terrain surface.
#[derive(Debug, Clone, Copy)]
pub struct TileSettings {
    /// The tile's floor is visible from the level below (`TILE_FLAG_VIS_BELOW`).
    pub visible_below: bool,
    /// Scene-local origin of the zone that owns the tile.
    pub zone_origin: (i32, i32),
}

fn fract(value: f32) -> f32 {
    value - value.floor()
}

impl TerrainEmitter<'_> {
    /// The HD faces of one tile.
    pub fn tile_faces(&self, tile: TileInput<'_>, settings: TileSettings) -> Vec<HdFace> {
        let geometry = tile_geometry(tile.surface, tile.x, tile.y);
        if geometry.corners.is_some() && geometry.corner_colors[3] == HIDDEN_HSL {
            return Vec::new();
        }
        let ctx = self.ctx;
        let world = (
            self.base.0 + tile.x,
            self.base.1 + tile.y,
            i32::from(tile.level),
        );
        let main = self.data.tile_override(tile.level, tile.x, tile.y, OverrideSlot::Main);
        let overlay = self.data.tile_override(tile.level, tile.x, tile.y, OverrideSlot::Overlay);
        let underlay = self.data.tile_override(tile.level, tile.x, tile.y, OverrideSlot::Underlay);
        let textured = has_vanilla_texture(tile.surface);
        let is_paint = geometry.corners.is_some();

        let mut faces = Vec::with_capacity(geometry.faces.len());
        for face in &geometry.faces {
            if !is_paint && face.colors[0] == HIDDEN_HSL {
                continue;
            }
            let found = if is_paint {
                main
            } else if face.overlay {
                overlay
            } else {
                underlay
            };
            let mut override_index = found.index;
            let water = ctx
                .water
                .seasonal(ctx.tile_overrides.get(override_index).water_type, face.texture);
            let mut colors = face.colors;
            let mut materials = [0_usize; 3];
            let mut depths = [0_i32; 3];
            let mut heights = face.keys.map(|key| key.1);
            let mut uv_orientation = 0;
            let mut uv_scale = 1.0;

            if water == WaterTypes::NONE {
                if face.texture != -1 {
                    let material = ctx.materials.from_vanilla_texture(face.texture);
                    // Newly introduced vanilla textures ignore tile overrides.
                    if ctx.materials.is_fallback_vanilla(material) {
                        override_index = TileOverrides::NONE;
                    }
                    materials = [material; 3];
                }
                let override_data = ctx.tile_overrides.get(override_index);
                let use_default_color = textured
                    || (override_index != TileOverrides::NONE && !override_data.blended);
                // Overlay faces of models are not blended when they keep the default colour.
                let blocked = if is_paint {
                    use_default_color
                } else {
                    face.overlay && use_default_color
                };
                let allow_blending =
                    ctx.settings.ground_blending() && face.texture == -1 && !blocked;
                let blend_colors = ctx.settings.ground_blending_colors && allow_blending;
                let blend_textures = ctx.settings.ground_blending_textures && allow_blending;

                let ground = if override_index != TileOverrides::NONE {
                    uv_orientation = override_data.uv_orientation;
                    uv_scale = override_data.uv_scale;
                    if !blend_colors {
                        colors = colors.map(|color| override_data.modify_color(color));
                    }
                    Some(override_data.ground_material)
                } else if face.texture == -1 {
                    Some(override_data.ground_material)
                } else {
                    None
                };
                if blend_colors {
                    for corner in 0..3 {
                        if let Some(color) = self
                            .data
                            .vertex(face.keys[corner])
                            .and_then(|vertex| vertex.color)
                        {
                            colors[corner] = color;
                        }
                    }
                }
                if ctx.settings.ground_textures {
                    if blend_textures {
                        for corner in 0..3 {
                            if let Some(material) = self
                                .data
                                .vertex(face.keys[corner])
                                .and_then(|vertex| vertex.material)
                            {
                                materials[corner] = usize::from(material);
                            }
                        }
                    } else if let Some(ground) = ground {
                        for corner in 0..3 {
                            let key = face.keys[corner];
                            materials[corner] = ctx.grounds.get(ground).random_material(
                                world.0 + ((key.0 - tile.x * TILE) >> 7),
                                world.1 + ((key.2 - tile.y * TILE) >> 7),
                                world.2,
                            );
                        }
                    }
                }
            } else {
                // Underwater geometry.
                colors = [UNDERWATER_HSL; 3];
                if ctx.settings.ground_textures {
                    let ground = ctx.grounds.get(GroundMaterials::UNDERWATER_GENERIC);
                    for corner in 0..3 {
                        let key = face.keys[corner];
                        materials[corner] = ground.random_material(
                            world.0 + ((key.0 - tile.x * TILE) >> 7),
                            world.1 + ((key.2 - tile.y * TILE) >> 7),
                            world.2,
                        );
                    }
                }
                for corner in 0..3 {
                    depths[corner] = self.data.underwater_depth(face.keys[corner]);
                    heights[corner] += depths[corner];
                }
            }

            let override_data = ctx.tile_overrides.get(override_index);
            for height in &mut heights {
                *height -= override_data.height_offset;
            }
            let packed_materials = [0, 1, 2].map(|corner| {
                let vertex = self.data.vertex(face.keys[corner]);
                let both = vertex.is_some_and(|v| {
                    v.is(VERTEX_OVERLAY) && v.is(VERTEX_UNDERLAY)
                }) && water == WaterTypes::NONE;
                let index = materials[corner].min(0x0FFF) as u16;
                if both { index | HD_MATERIAL_OVERLAY } else { index }
            });
            let uvs = self.face_uvs(face, tile, world, uv_orientation, uv_scale, is_paint);
            let positions = [0, 1, 2].map(|corner| {
                let key = face.keys[corner];
                [
                    key.0 - settings.zone_origin.0,
                    heights[corner],
                    key.2 - settings.zone_origin.1,
                ]
            });
            faces.push(HdFace {
                positions,
                uvs,
                normals: face.keys.map(|key| self.data.normal(key)),
                hsl: colors.map(|color| (color & 0xFFFF) as u16),
                material: packed_materials,
                water_depth: depths.map(|depth| depth.clamp(0, 0x0FFF).max(i32::from(water != WaterTypes::NONE)) as u16),
                water_type: water,
                plane: tile.level,
                flags: HD_FLAG_TERRAIN,
                alpha: 0,
                alpha_pass: false,
                bias: 0,
                double_sided: (!settings.visible_below && tile.level != 0)
                    || override_data.double_sided_faces,
            });
        }
        faces
    }

    /// Texture coordinates of a terrain triangle: world-aligned, so textures continue across
    /// tiles and zones. Paint tiles and tile models use the plugin's two slightly different forms.
    fn face_uvs(
        &self,
        face: &TileFace,
        tile: TileInput<'_>,
        world: (i32, i32, i32),
        orientation: i32,
        scale: f32,
        is_paint: bool,
    ) -> [[f32; 2]; 3] {
        let (mut cos, mut sin) = (-scale, 0.0);
        if orientation % 2048 != 0 {
            let radians = -(orientation as f32) * JAU_TO_RAD;
            cos = radians.cos() * -scale;
            sin = radians.sin() * -scale;
        }
        let uvx = fract(world.0 as f32 * cos - world.1 as f32 * sin);
        let uvy = fract(world.0 as f32 * sin + world.1 as f32 * cos);
        let (tile_x, tile_z) = (tile.x * TILE, tile.y * TILE);
        face.keys.map(|key: VertexKey| {
            let dx = (key.0 - tile_x) as f32 / TILE as f32;
            let dz = (key.2 - tile_z) as f32 / TILE as f32;
            if is_paint {
                // Relative to the north-east corner, which carries the base coordinate.
                let (ox, oz) = (1.0 - dx, 1.0 - dz);
                [uvx - ox * cos + oz * sin, uvy - ox * sin - oz * cos]
            } else {
                [uvx + dx * cos - dz * sin, uvy + dx * sin + dz * cos]
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fract_wraps_negative_values_into_the_unit_interval() {
        assert!((fract(-0.25) - 0.75).abs() < 1e-6);
        assert!((fract(3.5) - 0.5).abs() < 1e-6);
    }
}
