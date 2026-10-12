//! The road shape grammar: which overlay shape a path tile takes, given which of its eight
//! neighbours are path. The table is a census of real maps: for each neighbourhood (up to
//! rotation) the shapes people placed and how often, as `mask shape,orientation,weight ...`.

use super::atlas::{EAST, NORTH, OverlayShapeAtlas, SOUTH, WEST};
use std::sync::OnceLock;

/// Neighbour offsets, clockwise from `(0, 1)`; bit `i` of a neighbourhood mask is neighbour `i`.
pub const NEIGHBOURS: [(i32, i32); 8] = [
    (0, 1),
    (1, 1),
    (1, 0),
    (1, -1),
    (0, -1),
    (-1, -1),
    (-1, 0),
    (-1, 1),
];

/// The side of the tile each neighbour touches (0 for a corner neighbour).
const SIDE_OF: [u8; 8] = [SOUTH, 0, EAST, 0, NORTH, 0, WEST, 0];

/// The mean overlay area of each shape (the grammar's estimate of how much of a tile it fills).
const SHAPE_AREA: [f64; 12] = [
    1.0, 0.5, 0.25, 0.25, 0.75, 0.75, 0.5, 0.125, 0.875, 0.34375, 0.65625, 0.1875,
];

const RAW: [&str; 48] = [
    "0 0,0,4628 10,0,56 10,1,56 10,2,56 10,3,56 1,0,1175 1,1,1175 1,2,1175 1,3,1175 9,0,112 9,1,112 9,2,112 9,3,112",
    "1 0,0,4172 10,2,654 10,3,838 1,1,470 1,2,614 2,0,1146 3,0,1063 11,2,1043",
    "2 0,0,4317 5,3,114 10,3,341 1,1,2273 1,2,682 1,3,1364 9,1,682 9,2,227",
    "3 0,0,1958 10,2,565 1,1,3217 1,2,695 9,0,783 3,0,1348 3,3,565 11,2,869",
    "5 0,0,2723 10,3,2623 1,2,3564 6,1,50 2,1,297 2,2,50 3,0,148 7,2,545",
    "6 0,0,2700 5,1,345 10,0,460 1,2,632 1,3,2127 9,2,1379 2,1,1725 11,3,632",
    "7 0,0,3591 4,0,78 5,1,45 10,3,2054 1,2,2230 2,1,339 3,0,360 7,2,1303",
    "9 0,0,6667 5,0,351 10,2,175 1,2,1053 9,2,175 9,3,351 3,0,1228",
    "10 0,0,9310 1,2,138 1,3,138 9,0,69 11,1,69 11,3,276",
    "13 0,0,1402 8,2,17 10,3,473 1,1,17 1,2,7753 9,1,51 3,0,270 3,3,17",
    "14 0,0,3644 1,2,607 1,3,769 9,1,405 9,2,445 2,1,202 2,2,122 11,3,3806",
    "15 0,0,833 4,0,12 10,3,524 1,2,2468 9,1,168 2,1,44 3,0,5173 7,2,778",
    "17 0,0,9452 4,0,83 4,2,83 5,0,58 5,2,58 1,0,3 1,2,3 6,0,130 6,2,130",
    "18 0,0,5600 4,2,200 5,3,200 10,1,200 1,3,2200 9,2,400 2,2,1200",
    "19 0,0,9129 4,0,143 4,2,364 5,0,13 5,2,91 10,1,13 6,0,52 6,2,195",
    "22 0,0,1128 10,0,514 1,0,17 1,3,7793 9,2,50 2,2,448 3,1,33 7,3,17",
    "23 0,0,8583 8,3,34 4,0,796 5,2,265 1,3,46 6,0,57 6,2,196 2,2,23",
    "25 0,0,9249 4,0,129 5,0,337 5,2,65 10,1,13 6,0,78 6,2,116 7,0,13",
    "27 0,0,9482 4,0,83 4,2,41 5,0,62 5,2,62 6,2,228 9,0,21 9,2,21",
    "29 0,0,8509 8,2,79 4,0,290 5,2,726 1,2,53 6,0,66 6,2,251 3,0,26",
    "30 0,0,785 10,0,463 1,3,2485 9,2,236 2,2,5134 3,1,37 11,3,16 7,3,844",
    "31 0,0,7334 4,0,639 5,2,626 1,2,23 1,3,17 6,2,1258 2,2,54 3,0,49",
    "39 0,0,6698 5,1,46 10,3,917 1,2,1881 6,2,23 2,1,206 3,0,23 7,2,206",
    "47 0,0,1251 10,3,357 1,2,4643 9,1,89 3,0,3214 7,2,446",
    "51 0,0,8896 4,0,512 4,2,512 6,0,40 6,2,40",
    "54 0,0,1339 8,3,29 10,0,530 1,3,7889 9,2,135 2,2,29 3,1,39 7,3,10",
    "55 0,0,4992 4,0,1041 10,3,374 1,2,716 1,3,2569 9,2,48 2,2,16 7,2,244",
    "57 0,0,9333 4,0,303 5,0,182 6,0,121 6,2,61",
    "59 0,0,9642 6,2,179 9,0,179",
    "61 0,0,7318 8,2,244 4,0,853 5,2,488 1,2,731 6,2,122 3,0,122 3,1,122",
    "62 0,0,480 10,0,246 1,3,5682 9,2,1461 2,2,862 3,1,827 11,3,7 7,3,435",
    "63 0,0,5567 4,0,3293 5,2,66 1,3,184 6,2,303 9,2,423 2,2,79 3,0,85",
    "94 0,0,8435 5,3,608 1,0,348 1,3,87 6,1,87 6,3,87 9,2,174 2,3,174",
    "95 0,0,9150 8,0,111 8,1,93 1,0,350 1,1,93 6,2,93 2,3,55 11,1,55",
    "102 0,0,9108 5,1,346 5,3,346 2,1,50 2,3,50 7,1,50 7,3,50",
    "103 0,0,5688 8,1,58 5,1,1470 10,3,213 1,1,2089 1,2,174 9,0,154 7,2,154",
    "111 0,0,2686 5,1,448 10,3,299 1,1,2538 1,2,1642 3,0,1791 3,3,223 7,2,373",
    "119 0,0,9154 8,1,324 8,3,324 4,0,20 4,2,20 10,1,24 10,3,24 1,0,31 1,2,31 7,0,24 7,2,24",
    "122 0,0,1062 5,0,88 10,1,442 1,0,3452 1,1,88 9,3,354 2,3,3541 7,0,973",
    "123 0,0,4604 4,2,531 10,1,619 1,0,619 1,1,973 9,0,442 9,3,442 2,3,1770",
    "126 0,0,5717 4,1,53 5,3,3113 1,3,170 6,3,288 9,2,523 2,3,74 3,1,62",
    "127 0,0,8932 8,1,924 8,3,15 4,0,21 5,3,29 1,1,45 6,2,15 9,2,19",
    "170 0,0,10000",
    "171 0,0,10000",
    "175 0,0,1074 10,3,621 1,2,4351 9,1,2995 2,1,508 3,0,282 7,2,169",
    "187 0,0,8400 4,0,100 4,2,100 5,0,100 5,2,100 9,0,200 9,2,200 11,0,400 11,2,400",
    "191 0,0,7236 4,0,269 5,2,319 1,2,454 1,3,531 9,1,418 9,2,461 3,1,312",
    "255 0,0,9944 6,0,8 6,1,8 6,2,8 6,3,8 9,0,6 9,1,6 9,2,6 9,3,6",
];

/// One option for a neighbourhood: shape, orientation, cumulative weight out of 10000.
type Choice = (u8, u8, i64);

struct Grammar {
    choices: Vec<Option<Vec<Choice>>>,
    mean_area: [f64; 256],
}

fn grammar() -> &'static Grammar {
    static GRAMMAR: OnceLock<Grammar> = OnceLock::new();
    GRAMMAR.get_or_init(|| {
        let atlas = OverlayShapeAtlas::new(8);
        let mut choices: Vec<Option<Vec<Choice>>> = vec![None; 256];
        let mut mean_area = [0.0; 256];
        for row in RAW {
            let mut parts = row.split(' ');
            let mask: usize = parts.next().and_then(|m| m.parse().ok()).unwrap_or(0);
            let mut list: Vec<Choice> = Vec::new();
            let mut total = 0_i64;
            for part in parts {
                let mut fields = part.split(',').map(|f| f.parse::<i64>().unwrap_or(0));
                let (shape, orientation, weight) = (
                    fields.next().unwrap_or(0) as u8,
                    fields.next().unwrap_or(0) as u8,
                    fields.next().unwrap_or(0),
                );
                // A shape that does not reach a side the path continues over is not a choice.
                let reaches = (0..8).step_by(2).all(|bit| {
                    mask & (1 << bit) == 0
                        || atlas.edge_mask(shape, orientation, SIDE_OF[bit]) != 0
                });
                if reaches {
                    total += weight;
                    list.push((shape, orientation, total));
                }
            }
            for choice in &mut list {
                choice.2 = choice.2 * 10_000 / total;
            }
            if let Some(last) = list.last_mut() {
                last.2 = 10_000;
            }
            let mut area = 0.0;
            let mut previous = 0;
            for (shape, _, cumulative) in &list {
                area += SHAPE_AREA[usize::from(*shape)] * (cumulative - previous) as f64 / 10_000.0;
                previous = *cumulative;
            }
            mean_area[mask] = area;
            choices[mask] = Some(list);
        }
        Grammar {
            choices,
            mean_area,
        }
    })
}

/// The neighbourhood turned by `quarters` quarter turns.
pub fn rotate(mask: u32, quarters: u32) -> u32 {
    let shift = (quarters & 3) << 1;
    if shift == 0 {
        mask & 0xFF
    } else {
        ((mask << shift) | (mask >> (8 - shift))) & 0xFF
    }
}

/// How many quarter turns bring the neighbourhood to its smallest rotation.
pub fn canonical_turns(mask: u32) -> u32 {
    let mut best = 0;
    for turns in 1..4 {
        if rotate(mask, turns) < rotate(mask, best) {
            best = turns;
        }
    }
    best
}

/// The neighbourhood's smallest rotation (what the table is keyed by).
pub fn canonical(mask: u32) -> u32 {
    rotate(mask, canonical_turns(mask))
}

/// The mean overlay area the table gives a neighbourhood (1 when it has no entry).
pub fn mean_area(mask: u32) -> f64 {
    let canonical = canonical(mask) as usize;
    let grammar = grammar();
    if grammar.choices[canonical].is_none() {
        1.0
    } else {
        grammar.mean_area[canonical]
    }
}

/// The shape and orientation, packed as `shape << 2 | orientation`, for a neighbourhood and a
/// draw `u` in `0..=1` (the same `u` always gives the same shape).
pub fn shape_for(mask: u32, u: f64) -> u32 {
    let turns = canonical_turns(mask);
    let Some(list) = grammar().choices[rotate(mask, turns) as usize].as_ref() else {
        return 0;
    };
    let threshold = (u.clamp(0.0, 1.0) * 10_000.0) as i64;
    let chosen = list
        .iter()
        .find(|c| threshold < c.2)
        .or(list.last())
        .copied();
    match chosen {
        Some((0, _, _)) | None => 0,
        Some((shape, orientation, _)) => {
            (u32::from(shape) << 2) + ((u32::from(orientation) + 4 - turns) & 3)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_turns_neighbours_two_places_a_quarter() {
        assert_eq!(rotate(0b0000_0001, 1), 0b0000_0100);
        assert_eq!(rotate(0b1000_0000, 1), 0b0000_0010);
        assert_eq!(rotate(0b0000_0001, 4), 0b0000_0001);
        assert_eq!(canonical(0b0000_0100), 0b0000_0001);
    }

    #[test]
    fn a_surrounded_tile_is_whole() {
        assert_eq!(shape_for(255, 0.0), 0);
        assert_eq!(shape_for(255, 0.5), 0);
        assert_eq!(shape_for(170, 0.9), 0);
    }

    #[test]
    fn every_grammar_row_has_a_distribution() {
        let g = grammar();
        let rows = g.choices.iter().flatten().count();
        assert_eq!(rows, 48);
        for list in g.choices.iter().flatten() {
            assert_eq!(list.last().map(|c| c.2), Some(10_000));
        }
    }

    #[test]
    fn choices_vary_with_the_draw() {
        // Some neighbourhood must offer more than one shape.
        let varied = (0..256_u32).any(|m| {
            let a = shape_for(m, 0.01);
            let b = shape_for(m, 0.99);
            a != b
        });
        assert!(varied);
    }
}
