pub const MATERIAL_TEXTURE_ROWS: usize = 6;
pub const MATERIAL_FLAG_WATER: u8 = 1;
pub const WATER_FLAG_HAS_FOAM: u8 = 1;
pub const WATER_FLAG_NORMAL_MAP_2: u8 = 2;

/// CPU reference for one column of the current RGBA8I material texture.
///
/// The TypeScript producer stores bytes in an Int8Array. GLSL recovers their
/// unsigned value with `& 0xff`; Rust mirrors that by casting i8 to u8.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Material {
    pub anim_u: i8,
    pub anim_v: i8,
    pub alpha_cut_off: f32,
    pub frame_count: u8,
    pub anim_speed: u8,
    pub flags: u8,
    pub has_foam: bool,
    pub use_normal_map_2: bool,
    pub water_surface_color: [f32; 3],
    pub water_base_opacity: f32,
    pub water_depth_color: [f32; 3],
    pub water_fresnel_amount: f32,
    pub water_normal_strength: f32,
    pub water_specular_strength: f32,
    pub water_specular_gloss: f32,
    pub water_duration: f32,
    pub water_foam_color: [f32; 3],
}

impl Material {
    pub fn is_water(self) -> bool {
        self.flags & MATERIAL_FLAG_WATER != 0
    }
}

pub fn decode_material(
    data: &[i8],
    texture_count: usize,
    texture_id: usize,
) -> Result<Material, &'static str> {
    if texture_count == 0 {
        return Err("material texture width must be positive");
    }
    let expected = texture_count
        .checked_mul(MATERIAL_TEXTURE_ROWS)
        .and_then(|value| value.checked_mul(4))
        .ok_or("material texture dimensions overflow")?;
    if data.len() != expected {
        return Err("material texture packet length does not match dimensions");
    }
    if texture_id >= texture_count {
        return Err("material texture id is outside packet width");
    }

    let row = |row: usize| -> [i8; 4] {
        let base = (row * texture_count + texture_id) * 4;
        [data[base], data[base + 1], data[base + 2], data[base + 3]]
    };
    let row0 = row(0);
    let row1 = row(1);
    let row2 = row(2);
    let row3 = row(3);
    let row4 = row(4);
    let row5 = row(5);

    let frame_count = unsigned(row0[3]).max(1);
    let water_flags = unsigned(row1[2]);

    Ok(Material {
        anim_u: row0[0],
        anim_v: row0[1],
        alpha_cut_off: normalized(row0[2]),
        frame_count,
        anim_speed: unsigned(row1[0]),
        flags: unsigned(row1[1]),
        has_foam: water_flags & WATER_FLAG_HAS_FOAM != 0,
        use_normal_map_2: water_flags & WATER_FLAG_NORMAL_MAP_2 != 0,
        water_surface_color: rgb(row2),
        water_base_opacity: normalized(row2[3]),
        water_depth_color: rgb(row3),
        water_fresnel_amount: normalized(row3[3]),
        water_normal_strength: normalized(row4[0]) * 0.5,
        water_specular_strength: normalized(row4[1]),
        water_specular_gloss: (normalized(row4[2]) * 500.0).max(1.0),
        water_duration: normalized(row4[3]) * 4.0,
        water_foam_color: rgb(row5),
    })
}

const fn unsigned(value: i8) -> u8 {
    value as u8
}

fn normalized(value: i8) -> f32 {
    f32::from(unsigned(value)) / 255.0
}

fn rgb(row: [i8; 4]) -> [f32; 3] {
    [
        normalized(row[0]),
        normalized(row[1]),
        normalized(row[2]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_storage_recovers_unsigned_material_bytes() {
        let texture_count = 1;
        let mut data = vec![0i8; texture_count * MATERIAL_TEXTURE_ROWS * 4];

        data[0] = -2;
        data[1] = 3;
        data[2] = -1;
        data[3] = 0;

        data[4] = 5;
        data[5] = MATERIAL_FLAG_WATER as i8;
        data[6] = (WATER_FLAG_HAS_FOAM | WATER_FLAG_NORMAL_MAP_2) as i8;

        data[8] = 127;
        data[9] = -128;
        data[10] = -1;
        data[11] = -1;

        data[12] = 0;
        data[13] = 64;
        data[14] = -128;
        data[15] = 127;

        data[16] = 51;
        data[17] = -1;
        data[18] = -1;
        data[19] = 64;

        data[20] = -1;
        data[21] = 0;
        data[22] = 127;

        let material = decode_material(&data, texture_count, 0).unwrap();

        assert_eq!(material.anim_u, -2);
        assert_eq!(material.anim_v, 3);
        assert_eq!(material.frame_count, 1);
        assert_eq!(material.anim_speed, 5);
        assert!(material.is_water());
        assert!(material.has_foam);
        assert!(material.use_normal_map_2);
        assert_eq!(material.alpha_cut_off, 1.0);
        assert_eq!(material.water_surface_color[2], 1.0);
        assert_eq!(material.water_base_opacity, 1.0);
        assert!((material.water_normal_strength - 0.1).abs() < 0.001);
        assert_eq!(material.water_specular_strength, 1.0);
        assert_eq!(material.water_foam_color[0], 1.0);
    }

    #[test]
    fn validates_texture_width_and_id() {
        assert!(decode_material(&[], 0, 0).is_err());
        assert!(decode_material(&[0; MATERIAL_TEXTURE_ROWS * 4], 1, 1).is_err());
    }
}
