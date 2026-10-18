//! Deterministic merged grid boundaries, including holes, without dropping selected tiles.
use std::collections::{BTreeMap, BTreeSet, HashSet};
type Tile = (i32, i32);

/// Collinear runs of corner coordinates; callers sample each vertex to follow uneven ground.
pub(super) fn runs(tiles: &HashSet<Tile>) -> Vec<Vec<Tile>> {
    let mut lines: BTreeMap<(bool, i32), BTreeSet<i32>> = BTreeMap::new();
    for &(x, y) in tiles {
        for (beside, vertical, line, start) in [
            ((x, y - 1), false, y, x),
            ((x, y + 1), false, y + 1, x),
            ((x - 1, y), true, x, y),
            ((x + 1, y), true, x + 1, y),
        ] {
            if !tiles.contains(&beside) {
                lines.entry((vertical, line)).or_default().insert(start);
            }
        }
    }
    let mut result = Vec::new();
    for ((vertical, line), starts) in lines {
        let mut run = Vec::new();
        let point = |n| if vertical { (line, n) } else { (n, line) };
        for n in starts {
            if run.last() != Some(&point(n)) {
                if !run.is_empty() {
                    result.push(std::mem::take(&mut run));
                }
                run.push(point(n));
            }
            run.push(point(n + 1));
        }
        if !run.is_empty() {
            result.push(run);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn boundaries_merge_and_preserve_holes_independent_of_insertion_order() {
        let tiles: HashSet<_> = (0..3)
            .flat_map(|x| (0..3).map(move |y| (x, y)))
            .filter(|t| *t != (1, 1))
            .collect();
        let a = runs(&tiles);
        assert_eq!(a.len(), 8);
        assert_eq!(a.iter().map(|r| r.len() - 1).sum::<usize>(), 16);
        assert_eq!(a, runs(&tiles.iter().copied().collect()));
    }
}
