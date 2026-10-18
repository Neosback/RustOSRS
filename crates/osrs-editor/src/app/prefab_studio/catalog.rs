//! Learned catalogs, pinned patterns and bounded repair through the existing WFC solver.
use crate::app::EditorApp;
use eframe::egui;
use osrs_core::coords::{MapTile, SourcePlane};
use osrs_edit::{
    LearnedCatalog, PatternDomain, WfcConstraint, WfcCoord, WfcPatternId, WfcSolution,
};
#[derive(Default)]
pub(super) struct CatalogView {
    pub(super) catalog: Option<LearnedCatalog>,
    pins: Vec<WfcConstraint>,
    solution: Option<WfcSolution>,
    x: u16,
    y: u16,
    pattern: u16,
    problem: Option<String>,
    import: Option<crate::app::npc_runtime::OpenTask>,
}
impl CatalogView {
    pub(super) fn show(&mut self, ui: &mut egui::Ui, seed: u64, viewer: &mut EditorApp) {
        if let Some(job) = &mut self.import
            && let Some(result) = job.poll()
        {
            self.import = None;
            match result {
                Ok(Some(file)) => match serde_json::from_str::<LearnedCatalog>(&file.text)
                    .map_err(|e| e.to_string())
                    .and_then(|catalog| {
                        catalog.compile().map_err(|e| e.to_string())?;
                        Ok(catalog)
                    }) {
                    Ok(catalog) => {
                        *self = Self {
                            catalog: Some(catalog),
                            ..Self::default()
                        };
                    }
                    Err(e) => self.problem = Some(e),
                },
                Ok(None) => {}
                Err(e) => self.problem = Some(e),
            }
        }
        if ui.button("Import learned WFC catalog").clicked() {
            self.import = Some(crate::app::npc_runtime::open_text(
                "Load learned WFC catalog",
                "json",
            ));
        }
        if let Some(problem) = &self.problem {
            ui.colored_label(ui.visuals().error_fg_color, problem);
        }
        let Some(data) = &self.catalog else {
            ui.weak("Import a learned catalog to inspect its patterns and pin a solution.");
            return;
        };
        ui.weak("Applies terrain patterns to the editable prefab with normal undo and Save. Wall sockets constrain the solve; this tool paints floors only.");
        ui.label(format!(
            "{} / {} · {} patterns",
            data.style,
            data.role,
            data.patterns.len()
        ));
        ui.horizontal(|ui| {
            ui.label("Pin cell");
            ui.add(egui::DragValue::new(&mut self.x).range(0..=7));
            ui.add(egui::DragValue::new(&mut self.y).range(0..=7));
            ui.label("pattern");
            let max_pat = u16::try_from(data.patterns.len().saturating_sub(1)).unwrap_or(0);
            ui.add(egui::DragValue::new(&mut self.pattern).range(0..=max_pat));
            if ui.button("Pin").clicked()
                && let Some(pattern) = WfcPatternId::new(self.pattern)
            {
                self.pins
                    .retain(|p| p.coord != WfcCoord::new(self.x, self.y));
                self.pins.push(WfcConstraint {
                    coord: WfcCoord::new(self.x, self.y),
                    allowed: PatternDomain::singleton(pattern),
                });
            }
            if ui.button("Clear pins").clicked() {
                self.pins.clear();
            }
        });
        let mut solve = false;
        let mut repair = false;
        let mut apply_scene = false;
        ui.horizontal_wrapped(|ui| {
            solve = ui.button("Solve 8×8").clicked();
            repair = ui
                .add_enabled(
                    self.solution.is_some(),
                    egui::Button::new("Repair within 2 tiles"),
                )
                .clicked();
            apply_scene = ui
                .add_enabled(
                    self.solution.is_some(),
                    egui::Button::new("Apply floor layout to prefab"),
                )
                .clicked();
        });
        if solve || repair {
            let mut constraints = self.pins.clone();
            if repair && let Some(solution) = &self.solution {
                for y in 0..8u16 {
                    for x in 0..8u16 {
                        if x.abs_diff(self.x) > 2 || y.abs_diff(self.y) > 2 {
                            let coord = WfcCoord::new(x, y);
                            if let Some(p) = solution.at(coord) {
                                constraints.push(WfcConstraint {
                                    coord,
                                    allowed: PatternDomain::singleton(p),
                                });
                            }
                        }
                    }
                }
            }
            match data.compile().and_then(|catalog| {
                osrs_edit::solve_wfc_with_repair(&catalog, 8, 8, seed, &constraints, 64, 4)
                    .map_err(|_| osrs_edit::LearningError::InvalidCatalog)
            }) {
                Ok(solution) => {
                    self.solution = Some(solution);
                    self.problem = None
                }
                Err(error) => {
                    self.problem = Some(format!("Contradiction or unsupported catalog: {error}"))
                }
            }
        }
        if apply_scene
            && let Some(solution) = &self.solution
            && let Err(e) = apply_solution_to_scene(viewer, data, solution)
        {
            self.problem = Some(e);
        }
        if let Some(problem) = &self.problem {
            ui.colored_label(ui.visuals().error_fg_color, problem);
        }
        if let Some(solution) = &self.solution {
            for y in (0..8).rev() {
                ui.monospace(
                    (0..8)
                        .map(|x| {
                            solution
                                .at(WfcCoord::new(x, y))
                                .map_or(" ?".into(), |p| format!("{:2}", p.get()))
                        })
                        .collect::<Vec<_>>()
                        .join(" "),
                );
            }
        }
        egui::CollapsingHeader::new("Pattern sockets and weights").show(ui, |ui| {
            for (i, (p, w)) in data.patterns.iter().zip(&data.weights).enumerate() {
                ui.label(format!(
                    "{i}: floor {} · N/E/S/W {:?} · {w}",
                    p.floor_class, p.sockets
                ));
            }
        });
    }
}

fn apply_solution_to_scene(
    viewer: &mut EditorApp,
    data: &LearnedCatalog,
    solution: &WfcSolution,
) -> Result<(), String> {
    let plane = SourcePlane::new(viewer.interaction.view_plane()).ok_or("invalid plane")?;
    let origin = MapTile::new(
        super::CENTRE.0 - i32::from(solution.width()) / 2,
        super::CENTRE.1 - i32::from(solution.height()) / 2,
    );
    let patch =
        osrs_edit::plan_wfc_floors(viewer.session.document(), data, solution, origin, plane)
            .map_err(|e| e.to_string())?;
    if !patch.is_empty()
        && !crate::app::editing::commit_map_patch(viewer, patch, "Apply 8×8 WFC floor layout")
    {
        return Err("Could not apply WFC floor layout to Studio scene".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
