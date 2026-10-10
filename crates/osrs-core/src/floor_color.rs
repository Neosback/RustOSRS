//! Exact floor-definition color post-decode semantics for the pinned OSRS target.
//!
//! These helpers intentionally stop at the definition-level HSL state verified
//! by `TERRAIN-003`. The complete builder is now source-pinned in
//! `class470.method9712`; neighborhood blending, slope/shadow lighting, overlay
//! composition, and tile emission remain production work under `TERRAIN-004`.

use crate::definitions::Rgb24;

/// Post-decode underlay color state.
///
/// `weighted_hue` is not a generic 0..255 hue. The client scales the hue
/// fraction by `hue_multiplier`, and later terrain construction may consume the
/// two values independently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UnderlayHsl {
    pub weighted_hue: i32,
    pub saturation: i32,
    pub lightness: i32,
    pub hue_multiplier: i32,
}

impl UnderlayHsl {
    /// Reproduce `FloorUnderlayDefinition.setHsl` exactly for a 24-bit RGB.
    pub fn from_rgb(rgb: Rgb24) -> Self {
        let (hue_fraction, saturation_fraction, lightness_fraction) = rgb_fractions(rgb);
        let saturation = clamp_byte((256.0 * saturation_fraction) as i32);
        let lightness = clamp_byte((256.0 * lightness_fraction) as i32);

        let mut hue_multiplier = if lightness_fraction > 0.5 {
            ((1.0 - lightness_fraction) * saturation_fraction * 512.0) as i32
        } else {
            (lightness_fraction * saturation_fraction * 512.0) as i32
        };
        if hue_multiplier < 1 {
            hue_multiplier = 1;
        }

        Self {
            weighted_hue: (f64::from(hue_multiplier) * hue_fraction) as i32,
            saturation,
            lightness,
            hue_multiplier,
        }
    }
}

/// Post-decode overlay HSL state used for both primary and secondary RGB.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OverlayHsl {
    pub hue: i32,
    pub saturation: i32,
    pub lightness: i32,
}

impl OverlayHsl {
    /// Reproduce `FloorOverlayDefinition.setHsl` exactly for a 24-bit RGB.
    pub fn from_rgb(rgb: Rgb24) -> Self {
        let (hue_fraction, saturation_fraction, lightness_fraction) = rgb_fractions(rgb);
        Self {
            hue: (256.0 * hue_fraction) as i32,
            saturation: clamp_byte((256.0 * saturation_fraction) as i32),
            lightness: clamp_byte((256.0 * lightness_fraction) as i32),
        }
    }
}

fn rgb_fractions(rgb: Rgb24) -> (f64, f64, f64) {
    let value = rgb.get();
    let red = f64::from((value >> 16) & 255) / 256.0;
    let green = f64::from((value >> 8) & 255) / 256.0;
    let blue = f64::from(value & 255) / 256.0;

    let minimum = red.min(green).min(blue);
    let maximum = red.max(green).max(blue);
    let lightness = (minimum + maximum) / 2.0;
    let mut hue = 0.0;
    let mut saturation = 0.0;

    if minimum != maximum {
        saturation = if lightness < 0.5 {
            (maximum - minimum) / (minimum + maximum)
        } else {
            (maximum - minimum) / (2.0 - maximum - minimum)
        };

        hue = if maximum == red {
            (green - blue) / (maximum - minimum)
        } else if maximum == green {
            (blue - red) / (maximum - minimum) + 2.0
        } else {
            (red - green) / (maximum - minimum) + 4.0
        };
    }

    (hue / 6.0, saturation, lightness)
}

const fn clamp_byte(value: i32) -> i32 {
    if value < 0 {
        0
    } else if value > 255 {
        255
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb(value: u32) -> Rgb24 {
        match Rgb24::new(value) {
            Some(value) => value,
            None => unreachable!("test fixture is a valid 24-bit RGB"),
        }
    }

    #[test]
    fn underlay_matches_pinned_weighted_hue_vectors() {
        assert_eq!(
            UnderlayHsl::from_rgb(rgb(0x12_34_56)),
            UnderlayHsl {
                weighted_hue: 39,
                saturation: 167,
                lightness: 52,
                hue_multiplier: 68,
            }
        );
        assert_eq!(
            UnderlayHsl::from_rgb(rgb(0xff_00_ff)),
            UnderlayHsl {
                weighted_hue: -42,
                saturation: 255,
                lightness: 127,
                hue_multiplier: 255,
            }
        );
    }

    #[test]
    fn underlay_clamps_and_keeps_hue_multiplier_at_least_one() {
        assert_eq!(
            UnderlayHsl::from_rgb(rgb(0)),
            UnderlayHsl {
                weighted_hue: 0,
                saturation: 0,
                lightness: 0,
                hue_multiplier: 1,
            }
        );
        assert_eq!(
            UnderlayHsl::from_rgb(rgb(0xff_ff_ff)),
            UnderlayHsl {
                weighted_hue: 0,
                saturation: 0,
                lightness: 255,
                hue_multiplier: 1,
            }
        );
        assert_eq!(UnderlayHsl::from_rgb(rgb(0xff_00_00)).saturation, 255);
    }

    #[test]
    fn overlay_matches_pinned_primary_secondary_hsl_vectors() {
        assert_eq!(
            OverlayHsl::from_rgb(rgb(0x12_34_56)),
            OverlayHsl {
                hue: 149,
                saturation: 167,
                lightness: 52,
            }
        );
        assert_eq!(
            OverlayHsl::from_rgb(rgb(0xab_cd_ef)),
            OverlayHsl {
                hue: 149,
                saturation: 170,
                lightness: 205,
            }
        );
    }

    #[test]
    fn overlay_preserves_negative_hue_and_clamps_byte_fields() {
        assert_eq!(
            OverlayHsl::from_rgb(rgb(0xff_00_ff)),
            OverlayHsl {
                hue: -42,
                saturation: 255,
                lightness: 127,
            }
        );
        assert_eq!(OverlayHsl::from_rgb(rgb(0xff_ff_ff)).lightness, 255);
    }
}
