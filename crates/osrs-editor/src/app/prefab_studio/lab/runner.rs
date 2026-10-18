//! Native demonstration jobs reuse the existing Python tool policy and headless Lab server.
//! Jobs consume immutable tasks. Only replayed actions may change the Lab environment.
use osrs_lab::{Action, Env, LabTask};
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub(super) struct Demonstration {
    pub(super) actions: Vec<Action>,
    pub(super) score: f64,
    pub(super) directory: PathBuf,
}
enum Event {
    Phase(&'static str),
    Done(Result<Demonstration, String>),
}
pub(super) struct Job {
    events: Receiver<Event>,
    cancel: Arc<AtomicBool>,
    pub(super) phase: &'static str,
}
impl Job {
    pub(super) fn start(task: LabTask, seed: u64) -> Result<Self, String> {
        let parent = crate::settings::path()
            .and_then(|p| p.parent().map(|p| p.join("lab-runs")))
            .ok_or("No local Lab output directory is available")?;
        let (send, events) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        std::thread::Builder::new()
            .name("lab-demonstration".into())
            .spawn(move || {
                let result = run(task, seed, &parent, &stop, &send);
                let _ = send.send(Event::Done(result));
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            events,
            cancel,
            phase: "Preparing task",
        })
    }
    pub(super) fn poll(&mut self) -> Option<Result<Demonstration, String>> {
        loop {
            match self.events.try_recv() {
                Ok(Event::Phase(phase)) => self.phase = phase,
                Ok(Event::Done(result)) => return Some(result),
                Err(mpsc::TryRecvError::Empty) => return None,
                Err(_) => {
                    return Some(Err("Demonstration worker stopped before completion".into()));
                }
            }
        }
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
fn run(
    task: LabTask,
    seed: u64,
    parent: &Path,
    cancel: &AtomicBool,
    events: &Sender<Event>,
) -> Result<Demonstration, String> {
    let binary = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .with_file_name("osrs-lab");
    if !binary.is_file() {
        return Err("The Lab tool server is missing. Build osrs-lab alongside the editor before generating demonstrations.".into());
    }
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/lab/train.py");
    if !script.is_file() {
        return Err("The local reconstruction trainer scripts are missing".into());
    }
    let python = std::env::var_os("RUSTOSRS_PYTHON")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let homebrew = PathBuf::from("/opt/homebrew/bin/python3");
            if homebrew.is_file() {
                homebrew
            } else {
                PathBuf::from("python3")
            }
        });
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let directory = parent.join(format!("copy-{nonce}-seed-{seed}"));
    std::fs::create_dir(&directory).map_err(|e| e.to_string())?;
    let task_file = directory.join("task.json");
    let trace_file = directory.join("actions.json");
    write(&task_file, &task)?;
    let metrics =
        std::fs::File::create(directory.join("trainer-output.json")).map_err(|e| e.to_string())?;
    let errors = std::fs::File::create(directory.join("trainer.log")).map_err(|e| e.to_string())?;
    let _ = events.send(Event::Phase("Generating legal tool actions"));
    let mut child = Command::new(python)
        .arg(script)
        .args(["reconstruct", "--method", "imitate", "--binary"])
        .arg(binary)
        .arg("--task")
        .arg(&task_file)
        .arg("--seed")
        .arg(seed.to_string())
        .arg("--out")
        .arg(&trace_file)
        .stdin(Stdio::null())
        .stdout(metrics)
        .stderr(errors)
        .spawn()
        .map_err(|e| e.to_string())?;
    let started = Instant::now();
    let status = loop {
        if cancel.load(Ordering::Relaxed) || started.elapsed() > Duration::from_secs(120) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Demonstration canceled or exceeded its two-minute limit".into());
        }
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        std::thread::sleep(Duration::from_millis(75));
    };
    if !status.success() {
        return Err(format!(
            "Tool reconstruction failed. See {}",
            directory.join("trainer.log").display()
        ));
    }
    let _ = events.send(Event::Phase("Checking reconstruction and replay"));
    let actions: Vec<Action> =
        serde_json::from_str(&std::fs::read_to_string(trace_file).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let mut env = Env::reset(task.clone(), seed).map_err(|e| e.to_string())?;
    for action in &actions {
        if cancel.load(Ordering::Relaxed) {
            return Err("Demonstration canceled".into());
        }
        env.step(action.clone())
            .map_err(|e| format!("Replay validation failed: {e}"))?;
    }
    let score = env.score();
    write(
        &directory.join("metrics.json"),
        &serde_json::json!({
            "format":"rustosrs-copy-demonstration","version":1,"target_visible":true,"learned_policy":false,
            "source":task.source,"group":task.group,"seed":seed,"actions":actions.len(),"score":score,
            "objects":task.target.objects.len(),"terrain_tiles":task.target.tiles.len(),
        }),
    )?;
    if !score.exact {
        return Err(format!(
            "Demonstration replay reached {:.3}, not an exact copy. Inspect {}",
            score.total,
            directory.join("metrics.json").display()
        ));
    }
    Ok(Demonstration {
        actions,
        score: score.total,
        directory,
    })
}
fn write(path: &Path, value: &impl serde::Serialize) -> Result<(), String> {
    std::fs::write(
        path,
        serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
