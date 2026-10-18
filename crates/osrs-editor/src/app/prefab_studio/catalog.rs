//! Learned catalogs, pinned patterns and bounded repair through the existing WFC solver.
use eframe::egui;
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
    pattern: u8,
    problem: Option<String>,
}
impl CatalogView {
    pub(super) fn show(&mut self, ui: &mut egui::Ui, seed: u64) {
        let Some(data) = &self.catalog else {
            ui.weak("Import a learned catalog to inspect its patterns and pin a solution.");
            return;
        };
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
            ui.add(
                egui::DragValue::new(&mut self.pattern)
                    .range(0..=data.patterns.len().saturating_sub(1) as u8),
            );
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
        ui.horizontal(|ui| {
            solve = ui.button("Solve 8×8").clicked();
            repair = ui
                .add_enabled(
                    self.solution.is_some(),
                    egui::Button::new("Repair within 2 tiles"),
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
                osrs_edit::solve_wfc(&catalog, 8, 8, seed, &constraints, 64)
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
