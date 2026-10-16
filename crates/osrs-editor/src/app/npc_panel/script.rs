//! `npc ...` script commands: the headless way to drive the same code the panel uses.
//!
//! `new`, `folder PATH`, `preview FILTER|all`, `apply`, `place ID X Y [PLANE]`, `select N`,
//! `play|pause|step|restart`, `move X Y [route]`, `export`, `save PATH`, `info`.

use super::scenario::{apply_import, new_scenario, plan_and_commit, preview_export};
use super::{SIDECAR_FORMAT, Sidecar, actor_location};
use crate::app::{EditorApp, npc_runtime::Arm};
use osrs_core::{ids::NpcId, scenario::NpcSpawnId};
use osrs_edit::{SpawnDraft, plan_add_spawn};
use osrs_playtest::runtime::{MoveSpeed, RuntimeCommand};
use std::str::SplitWhitespace;

fn number<T: std::str::FromStr>(word: Option<&str>, what: &str) -> Result<T, String> {
    word.ok_or_else(|| format!("{what} is missing"))?
        .parse()
        .map_err(|_| format!("{what} is not a number"))
}

/// Returns `Ok(true)` when finished and `Ok(false)` when it should be retried next frame.
pub(in crate::app) fn command(
    app: &mut EditorApp,
    words: &mut SplitWhitespace<'_>,
) -> Result<bool, String> {
    match words.next().unwrap_or_default() {
        verb @ ("on" | "off") => app.settings.prefs.npc.spawns = verb == "on",
        "new" => {
            if app.npcs.source().is_none() {
                return Ok(false);
            }
            new_scenario(app);
        }
        #[cfg(not(target_arch = "wasm32"))]
        "folder" => {
            let path: Vec<&str> = words.collect();
            if app.npcs.installation.is_none() && app.npcs.import_job.is_none() {
                app.npcs.import_job = Some(crate::app::npc_runtime::read_installation(
                    path.join(" ").into(),
                ));
            }
            return Ok(app.npcs.installation.is_some());
        }
        #[cfg(not(target_arch = "wasm32"))]
        "preview" => {
            let filter = words.next().unwrap_or("all").to_owned();
            if app.npcs.preview.is_none() && app.npcs.import_job.is_none() {
                let Some(data) = &app.npcs.installation else {
                    return Err("read an installation first (npc folder PATH)".into());
                };
                let chosen = (filter != "all").then(|| {
                    data.spawn_files
                        .iter()
                        .enumerate()
                        .filter(|(_, f)| f.path.contains(&filter))
                        .map(|(i, _)| i)
                        .collect::<Vec<_>>()
                });
                app.npcs.import_job = Some(crate::app::npc_runtime::preview_import(data, chosen));
            }
            if let Some(preview) = &app.npcs.preview {
                eprintln!(
                    "smoke: npc preview: {} files, {} spawns, {} warnings, {} errors",
                    preview.files,
                    preview.report.spawns.len(),
                    preview.report.warnings.len(),
                    preview.report.errors.len()
                );
                return Ok(true);
            }
            return Ok(false);
        }
        "apply" => {
            if app.npcs.catalog().is_empty() {
                return Ok(false);
            }
            apply_import(app);
            eprintln!("smoke: npc apply: {:?}", app.npcs.notice);
        }
        "place" => {
            let npc = NpcId::new(number(words.next(), "npc id")?);
            let x: i32 = number(words.next(), "x")?;
            let y: i32 = number(words.next(), "y")?;
            let plane = words
                .next()
                .map_or(Ok(0), |w| number::<u8>(Some(w), "plane"))?;
            let location = actor_location((x, y), plane).ok_or("invalid plane")?;
            if app.session.scenario().definition().is_none() {
                return Ok(false);
            }
            let symbol = app
                .npcs
                .installation
                .as_ref()
                .and_then(|d| d.npc_symbols.get(&npc.get()).cloned())
                .unwrap_or_default();
            let mut placed = None;
            plan_and_commit(app, |doc| {
                plan_add_spawn(doc, SpawnDraft::new(npc, symbol, location)).map(|(patch, id)| {
                    placed = Some(id);
                    patch
                })
            });
            app.npcs.selection = placed;
        }
        "select" => app.npcs.selection = Some(NpcSpawnId(number(words.next(), "spawn id")?)),
        verb @ ("play" | "pause" | "step" | "restart") => {
            if !app.npcs.has_runtime() {
                return Ok(false);
            }
            app.npcs.queue(match verb {
                "play" => RuntimeCommand::Start,
                "pause" => RuntimeCommand::Pause,
                "step" => RuntimeCommand::SingleStep,
                _ => RuntimeCommand::Restart,
            });
        }
        "move" => {
            let x: i32 = number(words.next(), "x")?;
            let y: i32 = number(words.next(), "y")?;
            let route = words.next() == Some("route");
            let Some(actor) = app.npcs.selection else {
                return Err("no NPC selected".into());
            };
            let plane = app.interaction.view_plane();
            let destination = actor_location((x, y), plane).ok_or("invalid plane")?;
            if !app.npcs.has_runtime() {
                return Ok(false);
            }
            app.npcs.queue(RuntimeCommand::Move {
                actor,
                destination,
                route,
                speed: MoveSpeed::Walk,
            });
        }
        "export" => {
            preview_export(app);
            if let Some(preview) = &app.npcs.export_preview {
                eprintln!(
                    "smoke: npc export: error={:?} spawns_lines={} behavior_lines={}",
                    preview.error,
                    preview.spawns_toml.lines().count(),
                    preview.behaviors_toml.lines().count()
                );
            }
        }
        "save" => {
            let path: Vec<&str> = words.collect();
            let Some(definition) = app.session.scenario().definition() else {
                return Err("no spawns loaded".into());
            };
            let text = serde_json::to_string_pretty(&Sidecar {
                format: SIDECAR_FORMAT.into(),
                scenario: definition.clone(),
            })
            .map_err(|e| e.to_string())?;
            std::fs::write(path.join(" "), text).map_err(|e| e.to_string())?;
        }
        "info" => {
            let spawns = app
                .session
                .scenario()
                .definition()
                .map_or(0, |d| d.spawns.len());
            let selected = app.npcs.selection.and_then(|id| app.npcs.actor(id));
            eprintln!(
                "smoke: npc info: spawns={spawns} runtime={} running={} stamp={:?} stale={} selected={:?} diagnostics={:?} arm={:?}",
                app.npcs.has_runtime(),
                app.npcs.running(),
                app.npcs.stamp(),
                app.npcs.stale_rejected(),
                selected.map(|a| (a.location, a.status)),
                app.npcs.diagnostics(),
                matches!(app.npcs.arm, Arm::None),
            );
        }
        other => return Err(format!("unknown npc command '{other}'")),
    }
    Ok(true)
}
