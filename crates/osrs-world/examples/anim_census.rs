//! Census of object animations in the cache: which sequences placed objects can play and how.
//! usage: anim_census <cache-dir>
use osrs_core::ids::{ObjectId, SequenceId};
use osrs_world::WorldDefinitions;
use std::collections::BTreeMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let mut defs = WorldDefinitions::open(&args[1])?;
    for extra in args.iter().skip(2) {
        let id: u32 = extra.parse()?;
        if let Some(def) = defs.sequence(SequenceId::new(id))? {
            println!(
                "sequence {id}: frames {}, delays {:?}, step {:?}, max_loops {}, restart {}, skeletal {:?}",
                def.frame_ids.len(),
                def.frame_delays,
                def.frame_step,
                def.max_loops,
                def.restart_mode,
                def.skeletal_animation
            );
        }
    }
    let mut by_class: BTreeMap<&'static str, Vec<(u32, String, u32)>> = BTreeMap::new();
    let mut objects_with_anim = 0;
    for id in 0..80_000_u32 {
        let Some(object) = defs.object(ObjectId::new(id))? else {
            continue;
        };
        let mut sequences: Vec<u32> = object.animation.iter().map(|s| s.get()).collect();
        if let Some(morphs) = &object.morphs {
            for child in morphs_children(morphs) {
                if let Some(child) = defs.object(ObjectId::new(child))?
                    && let Some(animation) = child.animation
                {
                    sequences.push(animation.get());
                }
            }
        }
        if sequences.is_empty() {
            continue;
        }
        objects_with_anim += 1;
        let name = object.name.clone().unwrap_or_default();
        for seq in sequences {
            let Some(def) = defs.sequence(SequenceId::new(seq))? else {
                by_class
                    .entry("missing sequence")
                    .or_default()
                    .push((id, name.clone(), seq));
                continue;
            };
            let class = if def.skeletal_animation.is_some() {
                "skeletal"
            } else if def.frame_step.is_none() {
                "legacy, no frame step (plays once)"
            } else if def.frame_step == Some(0xFFFF) {
                "legacy, frame step 65535"
            } else {
                "legacy, loops"
            };
            by_class
                .entry(class)
                .or_default()
                .push((id, name.clone(), seq));
        }
    }
    println!("objects with an animation (directly or via morph): {objects_with_anim}");
    for (class, list) in &by_class {
        println!("\n{class}: {} (object, sequence) pairs", list.len());
        for (id, name, seq) in list.iter().take(12) {
            println!("  object {id} {name:?} seq {seq}");
        }
    }
    Ok(())
}

fn morphs_children(morphs: &osrs_core::definitions::ObjectMorphs) -> Vec<u32> {
    morphs
        .transforms
        .iter()
        .copied()
        .chain([morphs.fallback])
        .flatten()
        .map(|id| id.get())
        .collect()
}
