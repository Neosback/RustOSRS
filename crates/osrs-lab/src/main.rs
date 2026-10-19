//! JSON-lines server. Each environment retains a private target and exposes only tool actions.
use osrs_lab::{
    Action, Env, LabTask, ObservationMode, generated_paths, generated_tavern, prefab_paths_task,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{self, BufRead, Write},
};

fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case", deny_unknown_fields)]
enum Command {
    Reset {
        task: String,
        #[serde(default)]
        seed: u64,
        #[serde(default)]
        mode: ObservationMode,
    },
    Step {
        action: Action,
        #[serde(default = "default_true")]
        observe: bool,
    },
    Observe,
    Legal,
    Undo,
    Replay {
        actions: Vec<Action>,
    },
    Learn {
        grids: Vec<osrs_edit::LearningGrid>,
        style: String,
        role: String,
        source: String,
    },
}

#[derive(Deserialize)]
struct Request {
    #[serde(default)]
    env: u32,
    #[serde(flatten)]
    command: Command,
}

struct Running {
    env: Env,
    mode: ObservationMode,
    task: LabTask,
    seed: u64,
}

fn run(request: Request, envs: &mut BTreeMap<u32, Running>) -> Result<Value, String> {
    let id = request.env;
    match request.command {
        Command::Reset { task, seed, mode } => {
            let task: LabTask =
                serde_json::from_str(&std::fs::read_to_string(task).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            let env = Env::reset(task.clone(), seed).map_err(|e| e.to_string())?;
            let observation = env.observation(mode);
            let breakdown = env.score();
            envs.insert(
                id,
                Running {
                    env,
                    mode,
                    task,
                    seed,
                },
            );
            Ok(json!({
                "observation": observation,
                "score": breakdown.total,
                "breakdown": breakdown,
            }))
        }
        Command::Learn {
            grids,
            style,
            role,
            source,
        } => {
            let catalog = osrs_edit::learn_catalog_with_rotations(&grids, style, role, source)
                .map_err(|e| e.to_string())?;
            Ok(json!({"catalog": catalog}))
        }
        command => {
            let running = envs.get_mut(&id).ok_or("reset this environment first")?;
            match command {
                Command::Step { action, observe } => {
                    let result = running.env.step(action).map_err(|e| e.to_string())?;
                    if observe {
                        Ok(json!({
                            "result": result,
                            "observation": running.env.observation(running.mode),
                        }))
                    } else {
                        Ok(json!({ "result": result }))
                    }
                }
                Command::Undo => Ok(
                    json!({"result": running.env.step(Action::Undo).map_err(|e| e.to_string())?}),
                ),
                Command::Observe => {
                    let breakdown = running.env.score();
                    Ok(json!({
                        "observation": running.env.observation(running.mode),
                        "score": breakdown.total,
                        "breakdown": breakdown,
                    }))
                }
                Command::Legal => Ok(json!({"legal": running.env.legal_mask()})),
                Command::Replay { actions } => {
                    let replay = Env::replay(running.task.clone(), running.seed, &actions)
                        .map_err(|e| e.to_string())?;
                    let observation = replay.observation(running.mode);
                    running.env = replay;
                    let breakdown = running.env.score();
                    Ok(json!({
                        "observation": observation,
                        "score": breakdown.total,
                        "breakdown": breakdown,
                    }))
                }
                _ => Err("unsupported command".into()),
            }
        }
    }
}

mod cli_paths;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|s| s == "task-copy") {
        let path = args.get(1).ok_or("task-copy PREFAB_JSON")?;
        let prefab = serde_json::from_str(&std::fs::read_to_string(path)?)?;
        println!(
            "{}",
            serde_json::to_string(&osrs_lab::reconstruction_task(&prefab, &|_, _| None)?)?
        );
        return Ok(());
    }
    if args.first().is_some_and(|s| s == "task-relocate") {
        let path = args.get(1).ok_or("task-relocate PREFAB_JSON [SEED]")?;
        let prefab = serde_json::from_str(&std::fs::read_to_string(path)?)?;
        let seed = args.get(2).map_or(Ok(1), |s| s.parse::<u64>())?;
        println!(
            "{}",
            serde_json::to_string(&osrs_lab::relocation_task(&prefab, seed)?)?
        );
        return Ok(());
    }
    if args.first().is_some_and(|s| s == "task-tavern") {
        let path = args.get(1).ok_or("task-tavern KIT_JSON [SEED]")?;
        let seed = args.get(2).map_or(Ok(1), |s| s.parse::<u64>())?;
        let kit = serde_json::from_str(&std::fs::read_to_string(path)?)?;
        println!("{}", serde_json::to_string(&generated_tavern(&kit, seed)?)?);
        return Ok(());
    }
    if args.first().is_some_and(|s| s == "task-paths") {
        match args.get(1) {
            None => {
                println!("{}", serde_json::to_string(&generated_paths(1)?)?);
            }
            Some(arg) => {
                if let Ok(seed) = arg.parse::<u64>() {
                    println!("{}", serde_json::to_string(&generated_paths(seed)?)?);
                } else {
                    let prefab = serde_json::from_str(&std::fs::read_to_string(arg)?)?;
                    let seed = args.get(2).map_or(Ok(1), |s| s.parse::<u64>())?;
                    println!(
                        "{}",
                        serde_json::to_string(&prefab_paths_task(&prefab, seed)?)?
                    );
                }
            }
        }
        return Ok(());
    }
    if args.first().is_some_and(|s| s == "generate") {
        // generate KIT_JSON BRIEF_JSON | generate KIT_JSON KIND WIDTH HEIGHT [FLOORS] [SEED] [VARIATION]
        let usage =
            "generate KIT_JSON (BRIEF_JSON | KIND WIDTH HEIGHT [FLOORS] [SEED] [VARIATION])";
        let kit = serde_json::from_str(&std::fs::read_to_string(args.get(1).ok_or(usage)?)?)?;
        let second = args.get(2).ok_or(usage)?;
        let brief: osrs_lab::Brief = if second.ends_with(".json") {
            serde_json::from_str(&std::fs::read_to_string(second)?)?
        } else {
            let number =
                |i: usize, default: &str| args.get(i).map_or(default, String::as_str).to_owned();
            osrs_lab::Brief {
                kind: second.clone(),
                width: number(3, "12").parse()?,
                height: number(4, "10").parse()?,
                floors: number(5, "1").parse()?,
                seed: number(6, "0").parse()?,
                variation: number(7, "0").parse()?,
                rooms: None,
            }
        };
        println!(
            "{}",
            serde_json::to_string(&osrs_lab::generate(&kit, &brief)?)?
        );
        return Ok(());
    }
    if args.first().is_some_and(|s| s == "judge") {
        // judge KIT_JSON GENERATED_JSON: judge a generated building (for example one a model
        // changed) again, with the kit's names.
        let usage = "judge KIT_JSON GENERATED_JSON";
        let kit = serde_json::from_str(&std::fs::read_to_string(args.get(1).ok_or(usage)?)?)?;
        let generated: osrs_lab::Generated =
            serde_json::from_str(&std::fs::read_to_string(args.get(2).ok_or(usage)?)?)?;
        let verdict = osrs_lab::judge_snapshot(
            &generated.snapshot,
            generated.width,
            generated.height,
            &osrs_lab::kit_objects(&kit),
        );
        println!("{}", serde_json::to_string(&verdict)?);
        return Ok(());
    }
    if args.first().is_some_and(|s| s == "paths-stats") {
        return cli_paths::stats_command(&args[1..]);
    }
    if args.first().is_some_and(|s| s == "paths-show") {
        return cli_paths::show_command(&args[1..]);
    }
    if args.first().is_some_and(|s| s == "paths-census") {
        return cli_paths::census_command(&args[1..]);
    }
    if args.first().is_some_and(|s| s == "train-paths") {
        return cli_paths::train_paths_command(&args[1..]);
    }
    if args.first().is_some_and(|s| s == "train-model") {
        return train_model_command(&args[1..]);
    }
    if args.first().is_some_and(|s| s == "build-model") {
        // build-model MODEL_JSON KIND WIDTH HEIGHT [FLOORS] [SEED] [TRIES]
        let usage = "build-model MODEL_JSON KIND WIDTH HEIGHT [FLOORS] [SEED] [TRIES]";
        let model =
            osrs_lab::Model::from_json(&std::fs::read_to_string(args.get(1).ok_or(usage)?)?)?;
        let number =
            |i: usize, default: &str| args.get(i).map_or(default, String::as_str).to_owned();
        let brief = osrs_lab::Brief {
            kind: args.get(2).ok_or(usage)?.clone(),
            width: number(3, "12").parse()?,
            height: number(4, "10").parse()?,
            floors: number(5, "1").parse()?,
            seed: number(6, "0").parse()?,
            variation: 0,
            rooms: None,
        };
        let built = osrs_lab::build_with_model(&model, &brief, number(7, "8").parse()?)?;
        println!("{}", serde_json::to_string(&built)?);
        return Ok(());
    }
    if args.first().is_some_and(|s| s != "serve") {
        return Err("use serve, train-model, build-model MODEL_JSON KIND WIDTH HEIGHT, generate KIT_JSON BRIEF, judge KIT_JSON GENERATED_JSON, task-copy PREFAB_JSON, task-tavern KIT_JSON [SEED], task-relocate PREFAB_JSON [SEED], or task-paths [SEED|PREFAB_JSON] [SEED]".into());
    }
    let mut envs = BTreeMap::new();
    let mut stdout = io::stdout().lock();
    for line in io::stdin().lock().lines() {
        let line = line?;
        let response = (|| -> Result<Value, String> {
            let value: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
            if let Some(batch) = value.as_array() {
                Ok(Value::Array(
                    batch
                        .iter()
                        .map(|v| {
                            match serde_json::from_value::<Request>(v.clone())
                                .map_err(|e| e.to_string())
                                .and_then(|r| run(r, &mut envs))
                            {
                                Ok(v) => json!({"ok": true, "data": v}),
                                Err(e) => json!({"ok": false, "error": e}),
                            }
                        })
                        .collect(),
                ))
            } else {
                run(
                    serde_json::from_value(value).map_err(|e| e.to_string())?,
                    &mut envs,
                )
            }
        })();
        let response = match response {
            Ok(data) => json!({"ok": true, "data": data}),
            Err(error) => json!({"ok": false, "error": error}),
        };
        writeln!(stdout, "{response}")?;
        stdout.flush()?;
    }
    Ok(())
}

/// One value after a flag, or the values until the next flag.
fn flag_values<'a>(args: &'a [String], flag: &str) -> Vec<&'a str> {
    args.iter()
        .position(|a| a == flag)
        .map(|at| {
            args[at + 1..]
                .iter()
                .take_while(|a| !a.starts_with("--"))
                .map(String::as_str)
                .collect()
        })
        .unwrap_or_default()
}

/// `train-model --name N --id ID --created TEXT --prefabs FILE... --objects TABLE.json
/// --base-kit KIT.json --out MODEL.json [--corpus FILE] [--seed N]`
///
/// Prints one JSON line per progress step (`{"progress": 40, "message": "..."}`), then
/// `{"done": {...}}` or `{"error": "..."}`.
fn train_model_command(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    use osrs_scene::building_census::{ObjectInfo, ObjectTable};
    let one = |flag: &str| flag_values(args, flag).first().map(|s| (*s).to_owned());
    let fail = |message: String| {
        println!("{}", json!({"error": message}));
        Ok::<(), Box<dyn std::error::Error>>(())
    };
    let (Some(name), Some(out), Some(base)) = (one("--name"), one("--out"), one("--base-kit"))
    else {
        return fail("train-model needs --name, --base-kit and --out".into());
    };
    let id = one("--id").unwrap_or_else(|| "model".into());
    let created = one("--created").unwrap_or_default();
    let seed: u64 = one("--seed").map_or(Ok(1), |s| s.parse())?;
    let mut prefabs = Vec::new();
    for file in flag_values(args, "--prefabs") {
        let prefab: osrs_edit::Prefab = serde_json::from_str(&std::fs::read_to_string(file)?)?;
        prefabs.push((prefab.name.clone(), prefab));
    }
    let mut objects = ObjectTable::new();
    if let Some(path) = one("--objects") {
        let table: std::collections::BTreeMap<String, Value> =
            serde_json::from_str(&std::fs::read_to_string(path)?)?;
        for (id, entry) in table {
            let Ok(id) = id.parse::<u32>() else { continue };
            let text = |key: &str| entry[key].as_str().unwrap_or_default().to_lowercase();
            objects.insert(
                osrs_core::ids::ObjectId::new(id),
                ObjectInfo {
                    name: text("name"),
                    symbol: text("symbol"),
                    actions: entry["actions"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|a| a.as_str().map(str::to_lowercase))
                        .collect(),
                    size: (
                        entry["size"][0].as_u64().unwrap_or(1).clamp(1, 255) as u8,
                        entry["size"][1].as_u64().unwrap_or(1).clamp(1, 255) as u8,
                    ),
                },
            );
        }
    }
    let base_kit = serde_json::from_str(&std::fs::read_to_string(base)?)?;
    let corpus = match one("--corpus") {
        Some(path) => serde_json::from_str(&std::fs::read_to_string(path)?)?,
        None => osrs_lab::Corpus::shipped(),
    };
    let started = std::time::Instant::now();
    let result = osrs_lab::train_model(
        &osrs_lab::TrainRequest {
            id: &id,
            name: &name,
            created: &created,
            prefabs: &prefabs,
            objects: &objects,
            base_kit: &base_kit,
            corpus: &corpus,
            seed,
            cancelled: &|| false,
        },
        &|| started.elapsed().as_secs_f32(),
        &mut |osrs_lab::Progress::Step { percent, message }| {
            println!("{}", json!({"progress": percent, "message": message}));
            let _ = io::stdout().flush();
        },
    );
    match result {
        Ok(model) => {
            std::fs::write(&out, model.to_json()?)?;
            println!(
                "{}",
                json!({"done": {"model": out, "summary": model.summary, "metrics": model.metrics}})
            );
            Ok(())
        }
        Err(error) => fail(error),
    }
}
