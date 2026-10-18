//! Tool-driven Lab authority with a read-only scene projection. Authored session/history are parked.
mod accept;
mod projection;
#[cfg(not(target_arch = "wasm32"))]
mod runner;
#[cfg(target_arch = "wasm32")]
#[path = "lab/runner_web.rs"]
mod runner;
use super::*;
use crate::app::{
    edit_runtime::EditRuntime,
    editing::TerrainController,
    npc_runtime::{OpenTask, open_text},
};
use osrs_edit::plan_document_sync;
use osrs_lab::{Action, Env, LabTask, ObservationMode};
use projection::project;
use std::collections::VecDeque;
#[derive(Default)]
pub(super) struct LabPanel {
    pub(super) active: bool,
    parked: Option<(EditRuntime, TerrainController)>,
    env: Option<Env>,
    task: Option<LabTask>,
    import: Option<(Import, OpenTask)>,
    save: Option<crate::app::npc_runtime::SaveTask>,
    feedback: Option<bool>,
    pub(super) accepted: Option<Prefab>,
    replay: VecDeque<Action>,
    replay_source: Vec<Action>,
    playing: bool,
    replay_rate: f64,
    next_replay_at: f64,
    seed: u64,
    hidden: bool,
    problem: Option<String>,
    demonstration: Option<runner::Job>,
    demonstration_result: Option<(f64, usize, std::path::PathBuf)>,
}
#[derive(Clone, Copy)]
enum Import {
    Task,
    Replay,
}
impl LabPanel {
    pub(super) fn generate_demonstration(
        &mut self,
        viewer: &mut EditorApp,
        pad: &MapDocument,
        original: &Prefab,
    ) -> Result<(), String> {
        if self.hidden {
            return Err("Teacher demonstrations require a visible target".into());
        }
        let task = osrs_lab::reconstruction_task(original, &|id, _| {
            viewer
                .placer
                .known_object_size(id.get())
                .map(|(w, h)| (u16::from(w), u16::from(h)))
        })?;
        self.load(viewer, pad, task.clone())?;
        self.active = true;
        self.demonstration = Some(runner::Job::start(task, self.seed)?);
        Ok(())
    }
    pub(super) fn play_replay(&mut self) {
        self.playing = !self.replay.is_empty();
    }
    pub(super) fn report(&self) {
        if let Some(env) = &self.env {
            eprintln!(
                "smoke: lab: score {:.6}, exact {}, objects {}, replay {}, running {}",
                env.score().total,
                env.score().exact,
                env.document().object_entries().len(),
                self.replay.len(),
                self.demonstration.is_some()
            );
        }
    }
    pub(super) fn author_revision(&self, viewer: &EditorApp) -> u64 {
        self.parked.as_ref().map_or_else(
            || viewer.session.edit_revision(),
            |(s, _)| s.edit_revision(),
        )
    }
    pub(super) fn leave(&mut self, viewer: &mut EditorApp) -> Result<(), String> {
        if let Some((session, _)) = &self.parked {
            let patch = plan_document_sync(viewer.session.document(), session.document())
                .map_err(|_| "Scene height coverage changed; wait for the scene to settle")?;
            if !patch.is_empty()
                && !super::super::editing::apply_baseline_patch(
                    viewer,
                    patch,
                    "Restore authored prefab",
                )
            {
                return Err("Restoring the authored scene projection failed".into());
            }
        }
        if let Some((session, terrain)) = self.parked.take() {
            viewer.session = session;
            viewer.terrain = terrain;
            viewer.interaction.clear_selection();
        }
        self.demonstration = None;
        self.active = false;
        self.task = None;
        self.replay.clear();
        self.replay_source.clear();
        self.demonstration_result = None;
        self.env = None;
        self.playing = false;
        Ok(())
    }
    fn load(
        &mut self,
        viewer: &mut EditorApp,
        pad: &MapDocument,
        task: LabTask,
    ) -> Result<(), String> {
        let env = Env::reset(task.clone(), self.seed).map_err(|e| e.to_string())?;
        self.demonstration = None;
        self.demonstration_result = None;
        self.replay.clear();
        self.replay_source.clear();
        self.playing = false;
        let already_parked = self.parked.is_some();
        if !already_parked {
            viewer.cancel_stroke();
            let clone = viewer.session.document().clone();
            let session = std::mem::replace(
                &mut viewer.session,
                EditRuntime::new(crate::session::EditorSession::new(clone)),
            );
            let terrain = std::mem::take(&mut viewer.terrain);
            self.parked = Some((session, terrain));
        }
        if let Err(error) = project(viewer, pad, &env) {
            if !already_parked && let Some((session, terrain)) = self.parked.take() {
                viewer.session = session;
                viewer.terrain = terrain;
            }
            return Err(error);
        }
        self.env = Some(env);
        self.task = Some(task);
        self.problem = None;
        self.feedback = None;
        Ok(())
    }
    pub(super) fn show(
        &mut self,
        viewer: &mut EditorApp,
        pad: &MapDocument,
        original: &Prefab,
        ui: &mut egui::Ui,
    ) {
        if let Some(job) = &mut self.demonstration
            && let Some(result) = job.poll()
        {
            self.demonstration = None;
            match result {
                Ok(result) => {
                    self.demonstration_result =
                        Some((result.score, result.actions.len(), result.directory));
                    self.replay_source = result.actions.clone();
                    self.replay = result.actions.into();
                    self.problem = None;
                }
                Err(e) => self.problem = Some(e),
            }
        }
        ui.heading("Copy a building");
        ui.label(format!(
            "Saved target: {} · {}×{} · {} objects",
            original.name,
            original.width,
            original.height,
            original.objects.len()
        ));
        ui.weak("Start empty, reconstruct with legal tools, then compare every placement, rotation, floor and height. Teacher demonstrations create training data; they do not fit an AI model.");
        egui::CollapsingHeader::new("What training needs next").show(ui, |ui| {
            ui.label("1. Verify the copy demonstration and its tool actions.");
            ui.label("2. Train a policy on scene observations and the next tool action; measure reconstruction rewards.");
            ui.label("3. Add varied reviewed buildings and wall/door/roof labels. Keep copies and rotations together when splitting training from evaluation.");
            ui.label("One building tests memorization. New held-out buildings test whether the policy learned reusable rules.");
        });
        if !self.hidden {
            egui::CollapsingHeader::new("Target floor plan").show(ui, |ui| {
                crate::app::prefab_panel::preview::show(ui, original, 180.0);
            });
        }
        let demonstrate = ui
            .add_enabled(
                !self.hidden && self.demonstration.is_none() && !cfg!(target_arch = "wasm32"),
                egui::Button::new("Generate copy demonstration"),
            )
            .clicked();
        if demonstrate && let Err(e) = self.generate_demonstration(viewer, pad, original) {
            self.problem = Some(e);
        }
        if let Some(job) = &self.demonstration {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(job.phase);
            });
            if ui.button("Cancel demonstration").clicked() {
                self.demonstration = None;
            }
        }
        if let Some((score, actions, directory)) = &self.demonstration_result {
            ui.label(format!("Verified teacher score {score:.3} · {actions} actions. Step or play the reconstruction below."));
            ui.add(egui::Label::new(directory.display().to_string()).truncate())
                .on_hover_text(directory.display().to_string());
        }
        if let Some(job) = &mut self.save
            && let Some(result) = job.poll()
        {
            self.save = None;
            if let Err(e) = result {
                self.problem = Some(e);
            }
        }
        let mut task_to_load = None;
        if let Some((kind, job)) = &mut self.import
            && let Some(result) = job.poll()
        {
            let kind = *kind;
            self.import = None;
            match result {
                Ok(Some(file)) => {
                    let result: Result<(), String> = match kind {
                        Import::Task => serde_json::from_str(&file.text)
                            .map(|task| task_to_load = Some(task))
                            .map_err(|e| e.to_string()),
                        Import::Replay => serde_json::from_str::<Vec<Action>>(&file.text)
                            .map(|actions| {
                                self.replay_source = actions.clone();
                                self.replay = actions.into();
                            })
                            .map_err(|e| e.to_string()),
                    };
                    if let Err(e) = result {
                        self.problem = Some(e)
                    }
                }
                Ok(None) => {}
                Err(e) => self.problem = Some(e),
            }
        }
        ui.horizontal_wrapped(|ui| {
            ui.label("Task seed");
            ui.add(egui::DragValue::new(&mut self.seed));
            if ui
                .checkbox(&mut self.hidden, "Private target benchmark")
                .changed()
            {
                self.demonstration = None;
                self.demonstration_result = None;
                self.replay.clear();
                self.replay_source.clear();
                self.playing = false;
                task_to_load = self.task.clone();
            }
            if ui.button("Generated tavern").clicked() {
                match super::super::prefab_runtime::starter_kit(0)
                    .and_then(|kit| osrs_lab::generated_tavern(&kit, self.seed))
                {
                    Ok(task) => task_to_load = Some(task),
                    Err(e) => self.problem = Some(e),
                }
            }
            if ui
                .add_enabled(
                    original.building.is_some(),
                    egui::Button::new("Terrain relocation"),
                )
                .clicked()
            {
                match osrs_lab::relocation_task(original, self.seed) {
                    Ok(task) => task_to_load = Some(task),
                    Err(e) => self.problem = Some(e),
                }
            }
            if ui.button("This prefab").clicked() {
                match osrs_lab::reconstruction_task(original, &|id, _| {
                    viewer
                        .placer
                        .known_object_size(id.get())
                        .map(|(w, h)| (u16::from(w), u16::from(h)))
                }) {
                    Ok(task) => task_to_load = Some(task),
                    Err(e) => self.problem = Some(e),
                }
            }
            if ui.button("Export task").clicked() {
                let result = self
                    .task
                    .clone()
                    .map(Ok)
                    .unwrap_or_else(|| {
                        osrs_lab::reconstruction_task(original, &|id, _| {
                            viewer
                                .placer
                                .known_object_size(id.get())
                                .map(|(w, h)| (u16::from(w), u16::from(h)))
                        })
                    })
                    .and_then(|task| {
                        serde_json::to_string_pretty(&task).map_err(|e| e.to_string())
                    });
                match result {
                    Ok(text) => {
                        self.save = Some(crate::app::npc_runtime::save_text("copy-task.json", text))
                    }
                    Err(e) => self.problem = Some(e),
                }
            }
            if ui.button("Import task").clicked() {
                self.import = Some((Import::Task, open_text("Load a Lab task", "json")));
            }
        });
        if let Some(task) = task_to_load.take()
            && let Err(e) = self.load(viewer, pad, task)
        {
            self.problem = Some(e)
        }
        let mut action = None;
        let mut reset_replay = false;
        if let Some(env) = &self.env {
            let score = env.score();
            ui.strong(format!(
                "Score {:.3} · {} actions remaining{}",
                score.total,
                env.legal_mask().remaining,
                if score.exact { " · Complete" } else { "" }
            ));
            if !self.hidden {
                ui.label(format!(
                    "Terrain {:.3} · heights {:.3} · objects {:.3} · edges {:.3}",
                    score.terrain, score.height, score.objects, score.edges
                ));
            }
            ui.label(format!("{} replay actions remaining", self.replay.len()));
            if let Some(next) = self.replay.front() {
                egui::CollapsingHeader::new("Next tool action").show(ui, |ui| {
                    if let Ok(text) = serde_json::to_string_pretty(next) {
                        ui.monospace(text);
                    }
                });
            }
            ui.horizontal_wrapped(|ui| {
                if ui.button("Reset reconstruction").clicked() {
                    reset_replay = true;
                }
                if ui
                    .add_enabled(env.legal_mask().can_undo, egui::Button::new("Undo action"))
                    .clicked()
                {
                    action = Some(Action::Undo)
                }
                if ui.button("Import replay").clicked() {
                    self.import = Some((Import::Replay, open_text("Load tool actions", "json")));
                }
                if ui
                    .add_enabled(!self.replay.is_empty(), egui::Button::new("Step replay"))
                    .clicked()
                {
                    action = self.replay.pop_front();
                }
                ui.checkbox(&mut self.playing, "Play replay");
                if self.replay_rate <= 0.0 {
                    self.replay_rate = 10.0;
                }
                ui.add(egui::Slider::new(&mut self.replay_rate, 1.0..=30.0).text("Actions / sec"));
                if ui.button("Save action trace").clicked() {
                    match serde_json::to_string_pretty(env.trace()) {
                        Ok(text) => {
                            self.save =
                                Some(crate::app::npc_runtime::save_text("actions.json", text));
                        }
                        Err(e) => self.problem = Some(e.to_string()),
                    }
                }
            });
            ui.horizontal_wrapped(|ui| {
                if ui.button("Accept as new prefab").clicked() {
                    self.feedback = Some(true);
                    self.accepted = Some(accept::prefab(viewer, original, env));
                }
                if ui.button("Reject result").clicked() {
                    self.feedback = Some(false);
                }
                if ui.button("Save feedback").clicked()
                    && let Some(task) = &self.task
                {
                    let value = serde_json::json!({"format":"rustosrs-lab-feedback","version":1,"source":task.source,"group":task.group,"seed":env.seed(),"private_target":self.hidden,"accepted":self.feedback,"score":score,"actions":env.trace()});
                    match serde_json::to_string_pretty(&value) {
                        Ok(text) => {
                            self.save = Some(crate::app::npc_runtime::save_text(
                                "feedback.json",
                                text,
                            ))
                        }
                        Err(e) => self.problem = Some(e.to_string()),
                    }
                }
            });
            let now = ui.input(|i| i.time);
            if self.playing
                && action.is_none()
                && now >= self.next_replay_at
                && viewer.world.in_flight_count() == 0
            {
                self.next_replay_at = now + 1.0 / self.replay_rate.max(1.0);
                action = self.replay.pop_front();
                if action.is_none() {
                    self.playing = false;
                }
            }
        }
        if reset_replay && let Some(task) = self.task.clone() {
            action = None;
            let source = self.replay_source.clone();
            let result = self.demonstration_result.clone();
            match self.load(viewer, pad, task) {
                Ok(()) => {
                    self.replay_source = source.clone();
                    self.replay = source.into();
                    self.demonstration_result = result;
                }
                Err(e) => self.problem = Some(e),
            }
        }
        if let Some(action) = action
            && let Some(env) = &mut self.env
        {
            let before = env.clone();
            match env.step(action) {
                Ok(_) => {
                    if let Err(e) = project(viewer, pad, env) {
                        *env = before;
                        self.problem = Some(e);
                        self.playing = false
                    }
                }
                Err(e) => {
                    self.problem = Some(e.to_string());
                    self.playing = false
                }
            }
        }
        if let Some(problem) = &self.problem {
            ui.colored_label(ui.visuals().error_fg_color, problem);
        }
    }
}
