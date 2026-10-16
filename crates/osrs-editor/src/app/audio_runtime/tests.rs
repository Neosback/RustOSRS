use super::*;
use osrs_core::audio::RawSound;

fn sound() -> Arc<RawSound> {
    Arc::new(RawSound {
        sample_rate: 22_050,
        samples: vec![0, 100, -100],
        start: 0,
        end: 0,
    })
}

#[test]
fn a_rendered_sound_nobody_waits_for_is_kept_and_not_played() {
    let mut audio = AudioRuntime::default();
    audio.arrived(7, Ok(sound()));
    assert!(matches!(audio.state(7), Some(SoundState::Ready(_))));
    assert_eq!(audio.playing(), None);
    assert_eq!(audio.waiting(), None);
    assert_eq!(audio.error(), None);
}

#[test]
fn a_failed_sound_the_user_asked_for_is_reported_not_substituted() {
    let mut audio = AudioRuntime {
        waiting: Some((9, false)),
        ..AudioRuntime::default()
    };
    audio.arrived(9, Err("sound 9 is a sampled (Vorbis) effect".into()));
    assert!(matches!(audio.state(9), Some(SoundState::Failed(_))));
    assert_eq!(audio.waiting(), None);
    assert_eq!(audio.error(), Some("sound 9 is a sampled (Vorbis) effect"));
    assert_eq!(audio.playing(), None);
}

#[test]
fn a_result_for_another_sound_does_not_cancel_the_wait() {
    let mut audio = AudioRuntime {
        waiting: Some((5, true)),
        ..AudioRuntime::default()
    };
    audio.arrived(6, Err("unrelated".into()));
    assert_eq!(audio.waiting(), Some(5));
    assert_eq!(audio.error(), None);
}

#[test]
fn opening_a_new_cache_forgets_every_rendered_sound_and_stops_playback() {
    let mut audio = AudioRuntime::default();
    audio.arrived(7, Ok(sound()));
    audio.waiting = Some((8, false));
    audio.source_opened();
    assert!(audio.state(7).is_none());
    assert_eq!(audio.waiting(), None);
}
