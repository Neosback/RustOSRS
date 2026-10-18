//! The twelve overlay shapes of an OSRS tile, as triangles built from the real terrain tables so
//! thumbnails, the shape atlas and the renderer cannot drift apart.

use crate::{
    terrain::{ShapedTerrainInput, ShapedTerrainSurface, TerrainColorSource, TerrainCorners},
    terrain_appearance::shape_row,
};
use osrs_core::coords::SceneTile;

/// Triangles of one overlay shape on a unit square (`y` up) and whether each is overlay
/// (otherwise underlay).
pub fn shape_triangles(shape: u8, rotation: u8) -> Vec<([[f32; 2]; 3], bool)> {
    let flat = TerrainCorners::new(0, 0, 0, 0);
    // The builder indexes its tables with the shape row (rows 0 and 1 are "all underlay" and
    // "all overlay").
    let Ok(surface) = ShapedTerrainSurface::build(ShapedTerrainInput {
        shape: shape_row(shape) as u8,
        rotation: rotation & 3,
        texture_id: None,
        tile: SceneTile::new(0, 0),
        heights: flat,
        underlay_colors: flat,
        overlay_colors: flat,
        underlay_rgb: 0,
        overlay_rgb: 0,
    }) else {
        return Vec::new();
    };
    let point = |index: usize| {
        let position = surface.vertices[index].position;
        [
            position.x.units() as f32 / 128.0,
            position.z.units() as f32 / 128.0,
        ]
    };
    surface
        .faces
        .iter()
        .map(|face| {
            (
                [
                    point(face.indices[0]),
                    point(face.indices[1]),
                    point(face.indices[2]),
                ],
                face.color_source == TerrainColorSource::Overlay,
            )
        })
        .collect()
}

/// The overlay triangles of every shape and rotation, built once (the path preview draws
/// thousands of tiles a frame).
pub fn overlay_triangles(shape: u8, rotation: u8) -> &'static [[[f32; 2]; 3]] {
    use std::sync::OnceLock;
    static CACHE: OnceLock<Vec<Vec<[[f32; 2]; 3]>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| {
        (0..12_u8)
            .flat_map(|shape| (0..4_u8).map(move |rotation| (shape, rotation)))
            .map(|(shape, rotation)| {
                shape_triangles(shape, rotation)
                    .into_iter()
                    .filter(|(_, overlay)| *overlay)
                    .map(|(triangle, _)| triangle)
                    .collect()
            })
            .collect()
    });
    &cache[(usize::from(shape.min(11)) << 2) + usize::from(rotation & 3)]
}
