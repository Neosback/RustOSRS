/// The client's two-direction biquad-cascade filter with a time-varying blend between two sets
/// of coefficients. Arithmetic uses `f32` where the client uses `float`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AudioFilter {
    /// Pole/zero pairs in the forward and backward direction.
    pub pairs: [i32; 2],
    /// Gain at the start and end of the blend, in 1/655.36 dB.
    pub gain: [i32; 2],
    /// Pole frequency `[direction][start or end][pair]`.
    pub pole_frequency: [[[i32; 4]; 2]; 2],
    /// Zero magnitude `[direction][start or end][pair]`.
    pub zero_magnitude: [[[i32; 4]; 2]; 2],
}

/// Coefficients computed for one blend position.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Scratch {
    temp: [[f32; 8]; 2],
    pub coefficients: [[i32; 8]; 2],
    scale: f32,
    pub forward_multiplier: i32,
}

impl AudioFilter {
    pub(super) fn active(&self) -> bool {
        self.pairs[0] > 0 || self.pairs[1] > 0
    }

    fn magnitude(&self, direction: usize, pair: usize, blend: f32) -> f32 {
        let a = self.zero_magnitude[direction][0][pair];
        let b = self.zero_magnitude[direction][1][pair];
        let mut value = a as f32 + blend * (b - a) as f32;
        value *= 0.001_525_878_9;
        1.0 - f64::powf(10.0, f64::from(-value / 20.0)) as f32
    }

    fn frequency(&self, direction: usize, pair: usize, blend: f32, rate: i32) -> f32 {
        let a = self.pole_frequency[direction][0][pair];
        let b = self.pole_frequency[direction][1][pair];
        let mut value = a as f32 + blend * (b - a) as f32;
        value *= 1.220_703_1e-4;
        let hz = 32.703_197_f32 * f64::powf(2.0, f64::from(value)) as f32;
        hz * std::f32::consts::PI / (rate / 2) as f32
    }

    /// Fill `scratch` for `direction` at `blend` and return the number of coefficients.
    pub(super) fn compute(
        &self,
        scratch: &mut Scratch,
        direction: usize,
        blend: f32,
        rate: i32,
    ) -> usize {
        if direction == 0 {
            let mut gain = self.gain[0] as f32 + (self.gain[1] - self.gain[0]) as f32 * blend;
            gain *= 0.003_051_757_8;
            scratch.scale = f64::powf(0.1, f64::from(gain / 20.0)) as f32;
            scratch.forward_multiplier = (scratch.scale * 65_536.0) as i32;
        }
        let pairs = self.pairs[direction] as usize;
        if pairs == 0 {
            return 0;
        }
        let temp = &mut scratch.temp[direction];
        let mut m = self.magnitude(direction, 0, blend);
        temp[0] = -2.0 * m * f64::cos(f64::from(self.frequency(direction, 0, blend, rate))) as f32;
        temp[1] = m * m;
        for pair in 1..pairs {
            m = self.magnitude(direction, pair, blend);
            let linear =
                -2.0 * m * f64::cos(f64::from(self.frequency(direction, pair, blend, rate))) as f32;
            let square = m * m;
            temp[pair * 2 + 1] = temp[pair * 2 - 1] * square;
            temp[pair * 2] = temp[pair * 2 - 1] * linear + temp[pair * 2 - 2] * square;
            for i in (2..pair * 2).rev() {
                temp[i] += temp[i - 1] * linear + temp[i - 2] * square;
            }
            temp[1] += temp[0] * linear + square;
            temp[0] += linear;
        }
        if direction == 0 {
            for value in temp.iter_mut().take(pairs * 2) {
                *value *= scratch.scale;
            }
        }
        for i in 0..pairs * 2 {
            scratch.coefficients[direction][i] = (scratch.temp[direction][i] * 65_536.0) as i32;
        }
        pairs * 2
    }
}
