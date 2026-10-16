//! Synthesised sound effects (instrument format), archive 4 with one file per group.
//! Layouts follow the deobfuscated client's `SoundEffect`/`Instrument` readers. The sampled
//! (Vorbis) variant, which has two files in its group, is a different format and is rejected by
//! the caller before it gets here.
use super::{
    ArchiveFileProvenance, BinaryReader, ByteSpan, DecodeResult, DecodeSubject, DecoderContext,
};
use osrs_core::audio::{
    AudioFilter, INSTRUMENT_SLOTS, Instrument, OSCILLATORS, SoundEffect, SoundEnvelope,
};

/// Pole/zero pairs per direction the client's coefficient arrays can hold.
const MAX_FILTER_PAIRS: i32 = 4;

pub fn decode_sound_effect(
    id: u32,
    bytes: &[u8],
    context: &DecoderContext,
    source: &ArchiveFileProvenance,
) -> DecodeResult<SoundEffect> {
    let mut r =
        BinaryReader::new(bytes, context, source).with_subject(DecodeSubject::SoundEffect(id));
    let mut instruments: [Option<Instrument>; INSTRUMENT_SLOTS] = Default::default();
    for slot in &mut instruments {
        if r.peek_u8()? == 0 {
            r.read_u8()?;
        } else {
            *slot = Some(instrument(&mut r)?);
        }
    }
    let start = i32::from(r.read_u16_be()?);
    let end = i32::from(r.read_u16_be()?);
    r.finish()?;
    Ok(SoundEffect {
        instruments,
        start,
        end,
    })
}

fn envelope(r: &mut BinaryReader<'_>) -> DecodeResult<SoundEnvelope> {
    let form = r.read_u8()?;
    let start = r.read_i32_be()?;
    let end = r.read_i32_be()?;
    let (durations, phases) = segments(r)?;
    Ok(SoundEnvelope {
        form,
        start,
        end,
        durations,
        phases,
    })
}

fn segments(r: &mut BinaryReader<'_>) -> DecodeResult<(Vec<u16>, Vec<u16>)> {
    let offset = r.offset();
    let count = r.read_u8()?;
    if count == 0 {
        return Err(r.invalid_value(
            "envelope segments",
            "an envelope needs at least one segment",
            ByteSpan::new(offset, 1),
            None,
        ));
    }
    let (mut durations, mut phases) = (Vec::new(), Vec::new());
    for _ in 0..count {
        durations.push(r.read_u16_be()?);
        phases.push(r.read_u16_be()?);
    }
    Ok((durations, phases))
}

/// A pair of envelopes that is present when its marker byte is non-zero (the marker is not
/// consumed: the first envelope starts with it as its form).
fn optional_pair(r: &mut BinaryReader<'_>) -> DecodeResult<Option<(SoundEnvelope, SoundEnvelope)>> {
    if r.peek_u8()? == 0 {
        r.read_u8()?;
        return Ok(None);
    }
    Ok(Some((envelope(r)?, envelope(r)?)))
}

fn instrument(r: &mut BinaryReader<'_>) -> DecodeResult<Instrument> {
    let pitch = envelope(r)?;
    let volume = envelope(r)?;
    let pitch_modifier = optional_pair(r)?;
    let volume_multiplier = optional_pair(r)?;
    let gate = optional_pair(r)?;
    let mut oscillator_volume = [0; OSCILLATORS];
    let mut oscillator_pitch = [0; OSCILLATORS];
    let mut oscillator_delay = [0; OSCILLATORS];
    // The stream allows ten oscillators but the client's tables hold five.
    for index in 0..10 {
        let offset = r.offset();
        let volume = i32::from(r.read_unsigned_short_smart()?);
        if volume == 0 {
            break;
        }
        if index >= OSCILLATORS {
            return Err(r.invalid_value(
                "oscillators",
                "more than five oscillators",
                ByteSpan::new(offset, 1),
                None,
            ));
        }
        oscillator_volume[index] = volume;
        oscillator_pitch[index] = r.read_short_smart()?;
        oscillator_delay[index] = i32::from(r.read_unsigned_short_smart()?);
    }
    let delay_time = i32::from(r.read_unsigned_short_smart()?);
    let delay_decay = i32::from(r.read_unsigned_short_smart()?);
    let duration = i32::from(r.read_u16_be()?);
    let offset = i32::from(r.read_u16_be()?);
    let mut filter_envelope = SoundEnvelope::default();
    let filter = filter(r, &mut filter_envelope)?;
    Ok(Instrument {
        duration,
        offset,
        pitch,
        volume,
        pitch_modifier,
        volume_multiplier,
        gate,
        oscillator_volume,
        oscillator_pitch,
        oscillator_delay,
        delay_time,
        delay_decay,
        filter,
        filter_envelope,
    })
}

fn filter(r: &mut BinaryReader<'_>, envelope: &mut SoundEnvelope) -> DecodeResult<AudioFilter> {
    let mut filter = AudioFilter::default();
    let offset = r.offset();
    let combined = r.read_u8()?;
    filter.pairs = [i32::from(combined >> 4), i32::from(combined & 15)];
    if filter.pairs.iter().any(|p| *p > MAX_FILTER_PAIRS) {
        return Err(r.invalid_value(
            "filter pairs",
            "more than four pole/zero pairs per direction",
            ByteSpan::new(offset, 1),
            None,
        ));
    }
    if combined == 0 {
        return Ok(filter);
    }
    filter.gain = [i32::from(r.read_u16_be()?), i32::from(r.read_u16_be()?)];
    let flags = r.read_u8()?;
    for direction in 0..2 {
        for pair in 0..filter.pairs[direction] as usize {
            filter.pole_frequency[direction][0][pair] = i32::from(r.read_u16_be()?);
            filter.zero_magnitude[direction][0][pair] = i32::from(r.read_u16_be()?);
        }
    }
    for direction in 0..2 {
        for pair in 0..filter.pairs[direction] as usize {
            if flags & (1 << (direction * 4 + pair)) != 0 {
                filter.pole_frequency[direction][1][pair] = i32::from(r.read_u16_be()?);
                filter.zero_magnitude[direction][1][pair] = i32::from(r.read_u16_be()?);
            } else {
                filter.pole_frequency[direction][1][pair] =
                    filter.pole_frequency[direction][0][pair];
                filter.zero_magnitude[direction][1][pair] =
                    filter.zero_magnitude[direction][0][pair];
            }
        }
    }
    if flags != 0 || filter.gain[0] != filter.gain[1] {
        let (durations, phases) = segments(r)?;
        envelope.durations = durations;
        envelope.phases = phases;
    }
    Ok(filter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::test_support::{source, target_context};

    fn decode(bytes: &[u8]) -> DecodeResult<SoundEffect> {
        let context = target_context().unwrap_or_else(|e| unreachable!("{e:?}"));
        decode_sound_effect(1, bytes, &context, &source())
    }

    /// One envelope: form, start, end, one segment.
    fn env(form: u8) -> Vec<u8> {
        let mut b = vec![form, 0, 0, 1, 0xb8, 0, 0, 1, 0xb8, 1];
        b.extend([0, 0, 0xff, 0xff]);
        b
    }

    /// A minimal instrument: pitch, volume, no modifiers, one oscillator, no filter.
    fn instrument_bytes() -> Vec<u8> {
        let mut b = env(2);
        b.extend(env(0));
        b.extend([0, 0, 0]); // no pitch modifier, volume multiplier or gate
        b.extend([100, 0x40, 0]); // oscillator: volume 100, pitch smart 64 -> 0, delay 0
        b.push(0); // end of oscillators
        b.extend([0, 100]); // echo delay 0, decay 100
        b.extend([0x01, 0xf4, 0, 0]); // duration 500, offset 0
        b.push(0); // no filter
        b
    }

    fn effect_bytes(instruments: usize) -> Vec<u8> {
        let mut b = Vec::new();
        for slot in 0..10 {
            if slot < instruments {
                b.extend(instrument_bytes());
            } else {
                b.push(0);
            }
        }
        b.extend([0, 0, 0, 0]);
        b
    }

    #[test]
    fn a_minimal_effect_decodes_and_renders() {
        let effect = decode(&effect_bytes(2)).unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(effect.instruments.iter().flatten().count(), 2);
        let first = effect.instruments[0]
            .as_ref()
            .unwrap_or_else(|| unreachable!());
        assert_eq!((first.duration, first.offset), (500, 0));
        assert_eq!(first.oscillator_volume[0], 100);
        assert!(!effect.render().samples.is_empty());
    }

    #[test]
    fn every_truncation_and_trailing_bytes_are_rejected() {
        let bytes = effect_bytes(1);
        for end in 0..bytes.len() {
            assert!(decode(&bytes[..end]).is_err(), "truncated at {end}");
        }
        let mut extra = bytes;
        extra.push(0);
        assert!(decode(&extra).is_err());
    }

    #[test]
    fn an_envelope_without_segments_is_rejected() {
        let mut bytes = Vec::new();
        bytes.extend([2, 0, 0, 0, 0, 0, 0, 0, 0, 0]); // form 2, start, end, zero segments
        assert!(decode(&bytes).is_err());
    }
}
