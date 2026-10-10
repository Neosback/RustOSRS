//! The pinned client's 16-bit HSL to RGB palette (`Rasterizer3D.buildPalette`).
//!
//! Terrain uses it only for the tile-level RGB (minimap-style) values; 3D corner colors stay
//! packed HSL. The palette depends on the client brightness setting.

/// Number of packed 16-bit HSL entries (`hue6 << 10 | sat3 << 7 | light7`).
pub const PALETTE_LEN: usize = 65_536;

/// Build `Rasterizer3D_colorPalette` for `brightness` (the client default is `0.8`).
pub fn build_color_palette(brightness: f64) -> Vec<i32> {
    let mut palette = vec![0_i32; PALETTE_LEN];
    let mut cursor = 0_usize;
    for hs in 0..512_i32 {
        let hue = f64::from(hs >> 3) / 64.0 + 0.007_812_5;
        let saturation = f64::from(hs & 7) / 8.0 + 0.062_5;
        for light in 0..128_i32 {
            let lightness = f64::from(light) / 128.0;
            let (mut red, mut green, mut blue) = (lightness, lightness, lightness);
            if saturation != 0.0 {
                let q = if lightness < 0.5 {
                    lightness * (1.0 + saturation)
                } else {
                    lightness + saturation - lightness * saturation
                };
                let p = 2.0 * lightness - q;
                let mut red_hue = hue + 0.333_333_333_333_333_3;
                if red_hue > 1.0 {
                    red_hue -= 1.0;
                }
                let mut blue_hue = hue - 0.333_333_333_333_333_3;
                if blue_hue < 0.0 {
                    blue_hue += 1.0;
                }
                red = channel(p, q, red_hue);
                green = channel(p, q, hue);
                blue = channel(p, q, blue_hue);
            }
            let r = (red * 256.0) as i32;
            let g = (green * 256.0) as i32;
            let b = (blue * 256.0) as i32;
            let mut rgb = brighten(b + (g << 8) + (r << 16), brightness);
            if rgb == 0 {
                rgb = 1;
            }
            palette[cursor] = rgb;
            cursor += 1;
        }
    }
    palette
}

fn channel(p: f64, q: f64, hue: f64) -> f64 {
    if 6.0 * hue < 1.0 {
        p + (q - p) * 6.0 * hue
    } else if 2.0 * hue < 1.0 {
        q
    } else if 3.0 * hue < 2.0 {
        p + (q - p) * (0.666_666_666_666_666_6 - hue) * 6.0
    } else {
        p
    }
}

fn brighten(rgb: i32, brightness: f64) -> i32 {
    let r = f64::from(rgb >> 16) / 256.0;
    let g = f64::from(rgb >> 8 & 255) / 256.0;
    let b = f64::from(rgb & 255) / 256.0;
    let r = (r.powf(brightness) * 256.0) as i32;
    let g = (g.powf(brightness) * 256.0) as i32;
    let b = (b.powf(brightness) * 256.0) as i32;
    b + (g << 8) + (r << 16)
}
