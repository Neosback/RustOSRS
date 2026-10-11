#![allow(unused_variables)]
//! Tile-shape masks of the world map (`WorldMapScaleHandler.init0..init7`), generated from the
//! client's loops: every template is a pair of loops over rows and columns plus one comparison.
//! A set entry paints the overlay colour, a clear entry the ground colour.

// Generated from the client's `WorldMapScaleHandler.init0..init7` (every template is a pair of
// loops over rows and columns plus one comparison).
pub(super) fn build(ppt: i32, shape: usize, rotation: usize) -> Vec<bool> {
    let size = (ppt * ppt) as usize;
    let mut mask = vec![false; size];
    let mut index = 0_usize;
    match (shape, rotation) {
        (0, 0) => {
            for var3 in 0..ppt {
                for var4 in 0..ppt {
                    if var4 <= var3 {
                        mask[index] = true;
                    }
                    index += 1;
                }
            }
        }
        (0, 1) => {
            for var11 in (0..=ppt - 1).rev() {
                for var4 in 0..ppt {
                    if var4 <= var11 {
                        mask[index] = true;
                    }
                    index += 1;
                }
            }
        }
        (0, 2) => {
            for var12 in 0..ppt {
                for var4 in 0..ppt {
                    if var4 >= var12 {
                        mask[index] = true;
                    }
                    index += 1;
                }
            }
        }
        (0, 3) => {
            for var13 in (0..=ppt - 1).rev() {
                for var4 in 0..ppt {
                    if var4 >= var13 {
                        mask[index] = true;
                    }
                    index += 1;
                }
            }
        }
        (1, 0) => {
            for var3 in (0..=ppt - 1).rev() {
                for var4 in 0..ppt {
                    if var4 <= var3 >> 1 {
                        mask[index] = true;
                    }
                    index += 1;
                }
            }
        }
        (1, 1) => {
            for row in 0..ppt {
                for column in 0..ppt {
                    if column >= row << 1 {
                        mask[index] = true;
                    }
                    index += 1;
                }
            }
        }
        (1, 2) => {
            for var12 in 0..ppt {
                for var4 in (0..=ppt - 1).rev() {
                    if var4 <= var12 >> 1 {
                        mask[index] = true;
                    }
                    index += 1;
                }
            }
        }
        (1, 3) => {
            for var13 in (0..=ppt - 1).rev() {
                for var4 in (0..=ppt - 1).rev() {
                    if var4 >= var13 << 1 {
                        mask[index] = true;
                    }
                    index += 1;
                }
            }
        }
        (2, 0) => {
            for var3 in (0..=ppt - 1).rev() {
                for var4 in (0..=ppt - 1).rev() {
                    if var4 <= var3 >> 1 {
                        mask[index] = true;
                    }
                    index += 1;
                }
            }
        }
        (2, 1) => {
            for var11 in (0..=ppt - 1).rev() {
                for var4 in 0..ppt {
                    if var4 >= var11 << 1 {
                        mask[index] = true;
                    }
                    index += 1;
                }
            }
        }
        (2, 2) => {
            for var12 in 0..ppt {
                for var4 in 0..ppt {
                    if var4 <= var12 >> 1 {
                        mask[index] = true;
                    }
                    index += 1;
                }
            }
        }
        (2, 3) => {
            for var13 in 0..ppt {
                for var4 in (0..=ppt - 1).rev() {
                    if var4 >= var13 << 1 {
                        mask[index] = true;
                    }
                    index += 1;
                }
            }
        }
        (3, 0) => {
            for var3 in (0..=ppt - 1).rev() {
                for var4 in 0..ppt {
                    if var4 >= var3 >> 1 {
                        mask[index] = true;
                    }
                    index += 1;
                }
            }
        }
        (3, 1) => {
            for var11 in 0..ppt {
                for var4 in 0..ppt {
                    if var4 <= var11 << 1 {
                        mask[index] = true;
                    }
                    index += 1;
                }
            }
        }
        (3, 2) => {
            for var12 in 0..ppt {
                for var4 in (0..=ppt - 1).rev() {
                    if var4 >= var12 >> 1 {
                        mask[index] = true;
                    }
                    index += 1;
                }
            }
        }
        (3, 3) => {
            for var13 in (0..=ppt - 1).rev() {
                for var4 in (0..=ppt - 1).rev() {
                    if var4 <= var13 << 1 {
                        mask[index] = true;
                    }
                    index += 1;
                }
            }
        }
        (4, 0) => {
            for var3 in (0..=ppt - 1).rev() {
                for var4 in (0..=ppt - 1).rev() {
                    if var4 >= var3 >> 1 {
                        mask[index] = true;
                    }
                    index += 1;
                }
            }
        }
        (4, 1) => {
            for var11 in (0..=ppt - 1).rev() {
                for var4 in 0..ppt {
                    if var4 <= var11 << 1 {
                        mask[index] = true;
                    }
                    index += 1;
                }
            }
        }
        (4, 2) => {
            for var12 in 0..ppt {
                for var4 in 0..ppt {
                    if var4 >= var12 >> 1 {
                        mask[index] = true;
                    }
                    index += 1;
                }
            }
        }
        (4, 3) => {
            for var13 in 0..ppt {
                for var4 in (0..=ppt - 1).rev() {
                    if var4 <= var13 << 1 {
                        mask[index] = true;
                    }
                    index += 1;
                }
            }
        }
        (5, 0) => {
            for var3 in 0..ppt {
                for var4 in 0..ppt {
                    if var4 <= ppt / 2 {
                        mask[index] = true;
                    }
                    index += 1;
                }
            }
        }
        (5, 1) => {
            for var13 in 0..ppt {
                for var4 in 0..ppt {
                    if var13 <= ppt / 2 {
                        mask[index] = true;
                    }
                    index += 1;
                }
            }
        }
        (5, 2) => {
            for var14 in 0..ppt {
                for var4 in 0..ppt {
                    if var4 >= ppt / 2 {
                        mask[index] = true;
                    }
                    index += 1;
                }
            }
        }
        (5, 3) => {
            for var15 in 0..ppt {
                for var4 in 0..ppt {
                    if var15 >= ppt / 2 {
                        mask[index] = true;
                    }
                    index += 1;
                }
            }
        }
        (6, 0) => {
            for var3 in 0..ppt {
                for var4 in 0..ppt {
                    if var4 <= var3 - ppt / 2 {
                        mask[index] = true;
                    }
                    index += 1;
                }
            }
        }
        (6, 1) => {
            for var13 in (0..=ppt - 1).rev() {
                for var4 in 0..ppt {
                    if var4 <= var13 - ppt / 2 {
                        mask[index] = true;
                    }
                    index += 1;
                }
            }
        }
        (6, 2) => {
            for var14 in (0..=ppt - 1).rev() {
                for var4 in (0..=ppt - 1).rev() {
                    if var4 <= var14 - ppt / 2 {
                        mask[index] = true;
                    }
                    index += 1;
                }
            }
        }
        (6, 3) => {
            for var15 in 0..ppt {
                for var4 in (0..=ppt - 1).rev() {
                    if var4 <= var15 - ppt / 2 {
                        mask[index] = true;
                    }
                    index += 1;
                }
            }
        }
        (7, 0) => {
            for var3 in 0..ppt {
                for var4 in 0..ppt {
                    if var4 >= var3 - ppt / 2 {
                        mask[index] = true;
                    }
                    index += 1;
                }
            }
        }
        (7, 1) => {
            for var13 in (0..=ppt - 1).rev() {
                for var4 in 0..ppt {
                    if var4 >= var13 - ppt / 2 {
                        mask[index] = true;
                    }
                    index += 1;
                }
            }
        }
        (7, 2) => {
            for var14 in (0..=ppt - 1).rev() {
                for var4 in (0..=ppt - 1).rev() {
                    if var4 >= var14 - ppt / 2 {
                        mask[index] = true;
                    }
                    index += 1;
                }
            }
        }
        (7, 3) => {
            for var15 in 0..ppt {
                for var4 in (0..=ppt - 1).rev() {
                    if var4 >= var15 - ppt / 2 {
                        mask[index] = true;
                    }
                    index += 1;
                }
            }
        }
        _ => {}
    }
    mask
}
