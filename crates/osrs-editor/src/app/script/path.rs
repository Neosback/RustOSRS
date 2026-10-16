//! The Spline tool's script commands: `spline X,Y[:W][!] ...`, `width N` and `ground follow|smooth`.
use crate::terrain_path::ControlPoint;

fn float(word: &str, what: &str) -> Result<f32, String> {
    word.parse::<f32>()
        .map_err(|_| format!("{what} '{word}' is not a number"))
}

/// Control points from `X,Y[:WIDTH][!]` words: tile coordinates (the tile's centre), an optional
/// width in tiles (else `default_width`) and `!` for a corner.
pub(super) fn controls<'a>(
    words: impl Iterator<Item = &'a str>,
    default_width: f32,
) -> Result<Vec<ControlPoint>, String> {
    words
        .map(|word| {
            let (word, corner) = word
                .strip_suffix('!')
                .map_or((word, false), |rest| (rest, true));
            let (at, width) = word
                .split_once(':')
                .map_or((word, None), |(at, width)| (at, Some(width)));
            let (x, y) = at.split_once(',').ok_or("points are X,Y[:WIDTH][!]")?;
            Ok(ControlPoint {
                x: float(x, "x")? + 0.5,
                y: float(y, "y")? + 0.5,
                width: width.map_or(Ok(default_width), |w| float(w, "width"))?,
                corner,
            })
        })
        .collect()
}

/// `ground follow|smooth`: whether laid paths smooth the ground along them.
pub(super) fn smooth_ground(word: Option<&str>) -> Result<bool, String> {
    match word {
        Some("smooth") => Ok(true),
        Some("follow") => Ok(false),
        other => Err(format!("ground follow|smooth, not {other:?}")),
    }
}
