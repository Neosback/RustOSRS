//! Integer sine/cosine tables from the pinned client's `Rasterizer3D` static initializer.
//!
//! `table[i] = (int)(65536.0 * sin(i * 0.0030679615))` for `i` in `0..2048`, i.e. 2048 Jagex
//! angular units per full turn at 16.16 fixed point.

use std::{array, sync::OnceLock};

/// Number of entries in each table (one per JAU).
pub const TRIG_TABLE_SIZE: usize = 2048;
const TRIG_SCALE: f64 = 65_536.0;
const TRIG_STEP: f64 = 0.003_067_961_5;

/// Both reference trig tables.
#[derive(Debug)]
pub struct TrigTables {
    sine: [i32; TRIG_TABLE_SIZE],
    cosine: [i32; TRIG_TABLE_SIZE],
}

impl TrigTables {
    /// `Rasterizer3D_sine[index]`; `index` must be below [`TRIG_TABLE_SIZE`].
    pub const fn sine(&self, index: usize) -> i32 {
        self.sine[index]
    }

    /// `Rasterizer3D_cosine[index]`; `index` must be below [`TRIG_TABLE_SIZE`].
    pub const fn cosine(&self, index: usize) -> i32 {
        self.cosine[index]
    }
}

/// Shared lazily-built reference tables.
pub fn trig_tables() -> &'static TrigTables {
    static TABLES: OnceLock<TrigTables> = OnceLock::new();
    TABLES.get_or_init(|| TrigTables {
        sine: array::from_fn(|index| (TRIG_SCALE * ((index as f64) * TRIG_STEP).sin()) as i32),
        cosine: array::from_fn(|index| (TRIG_SCALE * ((index as f64) * TRIG_STEP).cos()) as i32),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_match_java_reference_values() {
        // Values produced by the pinned client's formula under Java `Math.sin/cos` + `(int)`.
        let tables = trig_tables();
        for (index, sine, cosine) in [
            (0, 0, 65_536),
            (1, 201, 65_535),
            (255, 46_198, 46_482),
            (256, 46_340, 46_340),
            (511, 65_535, 201),
            (512, 65_535, 0),
            (513, 65_535, -201),
            (1024, 0, -65_535),
            (1536, -65_535, 0),
            (2047, -201, 65_535),
        ] {
            assert_eq!(tables.sine(index), sine, "sine[{index}]");
            assert_eq!(tables.cosine(index), cosine, "cosine[{index}]");
        }
    }
}
