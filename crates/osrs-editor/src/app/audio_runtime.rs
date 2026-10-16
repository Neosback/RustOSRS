//! Sound preview: asks the world worker to render sound effects, keeps what comes back, and
//! plays one at a time. It owns the output device and the rendered sounds; it never touches the
//! map, and what the panel shows about a sound comes from here.
mod output;
#[cfg(test)]
mod tests;

use super::world_runtime::WorldRuntime;
use osrs_core::audio::RawSound;
use std::{collections::HashMap, sync::Arc};

pub(super) enum SoundState {
    Loading,
    Ready(Arc<RawSound>),
    Failed(String),
}

#[derive(Default)]
pub(super) struct AudioRuntime {
    sounds: HashMap<u32, SoundState>,
    output: output::Output,
    /// The sound playing now, and whether it repeats.
    playing: Option<(u32, bool)>,
    /// A play request waiting for its sound to be rendered.
    waiting: Option<(u32, bool)>,
    error: Option<String>,
}

impl AudioRuntime {
    /// A cache was opened: sounds from the old one are not this cache's sounds.
    pub(super) fn source_opened(&mut self) {
        self.output.stop();
        *self = Self {
            output: std::mem::take(&mut self.output),
            ..Self::default()
        };
    }

    pub(super) fn state(&self, id: u32) -> Option<&SoundState> {
        self.sounds.get(&id)
    }

    /// Why the last play failed (no device, unsupported form), for the panel to show.
    pub(super) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// The sound playing now; `None` once it has finished.
    pub(super) fn playing(&self) -> Option<u32> {
        self.playing
            .filter(|_| self.output.busy())
            .map(|(id, _)| id)
    }

    pub(super) fn waiting(&self) -> Option<u32> {
        self.waiting.map(|(id, _)| id)
    }

    /// Start `id`, rendering it first when it has not been. `looped` repeats until stopped.
    pub(super) fn play(&mut self, world: &WorldRuntime, id: u32, looped: bool) {
        self.error = None;
        match self.sounds.get(&id) {
            Some(SoundState::Ready(sound)) => {
                let sound = Arc::clone(sound);
                self.start(id, &sound, looped);
            }
            Some(SoundState::Failed(reason)) => self.error = Some(reason.clone()),
            Some(SoundState::Loading) => self.waiting = Some((id, looped)),
            None => {
                self.sounds.insert(id, SoundState::Loading);
                self.waiting = Some((id, looped));
                world.request_sound(id);
            }
        }
    }

    pub(super) fn stop(&mut self) {
        self.output.stop();
        self.playing = None;
        self.waiting = None;
    }

    /// The worker finished rendering `id`; start it if that is what was asked for.
    pub(super) fn arrived(&mut self, id: u32, result: Result<Arc<RawSound>, String>) {
        let state = match result {
            Ok(sound) => SoundState::Ready(sound),
            Err(reason) => SoundState::Failed(reason),
        };
        self.sounds.insert(id, state);
        if let Some((wanted, looped)) = self.waiting
            && wanted == id
        {
            self.waiting = None;
            match self.sounds.get(&id) {
                Some(SoundState::Ready(sound)) => {
                    let sound = Arc::clone(sound);
                    self.start(id, &sound, looped);
                }
                Some(SoundState::Failed(reason)) => self.error = Some(reason.clone()),
                _ => {}
            }
        }
    }

    fn start(&mut self, id: u32, sound: &RawSound, looped: bool) {
        match self.output.play(sound, looped) {
            Ok(()) => self.playing = Some((id, looped)),
            Err(reason) => {
                self.playing = None;
                self.error = Some(reason);
            }
        }
    }
}
