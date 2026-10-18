//! Tool-driven Lab authority with a read-only scene projection. Authored session/history are parked.
mod projection;
use super::*;
use crate::app::{
    edit_runtime::EditRuntime,
    editing::TerrainController,
    npc_runtime::{OpenTask, open_text},
};
use osrs_edit::plan_document_sync;
use osrs_lab::{Action, Env, LabTask, ObservationMode};
use projection::{project, task_from_prefab};
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
    playing: bool,
    seed: u64,
    hidden: bool,
    problem: Option<String>,
    catalog: super::catalog::CatalogView,
}
#[derive(Clone, Copy)]
enum Import {
    Task,
    Replay,
    Catalog,
}
impl LabPanel {
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
        self.active = false;
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
        if self.parked.is_none() {
            viewer.cancel_stroke();
            let clone = viewer.session.document().clone();
            let session = std::mem::replace(
                &mut viewer.session,
                EditRuntime::new(crate::session::EditorSession::new(clone)),
            );
            let terrain = std::mem::take(&mut viewer.terrain);
            self.parked = Some((session, terrain));
        }
        project(viewer, pad, &env)?;
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
                            .map(|actions| self.replay = actions.into())
                            .map_err(|e| e.to_string()),
                        Import::Catalog => {
                            serde_json::from_str::<osrs_edit::LearnedCatalog>(&file.text)
                                .map_err(|e| e.to_string())
                                .and_then(|catalog| {
                                    catalog.compile().map_err(|e| e.to_string())?;
                                    self.catalog.catalog = Some(catalog);
                                    Ok(())
                                })
                        }
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
            ui.checkbox(&mut self.hidden, "Private target benchmark");
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
                task_to_load = Some(task_from_prefab(original));
            }
            if ui.button("Import task").clicked() {
                self.import = Some((Import::Task, open_text("Load a Lab task", "json")));
            }
        });
        if let Some(task) = task_to_load
            && let Err(e) = self.load(viewer, pad, task)
        {
            self.problem = Some(e)
        }
        let mut action = None;
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
            ui.horizontal_wrapped(|ui| {
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
                    self.feedback=Some(true);
                    let current=env.observation(ObservationMode::ScoreOnly).current;
                    let legal=env.legal_mask();
                    self.accepted=Some(Prefab{format:osrs_edit::PREFAB_FORMAT.into(),version:osrs_edit::PREFAB_VERSION,name:format!("{} reconstruction",original.name),tags:vec!["lab-reviewed".into()],notes:format!("Accepted local reconstruction; score {} seed {}",score.total,self.seed),source:osrs_edit::PrefabSource::Authored,width:legal.width,height:legal.height,planes:legal.planes,tiles:current.tiles,heights:current.heights,objects:current.objects,building:None});
                }
                if ui.button("Reject result").clicked() {self.feedback=Some(false);}
                if ui.button("Save feedback").clicked() && let Some(task)=&self.task {
                    let value=serde_json::json!({"format":"rustosrs-lab-feedback","version":1,"source":task.source,"group":task.group,"seed":env.seed(),"private_target":self.hidden,"accepted":self.feedback,"score":score,"actions":env.trace()});
                    match serde_json::to_string_pretty(&value) {Ok(text)=>self.save=Some(crate::app::npc_runtime::save_text("feedback.json",text)),Err(e)=>self.problem=Some(e.to_string())}
                }
            });
            if self.playing && action.is_none() && viewer.world.in_flight_count() == 0 {
                action = self.replay.pop_front();
                if action.is_none() {
                    self.playing = false;
                }
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
        ui.separator();
        if ui.button("Import learned WFC catalog").clicked() {
            self.import = Some((
                Import::Catalog,
                open_text("Load learned WFC catalog", "json"),
            ));
        }
        self.catalog.show(ui, self.seed);
        if let Some(problem) = &self.problem {
            ui.colored_label(ui.visuals().error_fg_color, problem);
        }
    }
}
