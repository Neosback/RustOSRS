use super::*;
use crate::audio::{AudioFilter, OSCILLATORS, SoundEnvelope};

fn envelope(form: u8, start: i32, end: i32) -> SoundEnvelope {
    SoundEnvelope {
        form,
        start,
        end,
        ..SoundEnvelope::default()
    }
}

/// A plain sine voice: constant pitch, constant volume, one oscillator.
fn voice(duration: i32, offset: i32) -> Instrument {
    let mut oscillator_volume = [0; OSCILLATORS];
    oscillator_volume[0] = 100;
    Instrument {
        duration,
        offset,
        pitch: SoundEnvelope {
            durations: vec![0],
            phases: vec![65_535],
            ..envelope(2, 440, 440)
        },
        volume: SoundEnvelope {
            durations: vec![0],
            phases: vec![30_000],
            ..envelope(0, 0, 0)
        },
        pitch_modifier: None,
        volume_multiplier: None,
        gate: None,
        oscillator_volume,
        oscillator_pitch: [0; OSCILLATORS],
        oscillator_delay: [0; OSCILLATORS],
        delay_time: 0,
        delay_decay: 100,
        filter: AudioFilter::default(),
        filter_envelope: SoundEnvelope::default(),
    }
}

fn effect(instruments: Vec<Instrument>) -> SoundEffect {
    let mut slots: [Option<Instrument>; INSTRUMENT_SLOTS] = Default::default();
    for (slot, instrument) in slots.iter_mut().zip(instruments) {
        *slot = Some(instrument);
    }
    SoundEffect {
        instruments: slots,
        start: 0,
        end: 0,
    }
}

#[test]
fn a_sine_voice_renders_the_right_length_and_a_bounded_oscillation() {
    let sound = effect(vec![voice(500, 0)]).render();
    assert_eq!(sound.sample_rate, 22_050);
    assert_eq!(sound.samples.len(), 500 * 22_050 / 1000);
    let peak = sound
        .samples
        .iter()
        .map(|s| i32::from(*s).abs())
        .max()
        .unwrap_or(0);
    assert!(peak > 1000 && peak <= 32_767, "peak {peak}");
    let crossings = sound
        .samples
        .windows(2)
        .filter(|w| (w[0] < 0) != (w[1] < 0))
        .count();
    assert!(
        crossings > 100,
        "an oscillating signal crosses zero often: {crossings}"
    );
}

#[test]
fn rendering_is_deterministic_and_voices_start_at_their_offset() {
    let a = effect(vec![voice(200, 0), voice(200, 300)]);
    assert_eq!(a.render(), a.render());
    let sound = a.render();
    assert_eq!(sound.samples.len(), 500 * 22_050 / 1000);
    let gap = &sound.samples[(200 * 22_050 / 1000)..(300 * 22_050 / 1000)];
    assert!(gap.iter().all(|s| *s == 0), "silence between the voices");
}

#[test]
fn mixing_saturates_instead_of_wrapping() {
    let loud: Vec<Instrument> = (0..10).map(|_| voice(100, 0)).collect();
    let mut loud = effect(loud);
    for instrument in loud.instruments.iter_mut().flatten() {
        instrument.volume.phases = vec![65_535];
    }
    let sound = loud.render();
    assert!(
        sound
            .samples
            .iter()
            .all(|s| (-32_768..=32_767).contains(&i32::from(*s)))
    );
    assert!(sound.samples.contains(&32_767) || sound.samples.contains(&-32_768));
}

#[test]
fn leading_silence_is_removed_in_whole_client_cycles() {
    let mut sound = effect(vec![voice(100, 140), voice(100, 260)]);
    sound.start = 0;
    sound.end = 0;
    // 140 ms is 7 whole 20 ms cycles; the other offset keeps its distance.
    assert_eq!(sound.calculate_delay(), 7);
    let offsets: Vec<i32> = sound
        .instruments
        .iter()
        .flatten()
        .map(|i| i.offset)
        .collect();
    assert_eq!(offsets, [0, 120]);
    assert_eq!(sound.calculate_delay(), 0, "nothing left to remove");
    assert_eq!(effect(vec![]).calculate_delay(), 0);
}

#[test]
fn short_voices_and_empty_effects_render_silence_or_nothing() {
    assert!(effect(vec![]).render().samples.is_empty());
    let short = voice(5, 0).synthesize(110, 5, 22_050);
    assert!(
        short.iter().all(|s| *s == 0),
        "under 10 ms is silent, as in the client"
    );
}

#[test]
fn loop_points_scale_with_the_sample_rate() {
    let mut sound = effect(vec![voice(1000, 0)]);
    sound.start = 100;
    sound.end = 900;
    let raw = sound.render();
    assert_eq!((raw.start, raw.end), (2205, 19_845));
}
