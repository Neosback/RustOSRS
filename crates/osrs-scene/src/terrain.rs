//! Exact semantic terrain surface construction for M6.

use osrs_core::coords::{
    HALF_TILE, LOCAL_UNITS_PER_TILE, LocalCoord, LocalPoint, QUARTER_TILE, SceneTile,
    THREE_QUARTER_TILE,
};
use std::{error::Error, fmt};

const VERTEX_CODES: [&[u8]; 13] = [
    &[1, 3, 5, 7],
    &[1, 3, 5, 7],
    &[1, 3, 5, 7],
    &[1, 3, 5, 7, 6],
    &[1, 3, 5, 7, 6],
    &[1, 3, 5, 7, 6],
    &[1, 3, 5, 7, 6],
    &[1, 3, 5, 7, 2, 6],
    &[1, 3, 5, 7, 2, 8],
    &[1, 3, 5, 7, 2, 8],
    &[1, 3, 5, 7, 11, 12],
    &[1, 3, 5, 7, 11, 12],
    &[1, 3, 5, 7, 13, 14],
];

const FACE_CODES: [&[u8]; 13] = [
    &[0, 1, 2, 3, 0, 0, 1, 3],
    &[1, 1, 2, 3, 1, 0, 1, 3],
    &[0, 1, 2, 3, 1, 0, 1, 3],
    &[0, 0, 1, 2, 0, 0, 2, 4, 1, 0, 4, 3],
    &[0, 0, 1, 4, 0, 0, 4, 3, 1, 1, 2, 4],
    &[0, 0, 4, 3, 1, 0, 1, 2, 1, 0, 2, 4],
    &[0, 1, 2, 4, 1, 0, 1, 4, 1, 0, 4, 3],
    &[0, 4, 1, 2, 0, 4, 2, 5, 1, 0, 4, 5, 1, 0, 5, 3],
    &[0, 4, 1, 2, 0, 4, 2, 3, 0, 4, 3, 5, 1, 0, 4, 5],
    &[0, 0, 4, 5, 1, 4, 1, 2, 1, 4, 2, 3, 1, 4, 3, 5],
    &[
        0, 0, 1, 5, 0, 1, 4, 5, 0, 1, 2, 4, 1, 0, 5, 3, 1, 5, 4, 3, 1, 4, 2, 3,
    ],
    &[
        1, 0, 1, 5, 1, 1, 4, 5, 1, 1, 2, 4, 0, 0, 5, 3, 0, 5, 4, 3, 0, 4, 2, 3,
    ],
    &[
        1, 0, 5, 4, 1, 0, 1, 5, 0, 0, 4, 3, 0, 4, 5, 3, 0, 5, 2, 3, 0, 1, 2, 5,
    ],
];

/// Four corner values in semantic tile order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerrainCorners<T> {
    pub southwest: T,
    pub southeast: T,
    pub northeast: T,
    pub northwest: T,
}

impl<T> TerrainCorners<T> {
    pub const fn new(southwest: T, southeast: T, northeast: T, northwest: T) -> Self {
        Self {
            southwest,
            southeast,
            northeast,
            northwest,
        }
    }
}

/// Which source color family owns a shaped-terrain face.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainColorSource {
    Underlay,
    Overlay,
}

/// One generated shaped-terrain vertex in canonical semantic coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerrainVertex {
    pub position: LocalPoint,
    pub underlay_color: i32,
    pub overlay_color: i32,
}

/// One generated shaped-terrain face.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerrainFace {
    pub indices: [usize; 3],
    pub colors: [i32; 3],
    pub color_source: TerrainColorSource,
    pub texture_id: Option<i32>,
}

/// Exact semantic output of the audited `SceneTileModel` construction path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShapedTerrainSurface {
    pub shape: u8,
    pub rotation: u8,
    pub vertices: Vec<TerrainVertex>,
    pub faces: Vec<TerrainFace>,
    pub is_flat: bool,
    pub underlay_rgb: i32,
    pub overlay_rgb: i32,
}

/// Inputs required to construct one shaped terrain tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShapedTerrainInput {
    pub shape: u8,
    pub rotation: u8,
    pub texture_id: Option<i32>,
    pub tile: SceneTile,
    pub heights: TerrainCorners<i32>,
    pub underlay_colors: TerrainCorners<i32>,
    pub overlay_colors: TerrainCorners<i32>,
    pub underlay_rgb: i32,
    pub overlay_rgb: i32,
}

/// Flat-paint terrain corner identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainCorner {
    Southwest,
    Southeast,
    Northeast,
    Northwest,
}

/// Semantic flat terrain data. Renderer packing is deliberately absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlatTerrainSurface {
    pub heights: TerrainCorners<i32>,
    pub colors: TerrainCorners<i32>,
    pub texture_id: Option<i32>,
    pub is_flat: bool,
}

impl FlatTerrainSurface {
    pub const REFERENCE_TRIANGLES: [[TerrainCorner; 3]; 2] = [
        [
            TerrainCorner::Northeast,
            TerrainCorner::Northwest,
            TerrainCorner::Southeast,
        ],
        [
            TerrainCorner::Southwest,
            TerrainCorner::Southeast,
            TerrainCorner::Northwest,
        ],
    ];

    pub const fn new(
        heights: TerrainCorners<i32>,
        colors: TerrainCorners<i32>,
        texture_id: Option<i32>,
    ) -> Self {
        let is_flat = heights.southeast == heights.southwest
            && heights.northeast == heights.southwest
            && heights.northwest == heights.southwest;
        Self {
            heights,
            colors,
            texture_id,
            is_flat,
        }
    }
}

/// Mutually exclusive semantic terrain representation for one scene tile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerrainSurface {
    Flat(FlatTerrainSurface),
    Shaped(ShapedTerrainSurface),
}

/// Construction failures are explicit rather than silently normalizing bad input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainBuildError {
    InvalidShape(u8),
    InvalidRotation(u8),
    CoordinateOverflow,
}

impl fmt::Display for TerrainBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidShape(shape) => {
                write!(formatter, "terrain shape {shape} is outside 0..=12")
            }
            Self::InvalidRotation(rotation) => {
                write!(formatter, "terrain rotation {rotation} is outside 0..=3")
            }
            Self::CoordinateOverflow => formatter.write_str("terrain semantic coordinate overflow"),
        }
    }
}

impl Error for TerrainBuildError {}

impl ShapedTerrainSurface {
    /// Reproduce the pinned `SceneTileModel` integer construction exactly.
    pub fn build(input: ShapedTerrainInput) -> Result<Self, TerrainBuildError> {
        let vertex_codes = VERTEX_CODES
            .get(usize::from(input.shape))
            .ok_or(TerrainBuildError::InvalidShape(input.shape))?;
        if input.rotation > 3 {
            return Err(TerrainBuildError::InvalidRotation(input.rotation));
        }

        let base_x = tile_axis_origin(input.tile.x)?;
        let base_z = tile_axis_origin(input.tile.y)?;
        let mut vertices = Vec::with_capacity(vertex_codes.len());
        for &template_code in *vertex_codes {
            let code = rotated_vertex_code(template_code, input.rotation);
            vertices.push(build_vertex(code, base_x, base_z, input));
        }

        let face_codes = FACE_CODES[usize::from(input.shape)];
        let mut faces = Vec::with_capacity(face_codes.len() / 4);
        for chunk in face_codes.chunks_exact(4) {
            let source = if chunk[0] == 0 {
                TerrainColorSource::Underlay
            } else {
                TerrainColorSource::Overlay
            };
            let indices = [
                rotated_face_index(chunk[1], input.rotation),
                rotated_face_index(chunk[2], input.rotation),
                rotated_face_index(chunk[3], input.rotation),
            ];
            let colors = match source {
                TerrainColorSource::Underlay => [
                    vertices[indices[0]].underlay_color,
                    vertices[indices[1]].underlay_color,
                    vertices[indices[2]].underlay_color,
                ],
                TerrainColorSource::Overlay => [
                    vertices[indices[0]].overlay_color,
                    vertices[indices[1]].overlay_color,
                    vertices[indices[2]].overlay_color,
                ],
            };
            let texture_id = match source {
                TerrainColorSource::Underlay => None,
                TerrainColorSource::Overlay => input.texture_id,
            };
            faces.push(TerrainFace {
                indices,
                colors,
                color_source: source,
                texture_id,
            });
        }

        let is_flat = input.heights.southeast == input.heights.southwest
            && input.heights.northeast == input.heights.southwest
            && input.heights.northwest == input.heights.southwest;

        Ok(Self {
            shape: input.shape,
            rotation: input.rotation,
            vertices,
            faces,
            is_flat,
            underlay_rgb: input.underlay_rgb,
            overlay_rgb: input.overlay_rgb,
        })
    }
}

fn tile_axis_origin(tile: u32) -> Result<i32, TerrainBuildError> {
    i32::try_from(tile)
        .ok()
        .and_then(|value| value.checked_mul(LOCAL_UNITS_PER_TILE))
        .ok_or(TerrainBuildError::CoordinateOverflow)
}

fn rotated_vertex_code(code: u8, rotation: u8) -> u8 {
    let rotation = i32::from(rotation);
    let mut value = i32::from(code);
    if value & 1 == 0 && value <= 8 {
        value = ((value - rotation - rotation - 1) & 7) + 1;
    }
    if value > 8 && value <= 12 {
        value = ((value - 9 - rotation) & 3) + 9;
    }
    if value > 12 && value <= 16 {
        value = ((value - 13 - rotation) & 3) + 13;
    }
    value as u8
}

fn rotated_face_index(index: u8, rotation: u8) -> usize {
    if index < 4 {
        usize::from(((i32::from(index) - i32::from(rotation)) & 3) as u8)
    } else {
        usize::from(index)
    }
}

fn java_average(left: i32, right: i32) -> i32 {
    left.wrapping_add(right) >> 1
}

fn build_vertex(code: u8, base_x: i32, base_z: i32, input: ShapedTerrainInput) -> TerrainVertex {
    let h = input.heights;
    let u = input.underlay_colors;
    let o = input.overlay_colors;

    let (x, z, y, underlay_color, overlay_color) = match code {
        1 => (base_x, base_z, h.southwest, u.southwest, o.southwest),
        2 => (
            base_x + HALF_TILE,
            base_z,
            java_average(h.southeast, h.southwest),
            java_average(u.southeast, u.southwest),
            java_average(o.southeast, o.southwest),
        ),
        3 => (
            base_x + LOCAL_UNITS_PER_TILE,
            base_z,
            h.southeast,
            u.southeast,
            o.southeast,
        ),
        4 => (
            base_x + LOCAL_UNITS_PER_TILE,
            base_z + HALF_TILE,
            java_average(h.northeast, h.southeast),
            java_average(u.southeast, u.northeast),
            java_average(o.southeast, o.northeast),
        ),
        5 => (
            base_x + LOCAL_UNITS_PER_TILE,
            base_z + LOCAL_UNITS_PER_TILE,
            h.northeast,
            u.northeast,
            o.northeast,
        ),
        6 => (
            base_x + HALF_TILE,
            base_z + LOCAL_UNITS_PER_TILE,
            java_average(h.northeast, h.northwest),
            java_average(u.northwest, u.northeast),
            java_average(o.northwest, o.northeast),
        ),
        7 => (
            base_x,
            base_z + LOCAL_UNITS_PER_TILE,
            h.northwest,
            u.northwest,
            o.northwest,
        ),
        8 => (
            base_x,
            base_z + HALF_TILE,
            java_average(h.northwest, h.southwest),
            java_average(u.northwest, u.southwest),
            java_average(o.northwest, o.southwest),
        ),
        9 => (
            base_x + HALF_TILE,
            base_z + QUARTER_TILE,
            java_average(h.southeast, h.southwest),
            java_average(u.southeast, u.southwest),
            java_average(o.southeast, o.southwest),
        ),
        10 => (
            base_x + THREE_QUARTER_TILE,
            base_z + HALF_TILE,
            java_average(h.northeast, h.southeast),
            java_average(u.southeast, u.northeast),
            java_average(o.southeast, o.northeast),
        ),
        11 => (
            base_x + HALF_TILE,
            base_z + THREE_QUARTER_TILE,
            java_average(h.northeast, h.northwest),
            java_average(u.northwest, u.northeast),
            java_average(o.northwest, o.northeast),
        ),
        12 => (
            base_x + QUARTER_TILE,
            base_z + HALF_TILE,
            java_average(h.northwest, h.southwest),
            java_average(u.northwest, u.southwest),
            java_average(o.northwest, o.southwest),
        ),
        13 => (
            base_x + QUARTER_TILE,
            base_z + QUARTER_TILE,
            h.southwest,
            u.southwest,
            o.southwest,
        ),
        14 => (
            base_x + THREE_QUARTER_TILE,
            base_z + QUARTER_TILE,
            h.southeast,
            u.southeast,
            o.southeast,
        ),
        15 => (
            base_x + THREE_QUARTER_TILE,
            base_z + THREE_QUARTER_TILE,
            h.northeast,
            u.northeast,
            o.northeast,
        ),
        16 => (
            base_x + QUARTER_TILE,
            base_z + THREE_QUARTER_TILE,
            h.northwest,
            u.northwest,
            o.northwest,
        ),
        _ => unreachable!("SceneTileModel vertex templates only emit codes 1..=16"),
    };

    TerrainVertex {
        position: LocalPoint::new(
            LocalCoord::from_units(x),
            LocalCoord::from_units(y),
            LocalCoord::from_units(z),
        ),
        underlay_color,
        overlay_color,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_shape_and_rotation_fail_explicitly() {
        let base = ShapedTerrainInput {
            shape: 0,
            rotation: 0,
            texture_id: None,
            tile: SceneTile::new(0, 0),
            heights: TerrainCorners::new(0, 0, 0, 0),
            underlay_colors: TerrainCorners::new(1, 2, 3, 4),
            overlay_colors: TerrainCorners::new(5, 6, 7, 8),
            underlay_rgb: 0,
            overlay_rgb: 0,
        };
        assert_eq!(
            ShapedTerrainSurface::build(ShapedTerrainInput { shape: 13, ..base }),
            Err(TerrainBuildError::InvalidShape(13))
        );
        assert_eq!(
            ShapedTerrainSurface::build(ShapedTerrainInput {
                rotation: 4,
                ..base
            }),
            Err(TerrainBuildError::InvalidRotation(4))
        );
    }

    #[test]
    fn overlay_texture_applies_only_to_overlay_owned_faces() {
        let surface = ShapedTerrainSurface::build(ShapedTerrainInput {
            shape: 2,
            rotation: 0,
            texture_id: Some(42),
            tile: SceneTile::new(0, 0),
            heights: TerrainCorners::new(0, 0, 0, 0),
            underlay_colors: TerrainCorners::new(10, 11, 12, 13),
            overlay_colors: TerrainCorners::new(20, 21, 22, 23),
            underlay_rgb: 0,
            overlay_rgb: 0,
        })
        .expect("valid shape");
        assert_eq!(surface.faces[0].texture_id, None);
        assert_eq!(surface.faces[1].texture_id, Some(42));
    }

    #[test]
    fn flat_surface_preserves_reference_diagonal_and_sentinel() {
        let flat = FlatTerrainSurface::new(
            TerrainCorners::new(10, 20, 30, 40),
            TerrainCorners::new(1, 2, 12_345_678, 4),
            None,
        );
        assert_eq!(
            FlatTerrainSurface::REFERENCE_TRIANGLES,
            [
                [
                    TerrainCorner::Northeast,
                    TerrainCorner::Northwest,
                    TerrainCorner::Southeast,
                ],
                [
                    TerrainCorner::Southwest,
                    TerrainCorner::Southeast,
                    TerrainCorner::Northwest,
                ],
            ]
        );
        assert_eq!(flat.colors.northeast, 12_345_678);
        assert!(!flat.is_flat);
    }
}
