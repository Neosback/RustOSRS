//! The Spline tool's script commands: `spline X,Y[:W][!] ...`, `width N` and `ground follow|smooth`.
use crate::terrain_path::{ControlPoint, Wander};

fn float(word: &str, what: &str) -> Result<f32, String> {
    word.parse::<f32>()
        .map_err(|_| format!("{what} '{word}' is not a number"))
}

/// Control points from `X,Y[:WIDTH][!][~|~~][^]` words: tile coordinates (the tile's centre), an
/// optional width in tiles (else `default_width`), `!` for a corner, `~` / `~~` for a gentle /
/// natural wander on the stretch after the point and `^` to smooth the ground there. Each point
/// gets its own seed.
pub(super) fn controls<'a>(
    words: impl Iterator<Item = &'a str>,
    default_width: f32,
) -> Result<Vec<ControlPoint>, String> {
    words
        .enumerate()
        .map(|(index, word)| {
            let (word, smooth_ground) = strip(word, "^");
            let (word, wander) = if let (rest, true) = strip(word, "~~") {
                (rest, Wander::Natural)
            } else if let (rest, true) = strip(word, "~") {
                (rest, Wander::Gentle)
            } else {
                (word, Wander::Off)
            };
            let (word, corner) = strip(word, "!");
            let (at, width) = word
                .split_once(':')
                .map_or((word, None), |(at, width)| (at, Some(width)));
            let (x, y) = at.split_once(',').ok_or("points are X,Y[:WIDTH][!]")?;
            Ok(ControlPoint {
                x: float(x, "x")? + 0.5,
                y: float(y, "y")? + 0.5,
                width: width.map_or(Ok(default_width), |w| float(w, "width"))?,
                corner,
                wander,
                seed: index as u32 + 1,
                smooth_ground,
            })
        })
        .collect()
}

fn strip<'a>(word: &'a str, suffix: &str) -> (&'a str, bool) {
    word.strip_suffix(suffix)
        .map_or((word, false), |rest| (rest, true))
}

/// `ground follow|smooth`: whether laid paths smooth the ground along them.
pub(super) fn smooth_ground(word: Option<&str>) -> Result<bool, String> {
    match word {
        Some("smooth") => Ok(true),
        Some("follow") => Ok(false),
        other => Err(format!("ground follow|smooth, not {other:?}")),
    }
}
