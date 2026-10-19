//! Inference tasks expose the saved footprint/program, never its target placements to the policy.
use super::*;
use osrs_lab::{Action, Env, LabTask};

pub(super) fn read_replay(
    task: &std::path::Path,
    actions: &std::path::Path,
    seed: u64,
) -> Result<(LabTask, Vec<Action>), String> {
    let task: LabTask =
        serde_json::from_str(&std::fs::read_to_string(task).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let actions: Vec<Action> =
        serde_json::from_str(&std::fs::read_to_string(actions).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let mut env = Env::reset(task.clone(), seed).map_err(|e| e.to_string())?;
    for action in &actions {
        env.step(action.clone())
            .map_err(|e| format!("Policy replay rejected: {e}"))?;
    }
    Ok((task, actions))
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn controls(
    panel: &mut Panel,
    app: &mut EditorApp,
    ui: &mut egui::Ui,
    busy: bool,
) -> Result<(), String> {
    let mut mode = None;
    let bindings = palettes(&panel.model).unwrap_or_default();
    egui::ComboBox::from_label("Policy style palette")
        .selected_text(if panel.style.is_empty() {
            "Saved prefab assets"
        } else {
            &panel.style
        })
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut panel.style, String::new(), "Saved prefab assets");
            for name in bindings.keys() {
                ui.selectable_value(&mut panel.style, name.clone(), name);
            }
        });
    ui.add_enabled_ui(!busy, |ui| {
        ui.label("Test using the saved footprint and room program. Save scene edits first.");
        ui.horizontal_wrapped(|ui| {
            if ui.button("Build a variant").clicked() {
                mode = Some("build");
            }
            if ui.button("Test completion").clicked() {
                mode = Some("complete");
            }
            if ui.button("Test repair").clicked() {
                mode = Some("repair");
            }
        });
    });
    if let Some(mode) = mode {
        let current = app
            .prefabs
            .workshop
            .editing
            .as_ref()
            .or(app.prefabs.selected.as_ref())
            .ok_or("Open a saved prefab first")?;
        let prefab = &app
            .prefabs
            .entry(current)
            .ok_or("The saved prefab is missing")?
            .prefab;
        let mut task = osrs_lab::reconstruction_task(prefab, &|id, _| {
            app.placer
                .known_object_size(id.get())
                .map(|(w, h)| (u16::from(w), u16::from(h)))
        })?;
        task.budget = 4000;
        if !panel.style.is_empty() {
            let binding = bindings
                .get(&panel.style)
                .ok_or("The chosen style is missing from this model")?;
            let mut assets: Vec<osrs_lab::PaletteEntry> =
                serde_json::from_value(binding["palette"].clone()).map_err(|e| e.to_string())?;
            assets.extend(task.palette);
            assets.sort_by_key(|p| (p.id, p.kind));
            assets.dedup_by_key(|p| (p.id, p.kind));
            task.palette = assets;
            task.overlays.extend(
                serde_json::from_value::<Vec<u32>>(binding["overlays"].clone())
                    .map_err(|e| e.to_string())?,
            );
            task.underlays.extend(
                serde_json::from_value::<Vec<u32>>(binding["underlays"].clone())
                    .map_err(|e| e.to_string())?,
            );
        }
        if mode != "build" {
            task.initial = task.target.clone();
            task.initial.objects = task
                .initial
                .objects
                .into_iter()
                .enumerate()
                .filter_map(|(i, o)| {
                    ((mode == "repair" && i % 5 != 0) || (mode == "complete" && i % 2 == 0))
                        .then_some(o)
                })
                .collect();
        }
        for p in &task.palette {
            let name = app
                .names
                .resolve(
                    p.id,
                    app.placer.catalog_entry(p.id).map(|e| e.name.as_str()),
                )
                .text;
            task.object_names.insert(p.id, name);
        }
        if let Some(building) = &prefab.building {
            for &index in &building.semantics.locked_parts {
                if let Some(p) = prefab.objects.get(index) {
                    if !task.initial.objects.contains(p) {
                        task.initial.objects.push(p.clone());
                    }
                    task.locks.push(p.clone());
                }
            }
        }
        let value = serde_json::to_value(prefab).map_err(|e| e.to_string())?;
        let building = &value["building"];
        let footprint = building.get("floors").cloned().unwrap_or_else(|| {
            serde_json::json!(
                prefab
                    .tiles
                    .iter()
                    .map(|t| serde_json::json!({"x":t.x,"y":t.y,"plane":t.plane}))
                    .collect::<Vec<_>>()
            )
        });
        let rooms = building["semantics"]
            .get("rooms")
            .cloned()
            .unwrap_or_else(|| serde_json::json!([]));
        let goal = serde_json::json!({"width":prefab.width,"height":prefab.height,"planes":prefab.planes,"footprint":footprint,"rooms":rooms,"locks":task.locks,"style":panel.style});
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let directory = root().join(format!("previews/{nonce}"));
        std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        let task_file = directory.join("task.json");
        let goal_file = directory.join("goal.json");
        let actions = directory.join("actions.json");
        std::fs::write(
            &task_file,
            serde_json::to_vec(&task).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        std::fs::write(
            &goal_file,
            serde_json::to_vec(&goal).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        panel.start(
            vec![
                "rollout".into(),
                "--bundle".into(),
                panel.model.clone(),
                "--task".into(),
                task_file.display().to_string(),
                "--goal".into(),
                goal_file.display().to_string(),
                "--binary".into(),
                binary()?.display().to_string(),
                "--out".into(),
                actions.display().to_string(),
                "--seed".into(),
                panel.seed.to_string(),
            ],
            false,
        )?;
        panel.preview_files = Some((task_file, actions));
    }
    if panel.replay.is_some()
        && ui.button("Watch verified policy replay").clicked()
        && let Some(replay) = panel.replay.take()
    {
        app.prefabs.studio_command = Some(crate::app::prefab_studio::SceneCommand::PolicyReplay(
            Box::new(replay),
        ));
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn palettes(model: &str) -> Result<std::collections::BTreeMap<String, serde_json::Value>, String> {
    let mut root = std::path::PathBuf::from(model);
    if !root.join("bundle.json").is_file() {
        let reference = if root.join("best.json").is_file() {
            "best.json"
        } else {
            "latest.json"
        };
        let value: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(reference)).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let name = value["checkpoint"]
            .as_str()
            .ok_or("Missing checkpoint reference")?;
        if std::path::Path::new(name).components().count() != 1 {
            return Err("Invalid checkpoint reference".into());
        }
        root = root.join(name);
    }
    serde_json::from_str(
        &std::fs::read_to_string(root.join("palettes.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
