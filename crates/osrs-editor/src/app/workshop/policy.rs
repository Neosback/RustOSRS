//! Local neural policy experiments, deliberately separate from style tables and copy practice.
#![cfg_attr(target_arch = "wasm32", allow(dead_code, unused_imports))]
#[cfg(not(target_arch = "wasm32"))]
mod job;
mod preview;
use crate::app::EditorApp;
use eframe::egui;
use std::path::PathBuf;

pub(in crate::app) struct Panel {
    pub(super) dataset: String,
    pub(super) corpus: String,
    pub(super) model: String,
    steps: u32,
    seed: u64,
    style: String,
    #[cfg(not(target_arch = "wasm32"))]
    running: Option<job::Job>,
    message: String,
    log: Vec<String>,
    losses: Vec<(f32, f32)>,
    progress: f32,
    replay: Option<(osrs_lab::LabTask, Vec<osrs_lab::Action>)>,
    preview_files: Option<(PathBuf, PathBuf)>,
}
impl Default for Panel {
    fn default() -> Self {
        let root = root();
        Self {
            dataset: root.join("datasets/current").display().to_string(),
            corpus: String::new(),
            model: root.join("models/current").display().to_string(),
            steps: 2000,
            seed: 7,
            style: String::new(),
            #[cfg(not(target_arch = "wasm32"))]
            running: None,
            message: "Policy models remain experimental until held-out and baseline evaluation."
                .into(),
            log: vec![],
            losses: vec![],
            progress: 0.0,
            replay: None,
            preview_files: None,
        }
    }
}
fn root() -> PathBuf {
    crate::settings::path()
        .and_then(|p| p.parent().map(|p| p.join("building-policy")))
        .unwrap_or_else(|| PathBuf::from("building-policy"))
}
fn python(base: bool) -> PathBuf {
    if !base {
        if let Some(path) = std::env::var_os("RUSTOSRS_POLICY_PYTHON") {
            return path.into();
        }
        return root().join("runtime/bin/python");
    }
    std::env::var_os("RUSTOSRS_PYTHON")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let path = PathBuf::from("/opt/homebrew/bin/python3");
            if path.is_file() {
                path
            } else {
                PathBuf::from("python3")
            }
        })
}
pub(super) fn binary() -> Result<PathBuf, String> {
    let path = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .with_file_name("osrs-lab");
    if path.is_file() {
        return Ok(path);
    }
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/osrs-lab");
    if path.is_file() {
        Ok(path)
    } else {
        Err("Build the osrs-lab tool server beside the editor first".into())
    }
}
#[cfg(not(target_arch = "wasm32"))]
impl Panel {
    fn start(&mut self, args: Vec<String>, base: bool) -> Result<(), String> {
        if self.running.is_some() {
            return Err("Finish or cancel the current policy job first".into());
        }
        let script =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scripts/lab/policy_cli.py");
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let directory = root().join(format!("runs/{nonce}"));
        self.running = Some(job::Job::start(&python(base), &script, &args, &directory)?);
        self.message = "Policy worker starting…".into();
        self.progress = 0.0;
        Ok(())
    }
    fn poll(&mut self) {
        let Some(job) = &mut self.running else {
            return;
        };
        let mut done = job.poll();
        for line in job.events() {
            if let Ok(event) = serde_json::from_str::<serde_json::Value>(&line) {
                if let Some(message) = event["message"].as_str() {
                    self.message = message.into();
                }
                if let (Some(step), Some(total)) = (event["step"].as_f64(), event["total"].as_f64())
                {
                    self.progress = (step / total.max(1.0)) as f32;
                }
                if let (Some(step), Some(loss)) = (event["step"].as_f64(), event["loss"].as_f64()) {
                    self.losses.push((step as f32, loss as f32));
                    if self.losses.len() > 200 {
                        self.losses.remove(0);
                    }
                }
                if event["stage"] == "failed" {
                    done = Some(Err(self.message.clone()));
                }
            }
            self.log.push(line);
            if self.log.len() > 100 {
                self.log.remove(0);
            }
        }
        if let Some(result) = done {
            let cancelled = job.cancelled;
            self.running = None;
            if let Err(e) = result {
                self.message = e;
            } else if cancelled {
                self.message = "Job cancelled; completed checkpoints remain resumable.".into();
            } else if let Some((task, actions)) = self.preview_files.take() {
                match preview::read_replay(&task, &actions, self.seed) {
                    Ok(replay) => {
                        self.replay = Some(replay);
                        self.message =
                            "Policy replay verified. Watch and review it before saving.".into();
                    }
                    Err(e) => self.message = e,
                }
            }
        }
    }
}
pub(in crate::app) fn poll(app: &mut EditorApp) {
    #[cfg(not(target_arch = "wasm32"))]
    app.ui.building_policy.poll();
    #[cfg(target_arch = "wasm32")]
    let _ = app;
}
pub(in crate::app) fn show(app: &mut EditorApp, ui: &mut egui::Ui) {
    ui.heading("Train a building policy");
    ui.label("A small transformer learns editor tool actions from verified demonstrations, completion and repair examples. New-building inference receives no reference building.");
    #[cfg(target_arch = "wasm32")]
    {
        let _ = app;
        ui.label("Local policy jobs require the desktop app.");
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut panel = std::mem::take(&mut app.ui.building_policy);
        let result = controls(&mut panel, app, ui);
        if let Err(e) = result {
            panel.message = e;
        }
        ui.label(&panel.message);
        app.ui.building_policy = panel;
    }
}
#[cfg(not(target_arch = "wasm32"))]
fn controls(panel: &mut Panel, app: &mut EditorApp, ui: &mut egui::Ui) -> Result<(), String> {
    let busy = panel.running.is_some();
    ui.add_enabled_ui(!busy, |ui| {
        ui.label("Frozen dataset folder");
        ui.text_edit_singleline(&mut panel.dataset);
        ui.label("Optional exported cache corpus folder");
        ui.text_edit_singleline(&mut panel.corpus);
        ui.label("Policy model folder");
        ui.text_edit_singleline(&mut panel.model);
        ui.horizontal(|ui| {
            ui.label("Training steps");
            ui.add(egui::DragValue::new(&mut panel.steps).range(1..=100000));
            ui.label("Seed");
            ui.add(egui::DragValue::new(&mut panel.seed));
        });
    });
    let mut args = None;
    let mut base = false;
    ui.add_enabled_ui(!busy, |ui| {
        if ui.button("1 · Set up local MLX runtime").clicked() {
            base = true;
            args = Some(vec![
                "setup".into(),
                "--runtime".into(),
                root().join("runtime").display().to_string(),
            ]);
        }
        if ui.button("2 · Prepare and verify demonstrations").clicked() {
            base = true;
            let prefabs = crate::settings::path()
                .and_then(|p| p.parent().map(|p| p.join("prefabs")))
                .unwrap_or_default();
            let mut prepare = vec![
                "prepare".into(),
                "--prefabs".into(),
                prefabs.display().to_string(),
                "--out".into(),
                panel.dataset.clone(),
            ];
            if !panel.corpus.trim().is_empty() {
                prepare.extend(["--corpus".into(), panel.corpus.clone()]);
            }
            args = Some(prepare);
        }
        ui.horizontal(|ui| {
            for (label, resume) in [("3 · Train policy", false), ("Resume", true)] {
                if ui.button(label).clicked() {
                    let mut fit = vec![
                        "fit".into(),
                        "--dataset".into(),
                        panel.dataset.clone(),
                        "--out".into(),
                        panel.model.clone(),
                        "--steps".into(),
                        panel.steps.to_string(),
                        "--seed".into(),
                        panel.seed.to_string(),
                    ];
                    if resume {
                        fit.push("--resume".into());
                    }
                    args = Some(fit);
                }
            }
        });
        if ui.button("4 · Evaluate held-out buildings").clicked() {
            args = Some(vec![
                "evaluate".into(),
                "--bundle".into(),
                panel.model.clone(),
                "--dataset".into(),
                panel.dataset.clone(),
                "--out".into(),
                root().join("evaluation").display().to_string(),
            ]);
        }
    });
    if let Some(mut args) = args {
        if args[0] == "prepare" || args[0] == "evaluate" {
            args.extend(["--binary".into(), binary()?.display().to_string()]);
        }
        panel.start(args, base)?;
    }
    preview::controls(panel, app, ui, busy)?;
    if let Some(job) = &mut panel.running {
        ui.add(egui::ProgressBar::new(panel.progress).show_percentage());
        if ui.button("Cancel job safely").clicked() {
            job.cancel();
            panel.message = "Cancelling after the current step; keeping checkpoints.".into();
        }
    }
    ui.collapsing("Dataset readiness",|ui|{
        let manifest=PathBuf::from(&panel.dataset).join("manifest.json");
        if let Ok(text)=std::fs::read_to_string(manifest) && let Ok(value)=serde_json::from_str::<serde_json::Value>(&text){
            ui.label(format!("Examples by split: {}",value["examples"]));
            ui.label(format!("Excluded sources: {}",value["rejected"].as_array().map_or(0,Vec::len)));
            ui.weak("Inferred cache labels stay weak. A missing held-out split cannot establish generalization.");
        }else{ui.label("Prepare a dataset to inspect verified examples and source-group splits.");}
    });
    Ok(())
}
pub(in crate::app) fn tray(app: &mut EditorApp, ui: &mut egui::Ui) {
    let panel = &app.ui.building_policy;
    ui.strong("Runs and results");
    ui.label(&panel.message);
    if panel.losses.len() > 1 {
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 48.0), egui::Sense::hover());
        let max = panel.losses.iter().map(|(_, y)| *y).fold(1.0_f32, f32::max);
        let last = panel.losses.last().map_or(1.0, |(x, _)| *x).max(1.0);
        let points: Vec<_> = panel
            .losses
            .iter()
            .map(|(x, y)| {
                egui::pos2(
                    rect.left() + x / last * rect.width(),
                    rect.bottom() - y / max * rect.height(),
                )
            })
            .collect();
        ui.painter().add(egui::Shape::line(
            points,
            egui::Stroke::new(1.5, ui.visuals().selection.stroke.color),
        ));
    }
    ui.collapsing("Worker log", |ui| {
        egui::ScrollArea::vertical()
            .max_height(120.0)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for line in &panel.log {
                    ui.monospace(line);
                }
            });
    });
}
