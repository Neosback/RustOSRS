//! `sound ...` script commands (smoke tests): `play ID [loop]`, `stop`, `info`, `open N`.
use crate::app::{EditorApp, audio_runtime::SoundState};
use std::str::SplitWhitespace;

/// `Ok(true)` when done, `Ok(false)` to retry next frame.
pub(in crate::app) fn command(
    app: &mut EditorApp,
    words: &mut SplitWhitespace<'_>,
) -> Result<bool, String> {
    match words.next().unwrap_or_default() {
        "play" => {
            let id: u32 = words
                .next()
                .ok_or("sound id is missing")?
                .parse()
                .map_err(|_| "sound id is not a number")?;
            let looped = words.next() == Some("loop");
            app.audio.play(&app.world, id, looped);
        }
        "stop" => app.audio.stop(),
        "open" => {
            // Open the nth listed object's page.
            let n: usize = words
                .next()
                .ok_or("index is missing")?
                .parse()
                .map_err(|_| "index is not a number")?;
            super::refresh(app);
            let Some(key) = app
                .ui
                .sound
                .cache
                .as_ref()
                .and_then(|c| c.rows.get(n))
                .map(|r| (r.region, r.index))
            else {
                return Ok(false);
            };
            app.ui.sound.open_object(key);
        }
        "info" => {
            let state = |id| match app.audio.state(id) {
                None => "unrequested".to_owned(),
                Some(SoundState::Loading) => "rendering".to_owned(),
                Some(SoundState::Ready(s)) => format!("ready {} samples", s.samples.len()),
                Some(SoundState::Failed(e)) => format!("failed: {e}"),
            };
            let ids: Vec<u32> = app
                .ui
                .sound
                .cache
                .iter()
                .flat_map(|c| c.rows.iter())
                .filter_map(|r| r.main_sound())
                .take(3)
                .collect();
            eprintln!(
                "smoke: sound info: playing={:?} waiting={:?} error={:?} rows={} first={:?}",
                app.audio.playing(),
                app.audio.waiting(),
                app.audio.error(),
                app.ui.sound.cache.as_ref().map_or(0, |c| c.rows.len()),
                ids.iter().map(|id| (*id, state(*id))).collect::<Vec<_>>()
            );
        }
        other => return Err(format!("unknown sound command '{other}'")),
    }
    Ok(true)
}
