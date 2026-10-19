//! Paintable WFC pins and sparse-footprint previews. Only acceptance enters semantic history.
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
    width: u16,
    height: u16,
    mask: Vec<bool>,
    origin: Option<MapTile>,
    stamp: Option<(u64, u64, u8)>,
    x: u16,
    y: u16,
    pattern: u16,
    problem: Option<String>,
    relaxed: usize,
    import: Option<crate::app::npc_runtime::OpenTask>,
    #[cfg(not(target_arch = "wasm32"))]
    pending: Option<std::sync::mpsc::Receiver<Result<osrs_edit::WfcRepairReport, String>>>,
}
impl CatalogView {
    pub(super) fn show(
        &mut self,
        ui: &mut egui::Ui,
        seed: u64,
        viewer: &mut EditorApp,
        current: &str,
    ) {
        if self.width == 0 {
            self.width = 8;
            self.height = 8;
            self.mask = vec![true; 64];
        }
        let stamp = (
            viewer.project.generation(),
            viewer.session.map_revision(),
            viewer.interaction.view_plane(),
        );
        if self.stamp.is_some_and(|s| s != stamp) {
            self.solution = None;
            self.stamp = None;
            #[cfg(not(target_arch = "wasm32"))]
            {
                self.pending = None;
            }
            self.problem = Some("The scene changed; solve again before applying.".into());
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(receiver) = &self.pending {
            match receiver.try_recv() {
                Ok(result) => {
                    self.pending = None;
                    self.finish(result);
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(_) => {
                    self.pending = None;
                    self.problem = Some("WFC worker stopped".into());
                }
            }
        }
        if let Some(job) = &mut self.import
            && let Some(result) = job.poll()
        {
            self.import = None;
            match result {
                Ok(Some(file)) => match serde_json::from_str::<LearnedCatalog>(&file.text)
                    .map_err(|e| e.to_string())
                    .and_then(|c| {
                        c.compile().map_err(|e| e.to_string())?;
                        Ok(c)
                    }) {
                    Ok(c) => {
                        *self = Self {
                            catalog: Some(c),
                            width: 8,
                            height: 8,
                            mask: vec![true; 64],
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
                "Load WFC catalog",
                "json",
            ));
        }
        ui.horizontal_wrapped(|ui| {
            if ui.button("Use selected floor").clicked() {
                let cells: Vec<_> = viewer
                    .interaction
                    .selection_tiles()
                    .iter()
                    .copied()
                    .collect();
                self.use_cells(&cells);
            }
            if ui.button("Use prefab footprint").clicked()
                && let Some(entry) = viewer.prefabs.entry(current)
            {
                let origin = MapTile::new(
                    super::CENTRE.0 - i32::from(entry.prefab.width) / 2,
                    super::CENTRE.1 - i32::from(entry.prefab.height) / 2,
                );
                let cells: Vec<_> = entry
                    .prefab
                    .building
                    .as_ref()
                    .map(|b| {
                        b.floors
                            .iter()
                            .filter(|c| c.plane == stamp.2)
                            .map(|c| (origin.x + i32::from(c.x), origin.y + i32::from(c.y)))
                            .collect()
                    })
                    .unwrap_or_else(|| {
                        (0..entry.prefab.width)
                            .flat_map(|x| {
                                (0..entry.prefab.height).map(move |y| {
                                    (origin.x + i32::from(x), origin.y + i32::from(y))
                                })
                            })
                            .collect()
                    });
                self.use_cells(&cells);
            }
        });
        ui.horizontal(|ui| {
            ui.label("Width / height");
            let changed = ui
                .add(egui::DragValue::new(&mut self.width).range(1..=128))
                .changed()
                | ui.add(egui::DragValue::new(&mut self.height).range(1..=128))
                    .changed();
            if changed {
                self.cancel_pending();
                self.mask = vec![true; usize::from(self.width) * usize::from(self.height)];
                self.pins.clear();
                self.solution = None;
                self.origin = None;
            }
        });
        let Some(data) = &self.catalog else {
            ui.weak("Choose a catalog to solve floor patterns. Wall sockets constrain floors; object realization uses the building generator.");
            return;
        };
        ui.label(format!(
            "{} / {} · {} patterns",
            data.style,
            data.role,
            data.patterns.len()
        ));
        let max = u16::try_from(data.patterns.len().saturating_sub(1)).unwrap_or(0);
        ui.horizontal(|ui| {
            ui.label("Paint pattern");
            ui.add(egui::DragValue::new(&mut self.pattern).range(0..=max));
            if ui.button("Clear pins").clicked() {
                self.cancel_pending();
                self.pins.clear();
            }
        });
        self.grid(ui);
        let mut solve = false;
        let mut repair = false;
        let mut apply = false;
        ui.horizontal_wrapped(|ui| {
            solve = ui.button("Solve preview").clicked();
            repair = ui
                .add_enabled(
                    self.solution.is_some(),
                    egui::Button::new("Repair near selected cell"),
                )
                .clicked();
            apply = ui
                .add_enabled(self.can_apply(), egui::Button::new("Apply floors"))
                .clicked();
        });
        #[cfg(not(target_arch = "wasm32"))]
        if self.pending.is_some() {
            ui.spinner();
            if ui.button("Cancel solve").clicked() {
                self.pending = None;
            }
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(50));
        }
        if solve || repair {
            let data = self.catalog.clone();
            let locks = self.pins.clone();
            let mut frozen = Vec::new();
            let mut suggestions = Vec::new();
            if repair && let Some(solution) = &self.solution {
                for y in 0..self.height {
                    for x in 0..self.width {
                        let coord = WfcCoord::new(x, y);
                        if let Some(p) = solution.at(coord) {
                            let c = WfcConstraint {
                                coord,
                                allowed: PatternDomain::singleton(p),
                            };
                            if x.abs_diff(self.x) > 2 || y.abs_diff(self.y) > 2 {
                                frozen.push(c);
                            } else {
                                suggestions.push(c);
                            }
                        }
                    }
                }
            }
            let locks: Vec<_> = locks.into_iter().chain(frozen).collect();
            let mask = self.mask.clone();
            let request = osrs_edit::WfcRepairRequest {
                width: self.width,
                height: self.height,
                seed,
                max_backtracks: 32,
                max_repairs: 4,
                radius: 2,
            };
            let run = move || {
                let catalog = data
                    .ok_or("No catalog")?
                    .compile()
                    .map_err(|e| e.to_string())?;
                osrs_edit::repair_wfc_masked(&catalog, request, &locks, &suggestions, &mask)
                    .map_err(|e| format!("Cannot satisfy these locks: {e:?}"))
            };
            self.stamp = Some(stamp);
            self.solution = None;
            #[cfg(not(target_arch = "wasm32"))]
            {
                let (send, receive) = std::sync::mpsc::channel();
                self.pending = Some(receive);
                std::thread::spawn(move || {
                    let _ = send.send(run());
                });
            }
            #[cfg(target_arch = "wasm32")]
            self.finish(run());
        }
        if apply && let (Some(data), Some(solution)) = (&self.catalog, &self.solution) {
            let origin = self.origin.unwrap_or(MapTile::new(
                super::CENTRE.0 - i32::from(solution.width()) / 2,
                super::CENTRE.1 - i32::from(solution.height()) / 2,
            ));
            if let Err(e) = apply_solution_at(viewer, data, solution, origin) {
                self.problem = Some(e);
            }
        }
        if let Some(e) = &self.problem {
            ui.colored_label(ui.visuals().warn_fg_color, e);
        }
        if self.relaxed > 0 {
            ui.label(format!(
                "Relaxed {} solver suggestions. Every author pin was retained.",
                self.relaxed
            ));
        }
        if let Some(data) = &self.catalog {
            egui::CollapsingHeader::new("Patterns and weights").show(ui, |ui| {
                for (i, (p, w)) in data.patterns.iter().zip(&data.weights).enumerate() {
                    ui.label(format!(
                        "{i}: floor {} · N/E/S/W {:?} · weight {w}",
                        p.floor_class, p.sockets
                    ));
                }
            });
        }
    }
    fn finish(&mut self, result: Result<osrs_edit::WfcRepairReport, String>) {
        match result {
            Ok(report) => {
                self.solution = Some(report.solution);
                self.relaxed = report.relaxed.len();
                self.problem = None;
            }
            Err(e) => {
                self.solution = None;
                self.problem = Some(e);
            }
        }
    }
    fn cancel_pending(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.pending = None;
        }
    }
    fn can_apply(&self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        if self.pending.is_some() {
            return false;
        }
        self.solution.as_ref().is_some_and(|s| {
            s.width() == self.width
                && s.height() == self.height
                && self
                    .pins
                    .iter()
                    .all(|p| s.at(p.coord).is_some_and(|id| p.allowed.contains(id)))
        })
    }
    fn use_cells(&mut self, cells: &[(i32, i32)]) {
        let Some(min_x) = cells.iter().map(|c| c.0).min() else {
            self.problem = Some("Select floor tiles first".into());
            return;
        };
        let min_y = cells.iter().map(|c| c.1).min().unwrap_or(0);
        let max_x = cells.iter().map(|c| c.0).max().unwrap_or(min_x);
        let max_y = cells.iter().map(|c| c.1).max().unwrap_or(min_y);
        let (w, h) = (max_x - min_x + 1, max_y - min_y + 1);
        if w > 128 || h > 128 {
            self.problem = Some("Select an area up to 128×128 tiles".into());
            return;
        }
        self.width = w as u16;
        self.cancel_pending();
        self.height = h as u16;
        self.origin = Some(MapTile::new(min_x, min_y));
        self.mask = vec![false; w as usize * h as usize];
        for &(x, y) in cells {
            self.mask[(y - min_y) as usize * w as usize + (x - min_x) as usize] = true;
        }
        self.pins.clear();
        self.solution = None;
        self.problem = None;
    }
    fn grid(&mut self, ui: &mut egui::Ui) {
        ui.weak("Click: select repair center · Shift-click: pin pattern · Alt-click: remove pin. Outlined cells are author locks.");
        let cell = (ui.available_width() / f32::from(self.width)).clamp(3.0, 20.0);
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(f32::from(self.width) * cell, f32::from(self.height) * cell),
            egui::Sense::click_and_drag(),
        );
        for y in 0..self.height {
            for x in 0..self.width {
                if !self.mask[usize::from(y) * usize::from(self.width) + usize::from(x)] {
                    continue;
                }
                let coord = WfcCoord::new(x, y);
                let pin = self.pins.iter().find(|p| p.coord == coord);
                let pattern = pin
                    .and_then(|p| p.allowed.iter().next())
                    .or_else(|| self.solution.as_ref().and_then(|s| s.at(coord)));
                let color = pattern.map_or(egui::Color32::from_gray(65), |p| {
                    egui::ecolor::Hsva::new((f32::from(p.get()) * 0.381966).fract(), 0.5, 0.8, 1.0)
                        .into()
                });
                let r = egui::Rect::from_min_size(
                    rect.min
                        + egui::vec2(f32::from(x) * cell, f32::from(self.height - 1 - y) * cell),
                    egui::Vec2::splat(cell),
                );
                ui.painter().rect_filled(r.shrink(0.5), 0.0, color);
                if pin.is_some() {
                    ui.painter().rect_stroke(
                        r,
                        0.0,
                        egui::Stroke::new(2.0, egui::Color32::WHITE),
                        egui::StrokeKind::Inside,
                    );
                }
            }
        }
        if (response.clicked() || response.dragged())
            && let Some(pos) = response.interact_pointer_pos()
        {
            let x = ((pos.x - rect.left()) / cell).floor() as u16;
            let y = self.height
                - 1
                - (((pos.y - rect.top()) / cell).floor() as u16).min(self.height - 1);
            if x < self.width
                && self.mask[usize::from(y) * usize::from(self.width) + usize::from(x)]
            {
                self.x = x;
                self.y = y;
                let modifiers = ui.input(|i| i.modifiers);
                if modifiers.alt || modifiers.shift {
                    self.cancel_pending();
                    self.pins.retain(|p| p.coord != WfcCoord::new(x, y));
                    if modifiers.shift
                        && let Some(p) = WfcPatternId::new(self.pattern)
                    {
                        self.pins.push(WfcConstraint {
                            coord: WfcCoord::new(x, y),
                            allowed: PatternDomain::singleton(p),
                        });
                    }
                }
            }
        }
    }
}
fn apply_solution_at(
    viewer: &mut EditorApp,
    data: &LearnedCatalog,
    solution: &WfcSolution,
    origin: MapTile,
) -> Result<(), String> {
    let plane = SourcePlane::new(viewer.interaction.view_plane()).ok_or("invalid plane")?;
    let patch =
        osrs_edit::plan_wfc_floors(viewer.session.document(), data, solution, origin, plane)
            .map_err(|e| e.to_string())?;
    if !patch.is_empty()
        && !crate::app::editing::commit_map_patch(viewer, patch, "Apply WFC floor preview")
    {
        return Err("Could not apply floor preview".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests;
