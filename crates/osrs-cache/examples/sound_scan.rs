//! Decode and render every synthesised sound effect of a cache; report failures and statistics.
use osrs_cache::{
    decode::{DecoderContext, decode_sound_effect},
    openrune::installation::Installation,
    transport::CacheRepository,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args()
        .nth(1)
        .ok_or("usage: sound_scan <OpenRune root>")?;
    let installation = Installation::read(&root)?;
    let profile = installation.profile(false)?;
    let context = DecoderContext::from_profile(&profile)?;
    let repository = CacheRepository::open(installation.root.join(".data/cache/LIVE"), &profile)?;
    let (mut synth, mut sampled, mut failures, mut panics, mut silent) = (0, 0, 0, 0, 0);
    let (mut longest, mut samples_total) = (0usize, 0usize);
    let mut shown = 0;
    for group in repository.group_ids(4)? {
        let files = repository.read_group_files(4, group)?;
        // A group holding file 0 is the synthesised form; the sampled form is file 1.
        let Some((_, file)) = files.into_iter().find(|(id, _)| *id == 0) else {
            sampled += 1;
            continue;
        };
        match decode_sound_effect(group, &file.bytes, &context, &file.provenance) {
            Err(error) => {
                failures += 1;
                if shown < 8 {
                    shown += 1;
                    eprintln!("{error}");
                }
            }
            Ok(mut effect) => {
                synth += 1;
                let rendered = std::panic::catch_unwind(move || {
                    effect.calculate_delay();
                    effect.render()
                });
                match rendered {
                    Err(_) => panics += 1,
                    Ok(raw) => {
                        longest = longest.max(raw.samples.len());
                        samples_total += raw.samples.len();
                        silent += usize::from(raw.samples.iter().all(|s| *s == 0));
                    }
                }
            }
        }
    }
    println!(
        "synth={synth} sampled(two-file)={sampled} decode_failures={failures} render_panics={panics} \
         all_silent={silent} longest_samples={longest} total_samples={samples_total}"
    );
    Ok(())
}
