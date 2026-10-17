//! Name, size and actions of objects by id, for building kits by hand.
//! usage: loc_info <installed OpenRune root> <id | word> ...  (a word lists every object whose name or
//! symbol contains it)
use osrs_cache::{
    decode::{DecoderContext, decode_object_definition},
    openrune::installation::Installation,
    transport::CacheRepository,
};
use osrs_core::ids::ObjectId;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let root = args
        .next()
        .ok_or("usage: loc_info <installed OpenRune root> <id>...")?;
    let installation = Installation::read(&root)?;
    let profile = installation.profile(false)?;
    let context = DecoderContext::from_profile(&profile)?;
    let repository = CacheRepository::open(installation.root.join(".data/cache/LIVE"), &profile)?;
    let symbols: std::collections::HashMap<u32, &str> = installation
        .symbols
        .table_symbols("loc")
        .into_iter()
        .collect();
    let files: std::collections::BTreeMap<_, _> =
        repository.read_group_files(2, 6)?.into_iter().collect();
    let wanted: Vec<String> = args.collect();
    let mut ids = Vec::new();
    for want in &wanted {
        if let Ok(id) = want.parse::<u32>() {
            ids.push(id);
        } else {
            let word = want.to_lowercase();
            for (id, file) in &files {
                let symbol = symbols.get(id).copied().unwrap_or_default().to_lowercase();
                let name = decode_object_definition(
                    ObjectId::new(*id),
                    &file.bytes,
                    &context,
                    &file.provenance,
                )
                .ok()
                .and_then(|d| d.name)
                .unwrap_or_default()
                .to_lowercase();
                if symbol.contains(&word) || name.contains(&word) {
                    ids.push(*id);
                }
            }
        }
    }
    for id in ids {
        let Some(file) = files.get(&id) else {
            println!("{id}: not in the cache");
            continue;
        };
        match decode_object_definition(ObjectId::new(id), &file.bytes, &context, &file.provenance) {
            Ok(def) => println!(
                "{id}: name={:?} symbol={:?} size={}x{} actions={:?} models={}",
                def.name,
                symbols.get(&id),
                def.size_x,
                def.size_y,
                def.actions.iter().flatten().collect::<Vec<_>>(),
                def.models.as_ref().map_or(0, |_| 1)
            ),
            Err(error) => println!("{id}: {error}"),
        }
    }
    Ok(())
}
