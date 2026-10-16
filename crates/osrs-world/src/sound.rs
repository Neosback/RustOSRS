//! Sound-effect rendering on the world worker: cache bytes in, 16-bit PCM out.
use crate::WorldDefinitions;
use osrs_core::audio::RawSound;

/// Index of the sound-effect archive; each group is one sound ID.
const SOUND_EFFECT_INDEX: u8 = 4;

/// Render sound effect `id`. The synthesised form (file 0) is supported; the sampled form
/// (file 1, Vorbis packets) is reported as unsupported rather than substituted.
pub fn render_sound_effect(definitions: &WorldDefinitions, id: u32) -> Result<RawSound, String> {
    let files = definitions
        .cache()
        .read_group_files(SOUND_EFFECT_INDEX, id)
        .map_err(|error| format!("sound {id}: {error}"))?;
    let Some((_, file)) = files.iter().find(|(file_id, _)| *file_id == 0) else {
        return Err(if files.iter().any(|(file_id, _)| *file_id == 1) {
            format!("sound {id} is a sampled (Vorbis) effect, which is not supported yet")
        } else {
            format!("sound {id} does not exist in this cache")
        });
    };
    let mut effect = osrs_cache::decode::decode_sound_effect(
        id,
        &file.bytes,
        definitions.context(),
        &file.provenance,
    )
    .map_err(|error| error.to_string())?;
    // The game plays the sound after its leading silence instead of storing the gap.
    effect.calculate_delay();
    Ok(effect.render())
}
