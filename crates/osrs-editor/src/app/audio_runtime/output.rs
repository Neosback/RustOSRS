//! The audio device. Native builds play through the system's default output; the browser build
//! has no output yet and says so instead of staying silent.
use osrs_core::audio::RawSound;

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use super::RawSound;
    use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player, Source, buffer::SamplesBuffer};
    use std::num::NonZero;

    #[derive(Default)]
    pub struct Output {
        device: Option<MixerDeviceSink>,
        player: Option<Player>,
    }

    impl Output {
        pub fn play(&mut self, sound: &RawSound, looped: bool) -> Result<(), String> {
            if self.device.is_none() {
                self.device = Some(
                    DeviceSinkBuilder::open_default_sink()
                        .map_err(|e| format!("no audio output: {e}"))?,
                );
            }
            let Some(device) = &self.device else {
                return Err("no audio output".into());
            };
            let rate = NonZero::new(u32::try_from(sound.sample_rate).map_err(|e| e.to_string())?)
                .ok_or("sound has no sample rate")?;
            if sound.samples.is_empty() {
                return Err("this sound has no audible samples".into());
            }
            let data: Vec<f32> = sound
                .samples
                .iter()
                .map(|s| f32::from(*s) / 32_768.0)
                .collect();
            let buffer = SamplesBuffer::new(NonZero::<u16>::MIN, rate, data);
            // A new player each time: dropping the old one stops it.
            let player = Player::connect_new(device.mixer());
            if looped {
                player.append(buffer.repeat_infinite());
            } else {
                player.append(buffer);
            }
            self.player = Some(player);
            Ok(())
        }

        pub fn busy(&self) -> bool {
            self.player.as_ref().is_some_and(|p| !p.empty())
        }

        pub fn stop(&mut self) {
            self.player = None;
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::Output;

#[cfg(target_arch = "wasm32")]
#[derive(Default)]
pub struct Output;

#[cfg(target_arch = "wasm32")]
impl Output {
    pub fn play(&mut self, _sound: &RawSound, _looped: bool) -> Result<(), String> {
        Err("Sound playback is not available in the browser build yet".into())
    }
    pub fn busy(&self) -> bool {
        false
    }
    pub fn stop(&mut self) {}
}
