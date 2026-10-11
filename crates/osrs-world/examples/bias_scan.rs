//! Count object models carrying face bias data. usage: bias_scan <cache>
use osrs_core::definitions::{LocType, ObjectModels};
use osrs_core::ids::ObjectId;
use osrs_world::WorldDefinitions;
use std::collections::HashSet;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    let mut defs = WorldDefinitions::open(&a[1])?;
    let (mut models, mut with_bias, mut nonzero) = (0, 0, 0);
    let mut seen = HashSet::new();
    let mut examples = Vec::new();
    for id in 0..30000u32 {
        let Some(def) = defs.object(ObjectId::new(id))? else {
            continue;
        };
        let Some(ObjectModels::Typed(entries)) = &def.models else {
            continue;
        };
        for entry in entries.clone() {
            if !seen.insert(entry.model_id.get()) {
                continue;
            }
            let Some(model) = defs.resolve_model(&def, entry.loc_type, 0)? else {
                continue;
            };
            models += 1;
            if let Some(b) = model.face_biases() {
                with_bias += 1;
                if b.iter().any(|v| *v != 0) {
                    nonzero += 1;
                    if examples.len() < 8 {
                        examples.push((id, entry.loc_type.get(), entry.model_id.get()));
                    }
                }
            }
        }
        let _ = LocType::new(0);
    }
    println!(
        "typed object models {models}: with bias array {with_bias}, non-zero bias {nonzero}; examples (object, type, model) {examples:?}"
    );
    Ok(())
}
