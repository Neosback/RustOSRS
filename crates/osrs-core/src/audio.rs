//! Synthesised sound effects: the client's instrument format, rendered to 16-bit PCM.
//!
//! Most sound-effect groups in the cache hold one file in this format (instruments with
//! envelopes, oscillators, an echo and a filter). The arithmetic follows the deobfuscated client's
//! `SoundEffect`, `Instrument`, `SoundEnvelope` and `AudioFilter` exactly, including 32-bit
//! wrapping integer maths and `float` filter coefficients, so a preview sounds like the game.
//! Parsing lives in `osrs-cache`; this module only holds the values and renders them. The sampled
//! (Vorbis) variant of a sound effect is a different format and is not handled here.

mod effect;
mod envelope;
mod filter;
mod instrument;

pub use effect::{RawSound, SoundEffect};
pub use envelope::SoundEnvelope;
pub use filter::AudioFilter;
pub use instrument::Instrument;

/// Sample rate the client mixes synthesised effects at.
pub const SAMPLE_RATE: i32 = 22_050;
/// Instruments at most this many samples long render without growing the scratch buffer.
pub const INSTRUMENT_SLOTS: usize = 10;
/// Oscillators per instrument.
pub const OSCILLATORS: usize = 5;
