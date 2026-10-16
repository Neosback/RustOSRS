//! Run the object-relation census over every region of a cache and report how many objects
//! could be named, classified or related. Read-only.
//!
//! usage: object_semantics <OpenRune root> [max_region_x max_region_y]
use osrs_cache::{openrune::installation::Installation, source::CacheSource};
use osrs_core::{coords::RegionCoord, ids::ObjectId};
use osrs_scene::object_semantics::{
    Census, LabelSource, ObjectFacts, Placed, Role, infer, is_opaque, structure_of,
};
use osrs_world::WorldDefinitions;
use std::collections::BTreeMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let root = args
        .next()
        .ok_or("usage: object_semantics <OpenRune root> [max_x max_y]")?;
    let max_x: i32 = args.next().map_or(Ok(80), |v| v.parse())?;
    let max_y: i32 = args.next().map_or(Ok(200), |v| v.parse())?;
    let installation = Installation::read(&root)?;
    let symbols: std::collections::HashMap<u32, String> = installation
        .symbols
        .table_symbols("loc")
        .into_iter()
        .map(|(id, name)| (id, name.to_owned()))
        .collect();
    let mut definitions =
        WorldDefinitions::open(CacheSource::Dir(installation.root.join(".data/cache/LIVE")))?;
    let mut placements = Vec::new();
    let mut regions = 0;
    for x in 0..=max_x {
        for y in 0..=max_y {
            let region = RegionCoord::new(x, y);
            let Some(map) = definitions.region(region)? else {
                continue;
            };
            regions += 1;
            for loc in map.locations.locations() {
                placements.push(Placed {
                    id: loc.object_id,
                    kind: loc.loc_type,
                    orientation: loc.orientation,
                    tile: (
                        x * 64 + i32::from(loc.tile.x()),
                        y * 64 + i32::from(loc.tile.y()),
                    ),
                    plane: loc.source_plane.index().get(),
                });
            }
        }
    }
    println!("regions={regions} placements={}", placements.len());
    let mut census = Census::default();
    census.add(&placements);
    println!(
        "relations={} distinct_objects_placed={}",
        census.relations().len(),
        census.objects().count()
    );
    let mut facts = BTreeMap::new();
    for id in census.objects() {
        if let Some(def) = definitions.object(id)? {
            facts.insert(
                id,
                ObjectFacts {
                    id,
                    name: def.name.clone(),
                    symbol: symbols.get(&id.get()).cloned(),
                    structure: structure_of(&def),
                },
            );
        }
    }
    let suggestions = infer(&census, &facts);
    let mut roles: BTreeMap<String, u32> = BTreeMap::new();
    let (mut opaque, mut rescued, mut rescued_boundary, mut opaque_boundary) = (0, 0, 0, 0);
    let mut samples: Vec<(ObjectId, String, f32)> = Vec::new();
    for s in &suggestions {
        if let Some(role) = s.role {
            *roles.entry(format!("{role:?}")).or_default() += 1;
        }
        let f = &facts[&s.id];
        let given_opaque =
            f.name.as_deref().is_none_or(is_opaque) && f.symbol.as_deref().is_none_or(is_opaque);
        if given_opaque {
            opaque += 1;
            let boundary = f.structure.is_boundary();
            opaque_boundary += u32::from(boundary);
            if s.label_source == LabelSource::Inferred && s.label.is_some() {
                rescued += 1;
                rescued_boundary += u32::from(boundary);
                if samples.len() < 20 && s.confidence >= 0.6 {
                    samples.push((s.id, s.label.clone().unwrap_or_default(), s.confidence));
                }
            }
        }
    }
    println!(
        "placed objects with facts={} roles={roles:?}",
        suggestions.len()
    );
    println!(
        "opaque (no usable cache name or symbol)={opaque}, of which boundary pieces={opaque_boundary}; \
         given an inferred label={rescued} (boundary {rescued_boundary})"
    );
    for (id, label, confidence) in samples {
        println!("  {} -> {label} ({confidence:.2})", id.get());
    }
    // Names as ground truth: how often does the relation-based role agree with a name that
    // says door, window or corner?
    use osrs_scene::object_semantics::{Hint, hints, split_tokens};
    let mut agree: BTreeMap<&str, [u32; 4]> = BTreeMap::new(); // tp, fp, fn, named
    for s in &suggestions {
        let f = &facts[&s.id];
        let mut h = Vec::new();
        for text in [f.name.as_deref(), f.symbol.as_deref()]
            .into_iter()
            .flatten()
        {
            h.extend(hints(&split_tokens(text)));
        }
        let table = [
            (
                "door",
                Role::Door,
                h.contains(&Hint::Door) || h.contains(&Hint::Gate),
            ),
            ("window", Role::Window, h.contains(&Hint::Window)),
        ];
        for (label, role, named) in table {
            let row = agree.entry(label).or_default();
            let predicted = s.role == Some(role);
            row[0] += u32::from(predicted && named);
            row[1] += u32::from(predicted && !named);
            row[2] += u32::from(!predicted && named);
            row[3] += u32::from(named);
        }
    }
    for (label, [tp, fp, fnn, named]) in agree {
        println!(
            "{label}: named={named} predicted_and_named={tp} predicted_not_named={fp} named_not_predicted={fnn}"
        );
    }
    Ok(())
}
