//! Training a building model from saved buildings, start to finish, with progress reports.
//!
//! The steps are the ones the Train screen shows: look at the buildings, learn their style,
//! make practice buildings to compare with real ones, teach the critic the difference, then try
//! the result out on briefs it has not seen.

use crate::model::{MODEL_FORMAT, MODEL_VERSION, fingerprint};
use crate::{
    Brief, Model, ModelMetrics, ModelName, Structure, TrainedOn, build_with_model, features_of,
    generate_named, learn_style, prefab_buildings, train_critic_partitioned,
};
use osrs_edit::{KitFile, Prefab};
use osrs_scene::building_census::{FEATURES, ObjectTable, judge};
use serde::Deserialize;

/// Numbers describing real buildings of the game, one per row (width, height, split, then the
/// building numbers in [`osrs_scene::building_census::FEATURE_NAMES`] order). The split is 0
/// for train, 1 for validation and 2 for test, the same geographic split the dataset uses.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Corpus {
    pub names: Vec<String>,
    pub rows: Vec<Vec<f32>>,
}

impl Corpus {
    /// The corpus that ships with the editor.
    pub fn shipped() -> Self {
        serde_json::from_str(include_str!("../data/real-buildings.json")).unwrap_or_default()
    }

    fn usable(&self, split: u8) -> impl Iterator<Item = (u16, u16, [f32; FEATURES])> + '_ {
        self.rows.iter().filter_map(move |row| {
            // Real roofed buildings only (the game also has fenced yards and ruins), from the
            // requested source partition only.
            if row.len() != 3 + FEATURES
                || row[2] != f32::from(split)
                || row[3] < 12.0
                || row[3 + 7] < 0.3
            {
                return None;
            }
            let (w, h) = (row[0] as u16, row[1] as u16);
            let mut features = [0.0; FEATURES];
            features.copy_from_slice(&row[3..]);
            Some((w, h, features))
        })
    }
}

pub struct TrainRequest<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub created: &'a str,
    /// The saved buildings to learn from: `(name, prefab)`.
    pub prefabs: &'a [(String, Prefab)],
    /// The names of the objects they use (see `kit_objects` for generated ones).
    pub objects: &'a ObjectTable,
    /// The kit the style is learned on top of: what the prefabs do not show stays as it is.
    pub base_kit: &'a KitFile,
    pub corpus: &'a Corpus,
    pub seed: u64,
    /// Asked between steps: true stops the training with an error.
    pub cancelled: &'a dyn Fn() -> bool,
}

pub enum Progress {
    Step { percent: u8, message: String },
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
}

const KINDS: [&str; 6] = ["house", "shop", "tavern", "bank", "church", "warehouse"];

fn brief(kind: &str, width: u16, height: u16, seed: u64) -> Brief {
    Brief {
        sketch: None,
        kind: kind.to_owned(),
        width: width.clamp(6, 24),
        height: height.clamp(6, 24),
        floors: 1,
        seed,
        variation: 0,
        rooms: None,
    }
}

/// Every object id the kit uses, with the name the saved buildings gave it (or the kit's own).
fn kit_names(kit: &KitFile, objects: &ObjectTable) -> std::collections::BTreeMap<u32, ModelName> {
    let mut names = std::collections::BTreeMap::new();
    for (id, info) in crate::kit_objects(kit) {
        let real = objects
            .get(&id)
            .filter(|r| !r.name.is_empty() && r.name != "null");
        let chosen = real.filter(|_| info.name == "furniture" || info.name == "wall decoration");
        let info = chosen.unwrap_or(&info);
        names.insert(
            id.get(),
            ModelName {
                name: info.name.clone(),
                symbol: info.symbol.clone(),
                actions: info.actions.clone(),
                size: [info.size.0, info.size.1],
            },
        );
    }
    names
}

/// Ask for the room count a real building of this size would have.
fn structured(mut brief: Brief, structure: &Structure, roll: u64) -> Brief {
    let area = u32::from(brief.width.saturating_sub(2)) * u32::from(brief.height.saturating_sub(2));
    brief.rooms = structure.pick(area, roll >> 20);
    brief
}

fn check(request: &TrainRequest<'_>) -> Result<(), String> {
    if (request.cancelled)() {
        Err("Training was stopped.".to_owned())
    } else {
        Ok(())
    }
}

fn report(on: &mut dyn FnMut(Progress), percent: u8, message: &str) {
    on(Progress::Step {
        percent,
        message: message.to_owned(),
    });
}

/// Train a model. `elapsed` gives seconds since the start (the library never reads a clock).
pub fn train_model(
    request: &TrainRequest<'_>,
    elapsed: &dyn Fn() -> f32,
    on: &mut dyn FnMut(Progress),
) -> Result<Model, String> {
    if request.prefabs.is_empty() {
        return Err("pick at least one saved building to learn from".into());
    }

    report(on, 5, "Looking at your buildings");
    let prefabs: Vec<Prefab> = request.prefabs.iter().map(|(_, p)| p.clone()).collect();
    let buildings = prefab_buildings(&prefabs, request.objects);

    report(
        on,
        15,
        "Learning their style: walls, doors, floors and furniture",
    );
    let (kit, style) = learn_style(request.base_kit, request.name, &buildings, request.objects)?;

    report(
        on,
        25,
        "Making practice buildings to compare with real ones",
    );
    // The names of every object the learned kit uses.
    let names = kit_names(&kit, request.objects);
    let table = ObjectTable::from_iter(names.iter().map(|(id, n)| {
        (
            osrs_core::ids::ObjectId::new(*id),
            osrs_scene::building_census::ObjectInfo {
                name: n.name.clone(),
                symbol: n.symbol.clone(),
                actions: n.actions.clone(),
                size: (n.size[0], n.size[1]),
            },
        )
    }));
    let mut rng = Rng(request.seed ^ 0x5bd1_e995_a3c2_f1d7);
    let real_rows: Vec<(u16, u16, [f32; FEATURES])> = request
        .corpus
        .usable(0)
        .filter(|(w, h, _)| (6..=24).contains(w) && (6..=24).contains(h))
        .collect();
    // Up to 500 real buildings, spread through the corpus.
    let stride = (real_rows.len() / 500).max(1);
    let chosen: Vec<_> = real_rows.iter().step_by(stride).take(500).collect();
    let mut real: Vec<[f32; FEATURES]> = chosen.iter().map(|(_, _, f)| *f).collect();
    // Your own buildings count as real too, a few times over so a small library is not drowned.
    for building in &buildings {
        let features = judge(building).features;
        for _ in 0..4 {
            real.push(features);
        }
    }
    // What real buildings of each size have: how many rooms.
    let structure = Structure::from_buildings(
        request
            .corpus
            .usable(0)
            .map(|(_, _, f)| (f[0] as u32, f[1] as u32)),
    );
    let mut fake: Vec<[f32; FEATURES]> = Vec::new();
    let mut tried = 0_usize;
    for (index, (w, h, _)) in chosen.iter().enumerate() {
        if index % 25 == 0 {
            check(request)?;
        }
        tried += 1;
        let kind = KINDS[index % KINDS.len()];
        if let Ok(made) = generate_named(
            &kit,
            &structured(
                brief(kind, *w, *h, rng.next() % 1_000_000),
                &structure,
                rng.next(),
            ),
            &table,
        ) && made.verdict.usable
        {
            fake.push(features_of(&made)?);
        }
        if index % 50 == 0 {
            report(
                on,
                25 + (index * 25 / chosen.len().max(1)) as u8,
                "Making practice buildings to compare with real ones",
            );
        }
    }
    // Real buildings may be too few or too odd to match; practice buildings of your own sizes too.
    for index in 0..60_usize {
        let (w, h) = (8 + (rng.next() % 14) as u16, 8 + (rng.next() % 12) as u16);
        if let Ok(made) = generate_named(
            &kit,
            &structured(
                brief(KINDS[index % KINDS.len()], w, h, rng.next() % 1_000_000),
                &structure,
                rng.next(),
            ),
            &table,
        ) && made.verdict.usable
        {
            fake.push(features_of(&made)?);
        }
    }
    if fake.len() < 5 {
        return Err(format!(
            "only {} of {tried} practice buildings could be built in this style; \
             try a different starting kit or a building with clearer walls",
            fake.len()
        ));
    }

    check(request)?;
    report(on, 55, "Teaching the critic what real buildings look like");
    let mut real_validation = Vec::new();
    let mut fake_validation = Vec::new();
    for (index, (w, h, f)) in request.corpus.usable(1).take(100).enumerate() {
        check(request)?;
        if !(6..=24).contains(&w) || !(6..=24).contains(&h) {
            continue;
        }
        if let Ok(made) = generate_named(
            &kit,
            &structured(
                brief(KINDS[index % KINDS.len()], w, h, rng.next()),
                &structure,
                rng.next(),
            ),
            &table,
        ) && made.verdict.usable
        {
            real_validation.push(f);
            fake_validation.push(features_of(&made)?);
        }
    }
    let (critic, critic_metrics) = train_critic_partitioned(
        &real,
        &fake,
        &real_validation,
        &fake_validation,
        300,
        |fraction, _| {
            on(Progress::Step {
                percent: 55 + (fraction * 30.0) as u8,
                message: "Teaching the critic what real buildings look like".to_owned(),
            });
        },
    )?;

    // A model without its checksum yet, enough to try building with.
    let trained_on: Vec<TrainedOn> = request
        .prefabs
        .iter()
        .map(|(name, prefab)| TrainedOn {
            name: name.clone(),
            hash: fingerprint(&serde_json::to_vec(prefab).unwrap_or_default()),
        })
        .collect();
    let mut model = Model {
        format: MODEL_FORMAT.to_owned(),
        version: MODEL_VERSION,
        id: request.id.to_owned(),
        name: request.name.to_owned(),
        created: request.created.to_owned(),
        trained_on,
        kit,
        critic,
        structure,
        names,
        metrics: ModelMetrics {
            buildings_learned_from: style.buildings,
            rooms: style.rooms,
            furniture_rules: style.furniture_rules,
            floor_rules: style.floor_rules,
            usable_rate: 0.0,
            realism_single: 0.0,
            realism_best_of_16: 0.0,
            critic: critic_metrics,
            corpus_real: real.len(),
            corpus_generated: fake.len(),
            seconds: 0.0,
        },
        summary: Vec::new(),
        checksum: String::new(),
    };

    check(request)?;
    report(on, 88, "Trying it out on briefs it has not seen");
    let (mut built, mut usable) = (0_u32, 0_u32);
    let (mut single, mut best) = (0.0_f32, 0.0_f32);
    for index in 0..32_u64 {
        let (w, h) = (8 + (index % 12) as u16, 8 + ((index * 3) % 10) as u16);
        let trial = brief(
            KINDS[index as usize % KINDS.len()],
            w,
            h,
            request.seed.wrapping_add(10_000 + index * 101),
        );
        built += 1;
        if let Ok(one) = build_with_model(&model, &trial, 1) {
            usable += 1;
            single += one.realism;
            if let Ok(many) = build_with_model(&model, &trial, 16) {
                best += many.realism;
            }
        }
    }
    let ok = usable.max(1) as f32;
    model.metrics.usable_rate = usable as f32 / built.max(1) as f32;
    model.metrics.realism_single = single / ok;
    model.metrics.realism_best_of_16 = best / ok;
    model.metrics.seconds = elapsed();

    let metrics = &model.metrics;
    let mut summary = vec![
        format!(
            "Learned from {} building(s) with {} room(s): {}.",
            metrics.buildings_learned_from,
            metrics.rooms,
            if style.learned.is_empty() {
                "nothing new".to_owned()
            } else {
                style.learned.join(", ")
            }
        ),
        format!(
            "Made {} furniture rule(s) and {} floor rule(s).",
            metrics.furniture_rules, metrics.floor_rules
        ),
    ];
    if !style.kept.is_empty() {
        summary.push(format!(
            "Kept the starting kit's {} (your buildings did not show them).",
            style.kept.join(", ")
        ));
    }
    if !style.unnamed.is_empty() {
        summary.push(format!(
            "{} kind(s) of object had no name. Naming them makes the furniture rules more reliable.",
            style.unnamed.len()
        ));
    }
    summary.push(format!(
        "Tried {built} new briefs: {usable} built a usable building. The critic thinks they look \
         {:.0}% real on the first try and {:.0}% real keeping the best of 16.",
        metrics.realism_single, metrics.realism_best_of_16
    ));
    if request.prefabs.len() < 3 {
        summary.push(
            "Only a few buildings were used, so this model knows their style but little about \
             variety. More buildings make it more general."
                .to_owned(),
        );
    }
    model.summary = summary;
    report(on, 97, "Saving");
    model.seal()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Critic, Snapshot, census_buildings, generate, kit_objects};

    const BRICK: &str =
        include_str!("../../osrs-editor/src/app/prefab_runtime/kits/lumbridge-brick.kit.json");
    const TIMBER: &str =
        include_str!("../../osrs-editor/src/app/prefab_runtime/kits/timber-town.kit.json");

    fn kit(text: &str) -> KitFile {
        serde_json::from_str(text).unwrap_or_else(|e| panic!("kit: {e}"))
    }

    /// A stand-in for the real corpus: buildings with more furniture and windows than the plain
    /// generated ones, spread over plausible sizes.
    fn corpus() -> Corpus {
        let rows = (0..120_u32)
            .map(|i| {
                let mut row = vec![8.0 + (i % 12) as f32, 8.0 + (i % 9) as f32, 0.0];
                let mut features = [0.0_f32; FEATURES];
                features[0] = 60.0 + (i % 40) as f32;
                features[1] = 2.0 + (i % 3) as f32;
                features[4] = 0.25 + (i % 5) as f32 * 0.01;
                features[7] = 0.9;
                features[8] = 0.35 + (i % 4) as f32 * 0.01;
                row.extend(features);
                row
            })
            .collect();
        Corpus {
            names: Vec::new(),
            rows,
        }
    }

    /// A saved prefab built from a generated brick building, as the editor saves one.
    fn saved_prefab() -> (String, Prefab, ObjectTable) {
        let brick = kit(BRICK);
        let objects = kit_objects(&brick);
        let made = generate(&brick, &brief("tavern", 16, 12, 3)).unwrap_or_else(|e| panic!("{e}"));
        let mut prefab = Prefab {
            format: osrs_edit::PREFAB_FORMAT.to_owned(),
            version: osrs_edit::PREFAB_VERSION,
            name: "tavern".to_owned(),
            tags: vec![],
            notes: String::new(),
            source: osrs_edit::PrefabSource::Authored,
            width: made.width,
            height: made.height,
            planes: made.planes,
            tiles: made.snapshot.tiles.clone(),
            heights: made.snapshot.heights.clone(),
            objects: made.snapshot.objects.clone(),
            building: None,
        };
        prefab.planes = made.planes;
        // The census sees the same building the lab judged.
        let found = census_buildings(
            &Snapshot {
                tiles: prefab.tiles.clone(),
                heights: prefab.heights.clone(),
                objects: prefab.objects.clone(),
            },
            prefab.width,
            prefab.height,
            &objects,
        );
        assert!(!found.is_empty());
        ("tavern".to_owned(), prefab, objects)
    }

    #[test]
    fn training_makes_a_model_that_builds_a_new_building_without_an_example() {
        let (name, prefab, objects) = saved_prefab();
        let prefabs = vec![(name, prefab)];
        let base = kit(TIMBER);
        let corpus = corpus();
        let mut steps = Vec::new();
        let model = train_model(
            &TrainRequest {
                id: "brick-1",
                name: "brick style",
                created: "2026-10-18",
                prefabs: &prefabs,
                objects: &objects,
                base_kit: &base,
                corpus: &corpus,
                seed: 7,
                cancelled: &|| false,
            },
            &|| 1.5,
            &mut |Progress::Step { percent, message }| steps.push((percent, message)),
        )
        .unwrap_or_else(|e| panic!("{e}"));
        assert!(steps.first().is_some_and(|(p, _)| *p <= 10));
        assert!(
            steps.windows(2).all(|w| w[0].0 <= w[1].0),
            "progress never goes back"
        );
        assert!(steps.last().is_some_and(|(p, _)| *p >= 95));
        assert_eq!(model.kit.exterior_wall.id, kit(BRICK).exterior_wall.id);
        assert!(model.metrics.usable_rate > 0.7, "{:?}", model.metrics);
        assert!(model.summary.len() >= 3);
        model.validate().unwrap_or_else(|e| panic!("{e}"));

        // It builds from a brief it was never shown, and the same brief gives the same building.
        let ask = Brief {
            sketch: None,
            kind: "house".into(),
            width: 12,
            height: 10,
            floors: 1,
            seed: 40,
            variation: 0,
            rooms: None,
        };
        let a = build_with_model(&model, &ask, 8).unwrap_or_else(|e| panic!("{e}"));
        let b = build_with_model(&model, &ask, 8).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(a, b);
        assert!(a.generated.verdict.usable);
        assert!(a.tried.windows(2).all(|w| w[0].realism >= w[1].realism));
        // Keeping the best of many is never worse than the first try.
        let first = build_with_model(&model, &ask, 1).unwrap_or_else(|e| panic!("{e}"));
        assert!(a.realism >= first.realism);

        // The saved file round trips and notices tampering.
        let text = model.to_json().unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            Model::from_json(&text).unwrap_or_else(|e| panic!("{e}")),
            model
        );
        let tampered = text.replace("brick style", "something else");
        assert!(Model::from_json(&tampered).is_err());
    }

    #[test]
    fn training_with_nothing_picked_is_an_explicit_error() {
        let base = kit(BRICK);
        let corpus = corpus();
        let result = train_model(
            &TrainRequest {
                id: "x",
                name: "x",
                created: "",
                prefabs: &[],
                objects: &ObjectTable::new(),
                base_kit: &base,
                corpus: &corpus,
                seed: 1,
                cancelled: &|| false,
            },
            &|| 0.0,
            &mut |_| {},
        );
        assert!(result.is_err());
    }

    #[test]
    fn an_unusable_critic_shape_is_refused() {
        let critic = Critic {
            features: vec!["nope".into()],
            weights: vec![1],
            bias: 0,
        };
        assert!(!critic.is_compatible());
    }
}

#[cfg(test)]
mod cancel_tests {
    use super::*;

    #[test]
    fn a_stopped_training_ends_with_an_error_and_no_model() {
        let base: KitFile = serde_json::from_str(include_str!(
            "../../osrs-editor/src/app/prefab_runtime/kits/lumbridge-brick.kit.json"
        ))
        .unwrap_or_else(|e| panic!("{e}"));
        let prefab = Prefab {
            format: osrs_edit::PREFAB_FORMAT.to_owned(),
            version: osrs_edit::PREFAB_VERSION,
            name: "x".into(),
            tags: vec![],
            notes: String::new(),
            source: osrs_edit::PrefabSource::Authored,
            width: 4,
            height: 4,
            planes: 1,
            tiles: vec![],
            heights: vec![],
            objects: vec![],
            building: None,
        };
        let prefabs = vec![("x".to_owned(), prefab)];
        let result = train_model(
            &TrainRequest {
                id: "x",
                name: "x",
                created: "",
                prefabs: &prefabs,
                objects: &ObjectTable::new(),
                base_kit: &base,
                corpus: &Corpus::default(),
                seed: 1,
                cancelled: &|| true,
            },
            &|| 0.0,
            &mut |_| {},
        );
        assert!(result.is_err_and(|e| e.contains("stopped") || e.contains("no building")));
    }
}
