//! How much of the loc (object) ID space is named: cache name, OpenRune gameval symbol, both.
use osrs_cache::{
    decode::{DecoderContext, decode_object_definition},
    openrune::installation::Installation,
    transport::CacheRepository,
};
use osrs_core::ids::ObjectId;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args()
        .nth(1)
        .ok_or("usage: loc_names <installed OpenRune root>")?;
    let installation = Installation::read(&root)?;
    let profile = installation.profile(false)?;
    let context = DecoderContext::from_profile(&profile)?;
    let repository = CacheRepository::open(installation.root.join(".data/cache/LIVE"), &profile)?;
    let symbols: std::collections::HashMap<u32, &str> = installation
        .symbols
        .table_symbols("loc")
        .into_iter()
        .map(|(id, name)| (id, name))
        .collect();
    let files = repository.read_group_files(2, 6)?;
    let (mut total, mut named, mut symbolled, mut both, mut neither, mut failures) =
        (0, 0, 0, 0, 0, 0);
    let mut samples = Vec::new();
    for (id, file) in files {
        let Ok(def) =
            decode_object_definition(ObjectId::new(id), &file.bytes, &context, &file.provenance)
        else {
            failures += 1;
            continue;
        };
        total += 1;
        let has_name = def
            .name
            .as_deref()
            .is_some_and(|n| !n.is_empty() && n != "null");
        let has_symbol = symbols.contains_key(&id);
        named += u32::from(has_name);
        symbolled += u32::from(has_symbol);
        both += u32::from(has_name && has_symbol);
        neither += u32::from(!has_name && !has_symbol);
        if !has_name && has_symbol && samples.len() < 25 {
            samples.push((id, symbols[&id]));
        }
    }
    println!(
        "loc definitions={total} decode_failures={failures} cache_named={named} symbolled={symbolled} both={both} neither={neither} symbol_table={}",
        symbols.len()
    );
    for (id, symbol) in samples {
        println!("  unnamed-in-cache but symbolled: {id} {symbol}");
    }
    Ok(())
}
