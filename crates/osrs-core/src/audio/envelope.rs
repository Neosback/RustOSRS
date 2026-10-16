/// A piecewise-linear control curve: `durations[i]` (fraction of the instrument, out of 65536)
/// and `phases[i]` (the level reached, out of 65535) per segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoundEnvelope {
    /// Waveform: 1 square, 2 sine, 3 saw, 4 noise (0 and others are silent).
    pub form: u8,
    pub start: i32,
    pub end: i32,
    pub durations: Vec<u16>,
    pub phases: Vec<u16>,
}

impl Default for SoundEnvelope {
    /// The client's default curve, used when a stream does not supply one.
    fn default() -> Self {
        Self {
            form: 0,
            start: 0,
            end: 0,
            durations: vec![0, 65_535],
            phases: vec![0, 65_535],
        }
    }
}

/// Position along an envelope while one instrument renders (the client keeps this in the
/// envelope object itself).
pub(super) struct Cursor<'a> {
    envelope: &'a SoundEnvelope,
    ticks: i32,
    phase_index: usize,
    step: i32,
    amplitude: i32,
    count: i32,
}

impl<'a> Cursor<'a> {
    pub(super) fn new(envelope: &'a SoundEnvelope) -> Self {
        Self {
            envelope,
            ticks: 0,
            phase_index: 0,
            step: 0,
            amplitude: 0,
            count: 0,
        }
    }

    /// Advance one sample of a buffer `length` samples long and return the current level.
    pub(super) fn step(&mut self, length: i32) -> i32 {
        let segments = self.envelope.phases.len();
        if self.count >= self.ticks {
            self.amplitude = i32::from(self.envelope.phases[self.phase_index]) << 15;
            self.phase_index += 1;
            if self.phase_index >= segments {
                self.phase_index = segments - 1;
            }
            self.ticks = (f64::from(self.envelope.durations[self.phase_index]) / 65_536.0
                * f64::from(length)) as i32;
            if self.ticks > self.count {
                let target = i32::from(self.envelope.phases[self.phase_index]) << 15;
                self.step = target.wrapping_sub(self.amplitude) / (self.ticks - self.count);
            }
        }
        self.amplitude = self.amplitude.wrapping_add(self.step);
        self.count += 1;
        self.amplitude.wrapping_sub(self.step) >> 15
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_curve_ramps_from_zero_to_full_over_the_buffer() {
        let envelope = SoundEnvelope::default();
        let mut cursor = Cursor::new(&envelope);
        let levels: Vec<i32> = (0..100).map(|_| cursor.step(100)).collect();
        assert_eq!(levels[0], 0);
        assert!(levels.windows(2).all(|w| w[0] <= w[1]), "never decreases");
        assert!(levels[99] > 60_000, "ends near full level: {}", levels[99]);
    }

    #[test]
    fn a_single_segment_envelope_holds_its_level() {
        let envelope = SoundEnvelope {
            durations: vec![0],
            phases: vec![30_000],
            ..SoundEnvelope::default()
        };
        let mut cursor = Cursor::new(&envelope);
        for _ in 0..50 {
            assert_eq!(cursor.step(50), 30_000);
        }
    }
}
