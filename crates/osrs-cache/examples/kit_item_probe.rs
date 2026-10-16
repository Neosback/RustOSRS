//! Decode every kit and item of an installation's cache; report failures and a few facts.
use osrs_cache::{
    decode::{DecoderContext, decode_item_definition, decode_kit_definition},
    openrune::installation::Installation,
    transport::CacheRepository,
};
use osrs_core::ids::{ItemId, KitId};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args()
        .nth(1)
        .ok_or("usage: kit_item_probe <OpenRune root>")?;
    let installation = Installation::read(&root)?;
    let profile = installation.profile(false)?;
    let context = DecoderContext::from_profile(&profile)?;
    let repository = CacheRepository::open(installation.root.join(".data/cache/LIVE"), &profile)?;
    let mut kit_failures = 0;
    let kits = repository.read_group_files(2, 3)?;
    let mut selectable = std::collections::BTreeMap::<u8, u32>::new();
    for (id, file) in &kits {
        match decode_kit_definition(KitId::new(*id), &file.bytes, &context, &file.provenance) {
            Ok(kit) => {
                if !kit.non_selectable
                    && let Some(part) = kit.body_part
                {
                    *selectable.entry(part).or_default() += 1;
                }
            }
            Err(error) => {
                kit_failures += 1;
                if kit_failures <= 5 {
                    eprintln!("{error}");
                }
            }
        }
    }
    println!(
        "kits={} failures={kit_failures} selectable_by_part={selectable:?}",
        kits.len()
    );
    let items = repository.read_group_files(2, 10)?;
    let (mut failures, mut wearable, mut both_genders) = (0, 0, 0);
    let mut shown = 0;
    for (id, file) in &items {
        match decode_item_definition(ItemId::new(*id), &file.bytes, &context, &file.provenance) {
            Ok(item) => {
                if item.male.models[0].is_some() || item.female.models[0].is_some() {
                    wearable += 1;
                    both_genders +=
                        u32::from(item.male.models[0].is_some() && item.female.models[0].is_some());
                }
            }
            Err(error) => {
                failures += 1;
                if shown < 6 {
                    shown += 1;
                    eprintln!("{error}");
                }
            }
        }
    }
    println!(
        "items={} failures={failures} wearable={wearable} wearable_both_genders={both_genders}",
        items.len()
    );
    Ok(())
}
