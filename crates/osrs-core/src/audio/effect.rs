use super::{INSTRUMENT_SLOTS, Instrument, SAMPLE_RATE};

/// Mixed 16-bit mono audio and its loop points (in samples).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawSound {
    pub sample_rate: i32,
    pub samples: Vec<i16>,
    pub start: i32,
    pub end: i32,
}

/// Up to ten instruments mixed together, with loop bounds in milliseconds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoundEffect {
    pub instruments: [Option<Instrument>; INSTRUMENT_SLOTS],
    pub start: i32,
    pub end: i32,
}

impl SoundEffect {
    /// Remove silence before the first voice and return the removed time in client cycles of 20 ms
    /// (the game plays the sound that much later instead of storing the gap).
    pub fn calculate_delay(&mut self) -> i32 {
        let mut delay = 9_999_999;
        for instrument in self.instruments.iter().flatten() {
            delay = delay.min(instrument.offset / 20);
        }
        if self.start < self.end {
            delay = delay.min(self.start / 20);
        }
        if delay == 9_999_999 || delay == 0 {
            return 0;
        }
        for instrument in self.instruments.iter_mut().flatten() {
            instrument.offset -= delay * 20;
        }
        if self.start < self.end {
            self.start -= delay * 20;
            self.end -= delay * 20;
        }
        delay
    }

    /// Mix every instrument at 22,050 Hz with 16-bit saturation.
    pub fn render(&self) -> RawSound {
        let total = self
            .instruments
            .iter()
            .flatten()
            .map(|i| i.duration + i.offset)
            .max()
            .unwrap_or(0);
        let mut mixed = vec![0i16; (total.max(0) * SAMPLE_RATE / 1000) as usize];
        for instrument in self.instruments.iter().flatten() {
            let length = instrument.duration * SAMPLE_RATE / 1000;
            let delay = (instrument.offset * SAMPLE_RATE / 1000) as usize;
            let voice = instrument.synthesize(length, instrument.duration, SAMPLE_RATE);
            for (i, value) in voice.iter().take(length.max(0) as usize).enumerate() {
                if let Some(slot) = mixed.get_mut(i + delay) {
                    *slot = (i32::from(*slot) + value).clamp(-32_768, 32_767) as i16;
                }
            }
        }
        RawSound {
            sample_rate: SAMPLE_RATE,
            samples: mixed,
            start: SAMPLE_RATE * self.start / 1000,
            end: SAMPLE_RATE * self.end / 1000,
        }
    }
}

#[cfg(test)]
mod tests;
