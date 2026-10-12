//! Seeded noise: Java's `SplittableRandom` (so a seed means the same path as it does in the
//! reference tool), Perlin noise permuted by it, and the 64-bit mixer that derives sub-seeds.

/// The SplitMix64 finaliser.
pub fn mix64(value: i64) -> i64 {
    let mut z = value as u64;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (z ^ (z >> 31)) as i64
}

/// `java.util.SplittableRandom` with the default gamma.
struct SplittableRandom {
    seed: u64,
}

const GOLDEN_GAMMA: u64 = 0x9E37_79B9_7F4A_7C15;

impl SplittableRandom {
    fn new(seed: i64) -> Self {
        Self { seed: seed as u64 }
    }

    fn next_int(&mut self) -> i32 {
        self.seed = self.seed.wrapping_add(GOLDEN_GAMMA);
        let mut z = self.seed;
        z = (z ^ (z >> 33)).wrapping_mul(0x62A9_D9ED_7997_05F5);
        (((z ^ (z >> 28)).wrapping_mul(0xCB24_D0A5_C88C_35B3)) >> 32) as i32
    }

    /// A value in `0..bound` (`bound > 0`), as `SplittableRandom.nextInt(bound)` makes it.
    fn next_bounded(&mut self, bound: i32) -> i32 {
        let mut r = self.next_int();
        let m = bound - 1;
        if bound & m == 0 {
            return r & m;
        }
        let mut u = ((r as u32) >> 1) as i32;
        loop {
            r = u % bound;
            if u.wrapping_add(m).wrapping_sub(r) >= 0 {
                return r;
            }
            u = ((self.next_int() as u32) >> 1) as i32;
        }
    }
}

pub struct SeededPerlinNoise {
    perm: [i32; 512],
}

pub const MAX_OCTAVES: u32 = 8;

fn fade(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + t * (b - a)
}

fn gradient(hash: i32, x: f64, y: f64) -> f64 {
    match hash & 7 {
        0 => x + y,
        1 => -x + y,
        2 => x - y,
        3 => -x - y,
        4 => x,
        5 => -x,
        6 => y,
        _ => -y,
    }
}

impl SeededPerlinNoise {
    pub fn new(seed: i64) -> Self {
        let mut table: Vec<i32> = (0..256).collect();
        let mut random = SplittableRandom::new(seed);
        for i in (1..=255).rev() {
            let j = random.next_bounded(i + 1) as usize;
            table.swap(i as usize, j);
        }
        let mut perm = [0; 512];
        for (i, slot) in perm.iter_mut().enumerate() {
            *slot = table[i & 0xFF];
        }
        Self { perm }
    }

    pub fn noise(&self, x: f64, y: f64) -> f64 {
        let xi = (x.floor() as i64 & 0xFF) as usize;
        let yi = (y.floor() as i64 & 0xFF) as usize;
        let xf = x - x.floor();
        let yf = y - y.floor();
        let (u, v) = (fade(xf), fade(yf));
        let p = &self.perm;
        let aa = p[p[xi] as usize + yi];
        let ab = p[p[xi] as usize + yi + 1];
        let ba = p[p[xi + 1] as usize + yi];
        let bb = p[p[xi + 1] as usize + yi + 1];
        let x1 = lerp(gradient(aa, xf, yf), gradient(ba, xf - 1.0, yf), u);
        let x2 = lerp(gradient(ab, xf, yf - 1.0), gradient(bb, xf - 1.0, yf - 1.0), u);
        lerp(x1, x2, v)
    }

    /// Fractal noise: `octaves` (1..=8) layers, each twice as fine and half as strong.
    pub fn fbm(&self, x: f64, y: f64, octaves: u32) -> f64 {
        let octaves = octaves.clamp(1, MAX_OCTAVES);
        let (mut sum, mut amplitude, mut total, mut frequency) = (0.0, 1.0, 0.0, 1.0);
        for _ in 0..octaves {
            sum += self.noise(x * frequency, y * frequency) * amplitude;
            total += amplitude;
            frequency *= 2.0;
            amplitude *= 0.5;
        }
        sum / total
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splittable_random_matches_the_jdk() {
        // Values from a JDK: `new SplittableRandom(42)`, then `nextInt()` and `nextInt(7)`.
        let mut random = SplittableRandom::new(42);
        assert_eq!(random.next_int(), -491_277_234);
        assert_eq!(random.next_bounded(7), 5);
        // A Fisher-Yates shuffle's worth of bounded draws.
        let mut random = SplittableRandom::new(123_456_789);
        let mut hash = 0_i32;
        for i in (1..=255).rev() {
            hash = hash.wrapping_mul(31).wrapping_add(random.next_bounded(i + 1));
        }
        assert_eq!(hash, -1_000_247_084);
    }

    #[test]
    fn noise_is_deterministic_and_bounded() {
        let a = SeededPerlinNoise::new(7);
        let b = SeededPerlinNoise::new(7);
        for i in 0..200 {
            let (x, y) = (i as f64 * 0.37, i as f64 * 0.11);
            assert_eq!(a.fbm(x, y, 3), b.fbm(x, y, 3));
            assert!(a.fbm(x, y, 3).abs() <= 1.0);
        }
        assert_ne!(
            SeededPerlinNoise::new(7).fbm(3.3, 1.1, 3),
            SeededPerlinNoise::new(8).fbm(3.3, 1.1, 3)
        );
    }
}
