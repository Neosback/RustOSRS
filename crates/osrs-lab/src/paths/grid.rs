//! Small grid helpers: distances and counting windows over a `width` by `height` field.

/// Chessboard distance from every tile to the nearest `true` tile, capped at `cap` (unreached
/// tiles get `cap`). Two sweeps, no allocation beyond the result.
pub fn distance_to(sources: &[bool], width: usize, height: usize, cap: u8) -> Vec<u8> {
    let mut d: Vec<u8> = sources.iter().map(|s| if *s { 0 } else { cap }).collect();
    let at = |x: usize, y: usize| y * width + x;
    for y in 0..height {
        for x in 0..width {
            let mut best = d[at(x, y)];
            if x > 0 {
                best = best.min(d[at(x - 1, y)].saturating_add(1));
                if y > 0 {
                    best = best.min(d[at(x - 1, y - 1)].saturating_add(1));
                }
            }
            if y > 0 {
                best = best.min(d[at(x, y - 1)].saturating_add(1));
                if x + 1 < width {
                    best = best.min(d[at(x + 1, y - 1)].saturating_add(1));
                }
            }
            d[at(x, y)] = best.min(cap);
        }
    }
    for y in (0..height).rev() {
        for x in (0..width).rev() {
            let mut best = d[at(x, y)];
            if x + 1 < width {
                best = best.min(d[at(x + 1, y)].saturating_add(1));
                if y + 1 < height {
                    best = best.min(d[at(x + 1, y + 1)].saturating_add(1));
                }
            }
            if y + 1 < height {
                best = best.min(d[at(x, y + 1)].saturating_add(1));
                if x > 0 {
                    best = best.min(d[at(x - 1, y + 1)].saturating_add(1));
                }
            }
            d[at(x, y)] = best.min(cap);
        }
    }
    d
}

/// For every tile, how many `true` tiles lie within `radius` (a square window).
pub fn count_within(marks: &[bool], width: usize, height: usize, radius: usize) -> Vec<u16> {
    let stride = width + 1;
    let mut sum = vec![0_u32; stride * (height + 1)];
    for y in 0..height {
        let mut row = 0;
        for x in 0..width {
            row += u32::from(marks[y * width + x]);
            sum[(y + 1) * stride + x + 1] = sum[y * stride + x + 1] + row;
        }
    }
    let mut out = vec![0_u16; width * height];
    for y in 0..height {
        for x in 0..width {
            let (x0, x1) = (x.saturating_sub(radius), (x + radius + 1).min(width));
            let (y0, y1) = (y.saturating_sub(radius), (y + radius + 1).min(height));
            let total = sum[y1 * stride + x1] + sum[y0 * stride + x0]
                - sum[y0 * stride + x1]
                - sum[y1 * stride + x0];
            out[y * width + x] = total as u16;
        }
    }
    out
}

/// The 8 neighbours of a tile that lie inside the grid.
pub fn neighbours8(
    x: usize,
    y: usize,
    width: usize,
    height: usize,
) -> impl Iterator<Item = (usize, usize)> {
    (-1_i32..=1)
        .flat_map(|dy| (-1_i32..=1).map(move |dx| (dx, dy)))
        .filter(|d| *d != (0, 0))
        .filter_map(move |(dx, dy)| {
            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
            (nx >= 0 && ny >= 0 && (nx as usize) < width && (ny as usize) < height)
                .then_some((nx as usize, ny as usize))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distances_and_counts_match_a_hand_grid() {
        let mut marks = vec![false; 25];
        marks[12] = true; // the centre of a 5x5
        let d = distance_to(&marks, 5, 5, 9);
        assert_eq!(d[12], 0);
        assert_eq!(d[13], 1);
        assert_eq!(d[0], 2);
        let c = count_within(&marks, 5, 5, 1);
        assert_eq!(c[12], 1);
        assert_eq!(c[6], 1);
        assert_eq!(c[0], 0);
        assert_eq!(neighbours8(0, 0, 5, 5).count(), 3);
    }
}
