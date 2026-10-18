//! JSON-lines server. Each environment retains a private target and exposes only tool actions.
use osrs_lab::{Action, Env, LabTask, ObservationMode, generated_tavern};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{self, BufRead, Write},
};
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
            let score = env.score().total;
            envs.insert(
                id,
                Running {
                    env,
                    mode,
                    task,
                    seed,
                },
            );
            Ok(json!({"observation":observation,"score":score}))
        }
        Command::Learn {
            grids,
            style,
            role,
            source,
        } => {
            let catalog =
                osrs_edit::learn_catalog(&grids, style, role, source).map_err(|e| e.to_string())?;
            Ok(json!({"catalog":catalog}))
        }
        command => {
            let running = envs.get_mut(&id).ok_or("reset this environment first")?;
            match command {
                Command::Step { action } => Ok(
                    json!({"result":running.env.step(action).map_err(|e|e.to_string())?,"observation":running.env.observation(running.mode)}),
                ),
                Command::Undo => {
                    Ok(json!({"result":running.env.step(Action::Undo).map_err(|e|e.to_string())?}))
                }
                Command::Observe => Ok(
                    json!({"observation":running.env.observation(running.mode),"score":running.env.score().total}),
                ),
                Command::Legal => Ok(json!({"legal":running.env.legal_mask()})),
                Command::Replay { actions } => {
                    let replay = Env::replay(running.task.clone(), running.seed, &actions)
                        .map_err(|e| e.to_string())?;
                    let observation = replay.observation(running.mode);
                    running.env = replay;
                    Ok(json!({"observation":observation,"score":running.env.score().total}))
                }
                _ => Err("unsupported command".into()),
            }
        }
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
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
    if args.first().is_some_and(|s| s != "serve") {
        return Err("use serve or task-tavern KIT_JSON [SEED]".into());
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
                                Ok(v) => json!({"ok":true,"data":v}),
                                Err(e) => json!({"ok":false,"error":e}),
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
            Ok(data) => json!({"ok":true,"data":data}),
            Err(error) => json!({"ok":false,"error":error}),
        };
        writeln!(stdout, "{response}")?;
        stdout.flush()?;
    }
    Ok(())
}
