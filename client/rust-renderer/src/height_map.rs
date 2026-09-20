/// CPU reference for the WebGL height-map texture and contour interpolation.
///
/// The implementation mirrors client/render/shaders/includes/height-map.glsl.
/// Keeping a CPU reference makes the shader behavior testable while the renderer
/// is migrated incrementally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeightMap {
    size: usize,
    planes: usize,
    values: Vec<i16>,
}

impl HeightMap {
    pub fn new(size: usize, planes: usize, values: Vec<i16>) -> Result<Self, &'static str> {
        if size == 0 || planes == 0 {
            return Err("height-map dimensions must be positive");
        }
        let expected = size
            .checked_mul(size)
            .and_then(|value| value.checked_mul(planes))
            .ok_or("height-map dimensions overflow")?;
        if values.len() != expected {
            return Err("height-map packet length does not match dimensions");
        }
        Ok(Self { size, planes, values })
    }

    pub const fn size(&self) -> usize {
        self.size
    }

    pub const fn planes(&self) -> usize {
        self.planes
    }

    pub fn tile_height(&self, x: i32, z: i32, plane: u32, border: i32) -> i32 {
        let tx = clamp_i32(border + x, 0, self.size as i32 - 1) as usize;
        let tz = clamp_i32(border + z, 0, self.size as i32 - 1) as usize;
        let plane = (plane as usize).min(self.planes - 1);
        i32::from(self.values[(plane * self.size * self.size) + (tz * self.size) + tx]) * 8
    }

    /// Mirrors getHeightInterp() in height-map.glsl.
    pub fn interpolate(&self, x: f32, z: f32, plane: u32, border: i32) -> f32 {
        const TILE_SIZE: i32 = 128;
        const TILE_SHIFT: i32 = 7;

        let ix = x as i32;
        let iz = z as i32;
        let tile_x = ix >> TILE_SHIFT;
        let tile_z = iz >> TILE_SHIFT;
        let offset_x = ix & (TILE_SIZE - 1);
        let offset_z = iz & (TILE_SIZE - 1);

        let h_sw = self.tile_height(tile_x, tile_z, plane, border);
        let h_se = self.tile_height(tile_x + 1, tile_z, plane, border);
        let h_nw = self.tile_height(tile_x, tile_z + 1, plane, border);
        let h_ne = self.tile_height(tile_x + 1, tile_z + 1, plane, border);

        let h0 = if offset_x + offset_z <= TILE_SIZE {
            (h_sw * TILE_SIZE
                + (h_se - h_sw) * offset_x
                + (h_nw - h_sw) * offset_z)
                >> TILE_SHIFT
        } else {
            let rx = TILE_SIZE - offset_x;
            let rz = TILE_SIZE - offset_z;
            (h_ne * TILE_SIZE
                + (h_nw - h_ne) * rx
                + (h_se - h_ne) * rz)
                >> TILE_SHIFT
        };

        let h1 = if offset_x <= offset_z {
            (h_sw * TILE_SIZE
                + (h_nw - h_sw) * offset_z
                + (h_ne - h_nw) * offset_x)
                >> TILE_SHIFT
        } else {
            (h_sw * TILE_SIZE
                + (h_se - h_sw) * offset_x
                + (h_ne - h_se) * offset_z)
                >> TILE_SHIFT
        };

        h0.max(h1) as f32
    }
}

const fn clamp_i32(value: i32, min: i32, max: i32) -> i32 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_wrong_packet_length() {
        assert_eq!(
            HeightMap::new(2, 1, vec![0; 3]).unwrap_err(),
            "height-map packet length does not match dimensions"
        );
    }

    #[test]
    fn heights_use_osrs_eight_unit_scale() {
        let map = HeightMap::new(2, 1, vec![1, 2, 3, 4]).unwrap();
        assert_eq!(map.tile_height(0, 0, 0, 0), 8);
        assert_eq!(map.tile_height(1, 0, 0, 0), 16);
    }

    #[test]
    fn interpolation_matches_flat_surface() {
        let map = HeightMap::new(3, 1, vec![5; 9]).unwrap();
        assert_eq!(map.interpolate(64.0, 64.0, 0, 0), 40.0);
    }
}
