//! The WFC tab: what the floor-pattern solver sees in the selected building, and a test that runs
//! it. Everything is drawn from [`osrs_edit::wfc_look`], so what is shown is exactly what the
//! solver is given.

use super::table::table_for;
use crate::app::EditorApp;
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};
use osrs_edit::{
    WfcLens, WfcLensEdge, WfcPieceClass, WfcSample, WfcStep, WfcTask, wfc_look, wfc_refloor_task,
    wfc_sample, wfc_trace,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Test {
    #[default]
    Refloor,
    Free,
}

pub(super) struct WfcState {
    stamp: Option<(String, usize, usize, u8)>,
    plane: u8,
    lens: Option<Result<WfcLens, String>>,
    kind: Option<u16>,
    test: Test,
    tries: u32,
    seed: u64,
    task: Option<WfcTask>,
    samples: Vec<WfcSample>,
    shown: usize,
    steps: Vec<WfcStep>,
    step: usize,
    trace_error: Option<String>,
}

impl Default for WfcState {
    fn default() -> Self {
        Self {
            stamp: None,
            plane: 0,
            lens: None,
            kind: None,
            test: Test::default(),
            tries: 8,
            seed: 1,
            task: None,
            samples: Vec::new(),
            shown: 0,
            steps: Vec::new(),
            step: 0,
            trace_error: None,
        }
    }
}

fn kind_color(kind: u16) -> Color32 {
    // Golden-angle hues keep neighbouring kinds apart.
    let hue = (f32::from(kind) * 0.381_966).fract();
    egui::ecolor::Hsva::new(hue, 0.5, 0.9, 1.0).into()
}

pub(super) fn show(app: &mut EditorApp, ui: &mut egui::Ui) {
    let Some(file) = app.prefabs.selected.clone() else {
        ui.weak("Pick a building on the left to see what WFC makes of it.");
        return;
    };
    let Some(prefab) = app.prefabs.entry(&file).map(|e| e.prefab.clone()) else {
        ui.weak("That building is no longer in the library.");
        return;
    };
    let stamp = (
        file.clone(),
        prefab.objects.len(),
        prefab.tiles.len(),
        app.prefabs.workshop.wfc.plane,
    );
    if app.prefabs.workshop.wfc.stamp.as_ref() != Some(&stamp) {
        let (table, _) = table_for(app, &[&prefab]);
        let classify = |id: u32, _kind: u8| {
            let info = table.get(&osrs_core::ids::ObjectId::new(id));
            let says = |w: &str| info.is_some_and(|i| i.says(w));
            if says("door") || says("gate") {
                WfcPieceClass::Door
            } else if says("window") {
                WfcPieceClass::Window
            } else {
                WfcPieceClass::Wall
            }
        };
        let state = &mut app.prefabs.workshop.wfc;
        let plane = state.plane.min(prefab.planes.saturating_sub(1));
        let lens = wfc_look(&prefab, plane, &classify);
        state.task = lens.as_ref().ok().and_then(wfc_refloor_task);
        *state = WfcState {
            stamp: Some(stamp),
            plane,
            lens: Some(lens),
            task: state.task.clone(),
            ..WfcState::default()
        };
        state.test = Test::Refloor;
    }
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.weak(
            "WFC (wave function collapse) is the editor's floor-pattern builder. It learns which \
             floor tiles may sit next to each other from your building, then fills floors in. \
             This page shows exactly what it learned from this building, and lets you try it.",
        );
            if prefab.planes > 1 {
                let state = &mut app.prefabs.workshop.wfc;
                ui.horizontal(|ui| {
                    ui.label("Floor");
                    for plane in 0..prefab.planes.min(4) {
                        if ui
                            .selectable_label(state.plane == plane, format!("{}", plane + 1))
                            .clicked()
                        {
                            state.plane = plane;
                        }
                    }
                });
            }
            let state = &mut app.prefabs.workshop.wfc;
            let Some(lens) = state.lens.clone() else {
                return;
            };
            let lens = match lens {
                Ok(lens) => lens,
                Err(error) => {
                    ui.colored_label(
                        ui.visuals().error_fg_color,
                        format!("WFC cannot use this building: {error}."),
                    );
                    return;
                }
            };
            sees(ui, state, &lens);
            ignores(ui, &lens);
            beside(ui, state, &lens);
            test(ui, state, &lens);
        });
}

fn heading(ui: &mut egui::Ui, text: &str) {
    ui.add_space(8.0);
    ui.label(egui::RichText::new(text).strong().size(16.0));
}

/// Draw `cells` (row by row from the south) with a colour per kind and the edges of each kind.
#[allow(clippy::type_complexity)]
fn draw_cells(
    ui: &mut egui::Ui,
    lens: &WfcLens,
    cells: &[Option<u16>],
    width: usize,
    height: usize,
    pick: Option<&mut Option<u16>>,
    heat: Option<(&[u16], Option<(u16, u16)>)>,
) {
    let cell = ((ui.available_width().min(420.0) - 8.0) / width as f32).clamp(8.0, 30.0);
    let size = Vec2::new(cell * width as f32, cell * height as f32);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, ui.visuals().extreme_bg_color);
    let origin = |x: usize, y: usize| {
        Pos2::new(
            rect.left() + x as f32 * cell,
            rect.top() + (height - 1 - y) as f32 * cell,
        )
    };
    for y in 0..height {
        for x in 0..width {
            let Some(kind) = cells[y * width + x] else {
                continue;
            };
            let r = Rect::from_min_size(origin(x, y), Vec2::splat(cell));
            let mut color = kind_color(kind);
            if let Some((options, _)) = heat {
                let n = options[y * width + x];
                color = if n <= 1 {
                    kind_color(kind)
                } else {
                    Color32::from_gray(200u8.saturating_sub((n.min(20) * 6) as u8))
                };
            }
            painter.rect_filled(r.shrink(0.5), 0.0, color);
            if cell >= 14.0 {
                painter.text(
                    r.center(),
                    egui::Align2::CENTER_CENTER,
                    kind.to_string(),
                    egui::FontId::proportional(cell * 0.45),
                    Color32::from_black_alpha(160),
                );
            }
            let edges = lens.kinds[usize::from(kind)].edges;
            let (tl, tr, bl, br) = (
                r.left_top(),
                r.right_top(),
                r.left_bottom(),
                r.right_bottom(),
            );
            for (edge, a, b) in [
                (edges[0], tl, tr),
                (edges[1], tr, br),
                (edges[2], bl, br),
                (edges[3], tl, bl),
            ] {
                match edge {
                    WfcLensEdge::Wall => {
                        painter.line_segment([a, b], Stroke::new(3.0, Color32::from_gray(40)));
                    }
                    WfcLensEdge::Door => {
                        painter.line_segment(
                            [a, b],
                            Stroke::new(3.0, Color32::from_rgb(235, 130, 20)),
                        );
                    }
                    WfcLensEdge::Open => {}
                }
            }
        }
    }
    if let Some((_, Some((sx, sy)))) = heat {
        let r = Rect::from_min_size(origin(usize::from(sx), usize::from(sy)), Vec2::splat(cell));
        painter.rect_stroke(
            r,
            0.0,
            Stroke::new(3.0, Color32::RED),
            egui::StrokeKind::Inside,
        );
    }
    if let (Some(pick), Some(pos)) = (pick, response.interact_pointer_pos())
        && response.clicked()
    {
        let x = ((pos.x - rect.left()) / cell) as usize;
        let y = height - 1 - (((pos.y - rect.top()) / cell) as usize).min(height - 1);
        if x < width {
            *pick = cells[y * width + x];
        }
    }
}

fn sees(ui: &mut egui::Ui, state: &mut WfcState, lens: &WfcLens) {
    heading(ui, "1  What WFC sees");
    ui.label(format!(
        "Your building's floor is {} tiles. WFC sorts them into {} kinds. A kind is the floor material plus what stands on each of the tile's four edges. Same colour, same kind. Dark edge: wall. Orange edge: door.",
        lens.floors,
        lens.kinds.len()
    ));
    let (w, h) = (usize::from(lens.width), usize::from(lens.height));
    ui.horizontal_top(|ui| {
        draw_cells(ui, lens, &lens.cells, w, h, Some(&mut state.kind), None);
        ui.vertical(|ui| {
            for line in lens.findings() {
                ui.label(format!("\u{2022} {line}"));
            }
        });
    });
    ui.weak("Click a tile to see which kinds it may sit beside.");
    ui.add_space(4.0);
    egui::CollapsingHeader::new(format!("The {} kinds", lens.kinds.len())).default_open(false).show(ui, |ui| {
        for kind in &lens.kinds {
            ui.horizontal(|ui| {
                swatch(ui, kind.id);
                let edge = |e: WfcLensEdge| match e {
                    WfcLensEdge::Open => "open",
                    WfcLensEdge::Wall => "wall",
                    WfcLensEdge::Door => "door",
                };
                ui.label(format!(
                    "Kind {} \u{b7} {} tile(s) \u{b7} floor {}/{} \u{b7} north {}, east {}, south {}, west {}",
                    kind.id,
                    kind.count,
                    kind.underlay,
                    kind.overlay,
                    edge(kind.edges[0]),
                    edge(kind.edges[1]),
                    edge(kind.edges[2]),
                    edge(kind.edges[3]),
                ));
            });
        }
    });
}

fn swatch(ui: &mut egui::Ui, kind: u16) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(14.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 2.0, kind_color(kind));
}

fn ignores(ui: &mut egui::Ui, lens: &WfcLens) {
    heading(ui, "2  What WFC does not see");
    let i = &lens.ignored;
    ui.label(format!(
        "It ignores everything that is not the floor or a wall on a tile edge: {} furniture, {} wall decorations, {} roof pieces and {} other objects. It also cannot tell what a room is for, and {} window(s) look like plain walls to it. Its picture of the building is only the coloured tiles above.",
        i.furniture, i.decorations, i.roof, i.other, i.windows
    ));
}

fn beside(ui: &mut egui::Ui, state: &mut WfcState, lens: &WfcLens) {
    let Some(kind) = state.kind.and_then(|k| lens.kinds.get(usize::from(k))) else {
        return;
    };
    heading(ui, "3  Who may sit beside whom");
    ui.horizontal(|ui| {
        swatch(ui, kind.id);
        ui.label(format!("Kind {} was seen next to these kinds:", kind.id));
    });
    for (name, list) in ["north of it", "east of it", "south of it", "west of it"]
        .iter()
        .zip(&kind.beside)
    {
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("  {name}:"));
            if list.is_empty() {
                ui.weak("nothing (it only goes on the edge of a floor)");
            }
            for other in list {
                swatch(ui, *other);
                ui.label(other.to_string());
            }
        });
    }
}

fn test(ui: &mut egui::Ui, state: &mut WfcState, lens: &WfcLens) {
    heading(ui, "4  Try it");
    ui.horizontal_wrapped(|ui| {
        ui.radio_value(&mut state.test, Test::Refloor, "Floor this building again")
            .on_hover_text("Tell WFC where this building's walls and doors are and let it choose the floor tiles");
        ui.radio_value(&mut state.test, Test::Free, "Make a floor from nothing (8 by 8)")
            .on_hover_text("No walls given: WFC must invent a whole floor from what it saw");
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("Tries");
        ui.add(egui::Slider::new(&mut state.tries, 1..=32));
        ui.label("Number");
        ui.add(egui::DragValue::new(&mut state.seed).range(0..=99_999));
    });
    let task = match state.test {
        Test::Refloor => state.task.clone(),
        Test::Free => Some(WfcTask::free(8, 8)),
    };
    let Some(task) = task else {
        ui.weak("This building has no all-floor rectangle to try on.");
        return;
    };
    if ui.button("Run it").clicked() {
        let seeds: Vec<u64> = (0..u64::from(state.tries))
            .map(|i| state.seed + i)
            .collect();
        state.samples = wfc_sample(lens, &task, &seeds);
        state.shown = state
            .samples
            .iter()
            .position(|s| s.result.is_ok())
            .unwrap_or(0);
        (state.steps, state.trace_error) = trace_of(lens, &task, state.seed + state.shown as u64);
        state.step = state.steps.len().saturating_sub(1);
    }
    if state.samples.is_empty() {
        return;
    }
    let worked = state.samples.iter().filter(|s| s.result.is_ok()).count();
    ui.add_space(4.0);
    ui.label(format!(
        "{worked} of {} tries gave a floor.",
        state.samples.len()
    ));
    let total = usize::from(task.width) * usize::from(task.height);
    if worked > 0 {
        let changed: Vec<usize> = state
            .samples
            .iter()
            .filter(|s| s.result.is_ok())
            .map(|s| s.changed)
            .collect();
        let familiar = state
            .samples
            .iter()
            .filter(|s| s.result.is_ok())
            .map(|s| s.familiar)
            .sum::<f32>()
            / worked as f32;
        if state.test == Test::Refloor {
            let average = changed.iter().sum::<usize>() as f32 / changed.len() as f32;
            ui.label(format!(
                "On average {average:.1} of {total} tiles ended up different from your building. {}",
                if average < 0.5 {
                    "So the walls already decided the floor: with one building WFC can only give the same floor back. It mixes only when it has learned from several buildings."
                } else {
                    "It re-chose the floor under your walls."
                }
            ));
        }
        ui.label(format!("{:.0}% of the 2 by 2 blocks of tiles in what it made also exist in your building, so the rest are new combinations.", familiar * 100.0));
    } else if let Some(Err(why)) = state.samples.first().map(|s| s.result.clone()) {
        ui.colored_label(ui.visuals().warn_fg_color, format!("It failed: {why}."));
        if state.test == Test::Free {
            ui.weak("A catalog learned from one building describes that whole building (its outer walls need an outside to touch), so it cannot fill a free area. That is normal; it works on the building's own walls (the other test).");
        }
    }
    ui.horizontal_wrapped(|ui| {
        for (i, s) in state.samples.iter().enumerate() {
            let label = format!(
                "{}{}",
                s.seed,
                if s.result.is_ok() { "" } else { " \u{2717}" }
            );
            if ui.selectable_label(state.shown == i, label).clicked() {
                state.shown = i;
                (state.steps, state.trace_error) = trace_of(lens, &task, s.seed);
                state.step = state.steps.len().saturating_sub(1);
            }
        }
    });
    let (w, h) = (usize::from(task.width), usize::from(task.height));
    let original: Vec<Option<u16>> = if task.original.is_empty() {
        vec![None; w * h]
    } else {
        task.original.iter().map(|k| Some(*k)).collect()
    };
    ui.horizontal_top(|ui| {
        if !task.original.is_empty() {
            ui.vertical(|ui| {
                ui.label("Your building");
                draw_cells(ui, lens, &original, w, h, None, None);
            });
        }
        if let Some(Ok(cells)) = state.samples.get(state.shown).map(|s| s.result.clone()) {
            ui.vertical(|ui| {
                ui.label("What WFC made");
                let cells: Vec<Option<u16>> = cells.into_iter().map(Some).collect();
                draw_cells(ui, lens, &cells, w, h, None, None);
            });
        }
    });
    stepper(ui, state, lens, w, h);
}

fn trace_of(lens: &WfcLens, task: &WfcTask, seed: u64) -> (Vec<WfcStep>, Option<String>) {
    let (steps, result) = wfc_trace(lens, task, seed);
    (steps, result.err())
}

/// Step through the solve: grey tiles are undecided (darker = fewer choices), coloured are fixed.
fn stepper(ui: &mut egui::Ui, state: &mut WfcState, lens: &WfcLens, w: usize, h: usize) {
    if state.steps.is_empty() {
        return;
    }
    ui.add_space(4.0);
    egui::CollapsingHeader::new("Watch how it decided").id_salt("wfc-steps").show(ui, |ui| {
        ui.weak("Each step fixes the tile with the fewest choices left. Grey tiles are undecided (darker means fewer choices), coloured tiles are decided, red is where it got stuck.");
        let last = state.steps.len() - 1;
        ui.horizontal(|ui| {
            if ui.small_button("\u{23ee}").clicked() { state.step = 0; }
            if ui.small_button("\u{25c0}").clicked() { state.step = state.step.saturating_sub(1); }
            ui.add(egui::Slider::new(&mut state.step, 0..=last).text(format!("of {last}")));
            if ui.small_button("\u{25b6}").clicked() { state.step = (state.step + 1).min(last); }
        });
        let step = &state.steps[state.step.min(last)];
        ui.label(&step.note);
        // A decided tile shows its kind: take it from the solution when this step fixed it.
        let mut cells: Vec<Option<u16>> = vec![Some(0); w * h];
        for earlier in &state.steps[..=state.step.min(last)] {
            if let Some((x, y, k)) = earlier.fixed {
                cells[usize::from(y) * w + usize::from(x)] = Some(k);
            }
        }
        let known = state.task.as_ref().filter(|t| !t.original.is_empty());
        for (i, options) in step.options.iter().enumerate() {
            if *options == 1 && cells[i] == Some(0)
                && let Some(t) = known
            {
                cells[i] = t.original.get(i).copied();
            }
        }
        draw_cells(ui, lens, &cells, w, h, None, Some((&step.options, step.stuck)));
        if let Some(error) = &state.trace_error {
            ui.colored_label(ui.visuals().warn_fg_color, format!("The solve ended: {error}."));
        }
    });
}
