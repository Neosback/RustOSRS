//! The Spline tool's script commands: `spline X,Y ...` places points, `width N`, `curve on|off`,
//! `edge N`, `height off|PASSES [BLEND]` set the tool's options, and `lay` lays the path.
use crate::paint::SplineSettings;

fn float(word: &str, what: &str) -> Result<f32, String> {
    word.parse::<f32>()
        .map_err(|_| format!("{what} '{word}' is not a number"))
}

/// Path points from `X,Y` words: the tile coordinates; the point is the tile's centre.
pub(super) fn points<'a>(words: impl Iterator<Item = &'a str>) -> Result<Vec<(f32, f32)>, String> {
    words
        .map(|word| {
            let (x, y) = word.split_once(',').ok_or("points are X,Y")?;
            Ok((float(x, "x")?.floor() + 0.5, float(y, "y")?.floor() + 0.5))
        })
        .collect()
}

/// `curve on|off`.
pub(super) fn on_off(word: Option<&str>, what: &str) -> Result<bool, String> {
    match word {
        Some("on") => Ok(true),
        Some("off") => Ok(false),
        other => Err(format!("{what} on|off, not {other:?}")),
    }
}

/// `height off` or `height PASSES [BLEND]`: smooth the terrain under the path.
pub(super) fn height<'a>(
    mut words: impl Iterator<Item = &'a str>,
    settings: &mut SplineSettings,
) -> Result<(), String> {
    match words.next() {
        Some("off") => settings.smooth_height = false,
        Some(passes) => {
            let passes = passes
                .parse::<u32>()
                .map_err(|_| format!("height passes '{passes}' is not a whole number"))?;
            settings.smooth_height = true;
            settings.height_passes = passes.clamp(
                *SplineSettings::PASSES.start(),
                *SplineSettings::PASSES.end(),
            );
            if let Some(blend) = words.next() {
                settings.height_blend = float(blend, "blend")?
                    .clamp(*SplineSettings::BLEND.start(), *SplineSettings::BLEND.end());
            }
        }
        None => return Err("height off|PASSES [BLEND]".to_owned()),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_are_tile_centres() {
        assert_eq!(
            points(["3200,3210", "3204.7,3210"].into_iter()),
            Ok(vec![(3200.5, 3210.5), (3204.5, 3210.5)])
        );
        assert!(points(["3200"].into_iter()).is_err());
        assert!(points(["a,b"].into_iter()).is_err());
    }

    #[test]
    fn height_option_sets_the_terrain_settings() {
        let mut settings = SplineSettings::default();
        assert!(height(["12", "6"].into_iter(), &mut settings).is_ok());
        assert!(settings.smooth_height);
        assert_eq!((settings.height_passes, settings.height_blend), (12, 6.0));
        assert!(height(["9999"].into_iter(), &mut settings).is_ok());
        assert_eq!(settings.height_passes, 250);
        assert!(height(["off"].into_iter(), &mut settings).is_ok());
        assert!(!settings.smooth_height);
        assert!(height(std::iter::empty(), &mut settings).is_err());
        assert!(height(["x"].into_iter(), &mut settings).is_err());
    }

    #[test]
    fn on_off_words() {
        assert_eq!(on_off(Some("on"), "curve"), Ok(true));
        assert_eq!(on_off(Some("off"), "curve"), Ok(false));
        assert!(on_off(Some("maybe"), "curve").is_err());
        assert!(on_off(None, "curve").is_err());
    }
}
