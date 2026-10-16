use super::{
    OSCILLATORS,
    envelope::{Cursor, SoundEnvelope},
    filter::{AudioFilter, Scratch},
};
use std::sync::OnceLock;

/// One voice of a sound effect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instrument {
    /// Length in milliseconds.
    pub duration: i32,
    /// Start delay in milliseconds.
    pub offset: i32,
    pub pitch: SoundEnvelope,
    pub volume: SoundEnvelope,
    pub pitch_modifier: Option<(SoundEnvelope, SoundEnvelope)>,
    pub volume_multiplier: Option<(SoundEnvelope, SoundEnvelope)>,
    /// Gate envelopes (`release`, `attack`) that chop the sound.
    pub gate: Option<(SoundEnvelope, SoundEnvelope)>,
    pub oscillator_volume: [i32; OSCILLATORS],
    pub oscillator_pitch: [i32; OSCILLATORS],
    pub oscillator_delay: [i32; OSCILLATORS],
    pub delay_time: i32,
    pub delay_decay: i32,
    pub filter: AudioFilter,
    pub filter_envelope: SoundEnvelope,
}

/// `java.util.Random` seeded with 0: the client builds its noise table from it.
struct JavaRandom(i64);

impl JavaRandom {
    fn new(seed: i64) -> Self {
        Self((seed ^ 0x5_DEEC_E66D) & ((1 << 48) - 1))
    }

    fn next_int(&mut self) -> i32 {
        self.0 = (self.0.wrapping_mul(0x5_DEEC_E66D).wrapping_add(0xB)) & ((1 << 48) - 1);
        (self.0 >> 16) as i32
    }
}

struct Tables {
    noise: Vec<i32>,
    sine: Vec<i32>,
}

fn tables() -> &'static Tables {
    static TABLES: OnceLock<Tables> = OnceLock::new();
    TABLES.get_or_init(|| {
        let mut random = JavaRandom::new(0);
        Tables {
            noise: (0..32_768).map(|_| (random.next_int() & 2) - 1).collect(),
            // The client divides by 5215.1903, not the exact 32768 / (2 pi); the difference moves
            // some samples by one unit, so the constant is kept.
            sine: (0..32_768)
                .map(|i| (f64::sin(f64::from(i) / 5_215.190_3) * 16_384.0) as i32)
                .collect(),
        }
    })
}

fn wave(tables: &Tables, phase: i32, amplitude: i32, form: u8) -> i32 {
    match form {
        1 => {
            if (phase & 32_767) < 16_384 {
                amplitude
            } else {
                amplitude.wrapping_neg()
            }
        }
        2 => tables.sine[(phase & 32_767) as usize].wrapping_mul(amplitude) >> 14,
        3 => (amplitude.wrapping_mul(phase & 32_767) >> 14).wrapping_sub(amplitude),
        4 => amplitude.wrapping_mul(tables.noise[((phase / 2607) & 32_767) as usize]),
        _ => 0,
    }
}

impl Instrument {
    /// Render `sample_count` samples of a voice lasting `duration_ms` at `rate` Hz. The result
    /// is clamped to 16 bits and may hold values past `sample_count` only as zeros.
    pub fn synthesize(&self, sample_count: i32, duration_ms: i32, rate: i32) -> Vec<i32> {
        let count = sample_count.max(0) as usize;
        let mut samples = vec![0i32; count];
        if duration_ms < 10 {
            return samples;
        }
        let tables = tables();
        let per_ms = f64::from(sample_count) / f64::from(duration_ms);
        let mut pitch = Cursor::new(&self.pitch);
        let mut volume = Cursor::new(&self.volume);
        let (mut pitch_mod, mut pitch_mod_step, mut pitch_mod_base, mut pitch_mod_phase) =
            (None, 0, 0, 0i32);
        if let Some((rate_env, amp_env)) = &self.pitch_modifier {
            pitch_mod = Some((Cursor::new(rate_env), Cursor::new(amp_env), rate_env.form));
            pitch_mod_step = (f64::from(rate_env.end - rate_env.start) * 32.768 / per_ms) as i32;
            pitch_mod_base = (f64::from(rate_env.start) * 32.768 / per_ms) as i32;
        }
        let (mut volume_mod, mut volume_mod_step, mut volume_mod_base, mut volume_mod_phase) =
            (None, 0, 0, 0i32);
        if let Some((rate_env, amp_env)) = &self.volume_multiplier {
            volume_mod = Some((Cursor::new(rate_env), Cursor::new(amp_env), rate_env.form));
            volume_mod_step = (f64::from(rate_env.end - rate_env.start) * 32.768 / per_ms) as i32;
            volume_mod_base = (f64::from(rate_env.start) * 32.768 / per_ms) as i32;
        }
        let mut phases = [0i32; OSCILLATORS];
        let mut delays = [0i32; OSCILLATORS];
        let mut volume_steps = [0i32; OSCILLATORS];
        let mut pitch_steps = [0i32; OSCILLATORS];
        let mut pitch_base_steps = [0i32; OSCILLATORS];
        for o in 0..OSCILLATORS {
            if self.oscillator_volume[o] != 0 {
                delays[o] = (f64::from(self.oscillator_delay[o]) * per_ms) as i32;
                volume_steps[o] = (self.oscillator_volume[o] << 14) / 100;
                pitch_steps[o] = (f64::from(self.pitch.end - self.pitch.start)
                    * 32.768
                    * f64::powf(1.005_792_941_067_853_4, f64::from(self.oscillator_pitch[o]))
                    / per_ms) as i32;
                pitch_base_steps[o] = (f64::from(self.pitch.start) * 32.768 / per_ms) as i32;
            }
        }
        for sample in 0..sample_count {
            let mut pitch_value = pitch.step(sample_count);
            let mut volume_value = volume.step(sample_count);
            if let Some((rate_cursor, amp_cursor, form)) = &mut pitch_mod {
                let rate_value = rate_cursor.step(sample_count);
                let amp_value = amp_cursor.step(sample_count);
                pitch_value =
                    pitch_value.wrapping_add(wave(tables, pitch_mod_phase, amp_value, *form) >> 1);
                pitch_mod_phase = pitch_mod_phase
                    .wrapping_add(pitch_mod_base)
                    .wrapping_add(rate_value.wrapping_mul(pitch_mod_step) >> 16);
            }
            if let Some((rate_cursor, amp_cursor, form)) = &mut volume_mod {
                let rate_value = rate_cursor.step(sample_count);
                let amp_value = amp_cursor.step(sample_count);
                volume_value = volume_value
                    .wrapping_mul((wave(tables, volume_mod_phase, amp_value, *form) >> 1) + 32_768)
                    >> 15;
                volume_mod_phase = volume_mod_phase
                    .wrapping_add(volume_mod_base)
                    .wrapping_add(rate_value.wrapping_mul(volume_mod_step) >> 16);
            }
            for o in 0..OSCILLATORS {
                if self.oscillator_volume[o] == 0 {
                    continue;
                }
                let at = delays[o].wrapping_add(sample);
                if at < sample_count {
                    let level = volume_value.wrapping_mul(volume_steps[o]) >> 15;
                    samples[at as usize] = samples[at as usize].wrapping_add(wave(
                        tables,
                        phases[o],
                        level,
                        self.pitch.form,
                    ));
                    phases[o] = phases[o]
                        .wrapping_add(pitch_base_steps[o])
                        .wrapping_add(pitch_value.wrapping_mul(pitch_steps[o]) >> 16);
                }
            }
        }
        if let Some((release, attack)) = &self.gate {
            let mut release_cursor = Cursor::new(release);
            let mut attack_cursor = Cursor::new(attack);
            let mut counter = 0i32;
            let mut muted = true;
            for sample in samples.iter_mut() {
                let release_value = release_cursor.step(sample_count);
                let attack_value = attack_cursor.step(sample_count);
                let level = if muted { release_value } else { attack_value };
                let threshold =
                    (level.wrapping_mul(release.end - release.start) >> 8) + release.start;
                counter += 256;
                if counter >= threshold {
                    counter = 0;
                    muted = !muted;
                }
                if muted {
                    *sample = 0;
                }
            }
        }
        if self.delay_time > 0 && self.delay_decay > 0 {
            let delay = (f64::from(self.delay_time) * per_ms) as usize;
            for i in delay..count {
                samples[i] = samples[i]
                    .wrapping_add(samples[i - delay].wrapping_mul(self.delay_decay) / 100);
            }
        }
        if self.filter.active() {
            self.apply_filter(&mut samples, rate);
        }
        for sample in &mut samples {
            *sample = (*sample).clamp(-32_768, 32_767);
        }
        samples
    }

    fn apply_filter(&self, samples: &mut [i32], rate: i32) {
        let count = samples.len() as i32;
        let mut scratch = Scratch::default();
        let mut cursor = Cursor::new(&self.filter_envelope);
        let mut level = cursor.step(count + 1);
        let blend = |level: i32| level as f32 / 65_536.0;
        let mut forward = self.filter.compute(&mut scratch, 0, blend(level), rate) as i32;
        let mut backward = self.filter.compute(&mut scratch, 1, blend(level), rate) as i32;
        if count < forward + backward {
            return;
        }
        let mac = |sample: i32, value: i32| i64::from(sample) * i64::from(value);
        let mut at = 0i32;
        let mut limit = backward.min(count - forward);
        // Lead-in: the feedback taps reach back past the start of the buffer.
        while at < limit {
            let mut acc =
                (mac(samples[(at + forward) as usize], scratch.forward_multiplier) >> 16) as i32;
            for i in 0..forward {
                acc = acc.wrapping_add(
                    (mac(
                        samples[(at + forward - 1 - i) as usize],
                        scratch.coefficients[0][i as usize],
                    ) >> 16) as i32,
                );
            }
            for i in 0..at {
                acc = acc.wrapping_sub(
                    (mac(
                        samples[(at - 1 - i) as usize],
                        scratch.coefficients[1][i as usize],
                    ) >> 16) as i32,
                );
            }
            samples[at as usize] = acc;
            level = cursor.step(count + 1);
            at += 1;
        }
        limit = 128;
        loop {
            if limit > count - forward {
                limit = count - forward;
            }
            while at < limit {
                let mut acc = (mac(samples[(at + forward) as usize], scratch.forward_multiplier)
                    >> 16) as i32;
                for i in 0..forward {
                    acc = acc.wrapping_add(
                        (mac(
                            samples[(at + forward - 1 - i) as usize],
                            scratch.coefficients[0][i as usize],
                        ) >> 16) as i32,
                    );
                }
                for i in 0..backward {
                    acc = acc.wrapping_sub(
                        (mac(
                            samples[(at - 1 - i) as usize],
                            scratch.coefficients[1][i as usize],
                        ) >> 16) as i32,
                    );
                }
                samples[at as usize] = acc;
                level = cursor.step(count + 1);
                at += 1;
            }
            if at >= count - forward {
                while at < count {
                    let mut acc = 0i32;
                    for i in (at + forward - count)..forward {
                        acc = acc.wrapping_add(
                            (mac(
                                samples[(at + forward - 1 - i) as usize],
                                scratch.coefficients[0][i as usize],
                            ) >> 16) as i32,
                        );
                    }
                    for i in 0..backward {
                        acc = acc.wrapping_sub(
                            (mac(
                                samples[(at - 1 - i) as usize],
                                scratch.coefficients[1][i as usize],
                            ) >> 16) as i32,
                        );
                    }
                    samples[at as usize] = acc;
                    cursor.step(count + 1);
                    at += 1;
                }
                break;
            }
            forward = self.filter.compute(&mut scratch, 0, blend(level), rate) as i32;
            backward = self.filter.compute(&mut scratch, 1, blend(level), rate) as i32;
            limit += 128;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_noise_table_starts_with_java_random_seed_zero() {
        // new java.util.Random(0): nextInt() is -1155484576, then -723955400.
        let mut random = JavaRandom::new(0);
        assert_eq!(random.next_int(), -1_155_484_576);
        assert_eq!(random.next_int(), -723_955_400);
        // Noise is the second bit of each, mapped to -1 or +1.
        let t = tables();
        assert_eq!(t.noise[0], (-1_155_484_576 & 2) - 1);
        assert!(t.noise.iter().all(|v| *v == -1 || *v == 1));
    }

    #[test]
    fn waves_follow_the_client_formulas() {
        let t = tables();
        assert_eq!(wave(t, 0, 1000, 1), 1000);
        assert_eq!(wave(t, 20_000, 1000, 1), -1000);
        assert_eq!(wave(t, 0, 1000, 3), -1000, "saw starts at minus full");
        assert_eq!(wave(t, 8192, 16_384, 2), t.sine[8192]);
        assert_eq!(wave(t, 5, 100, 0), 0);
        assert!(t.sine[8192] > 16_000 && t.sine[24_576] < -16_000);
    }
}
