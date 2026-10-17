//! `prefab capture|place|build|page|list`: the prefab library from a script (smoke tests).

use crate::app::{EditorApp, prefab_runtime::Armed};
use osrs_core::coords::{MapTile, SourcePlane};
use osrs_edit::CaptureSpec;
use std::str::SplitWhitespace;

/// A name in a script has no spaces: underscores stand for them.
fn spoken(name: &str) -> String {
    name.replace('_', " ")
}

pub(super) fn run(app: &mut EditorApp, words: &mut SplitWhitespace<'_>) -> Result<(), String> {
    match words.next().unwrap_or_default() {
        "capture" => {
            let name = spoken(words.next().ok_or("a name is missing")?);
            let x0 = super::number::<i32>(words.next(), "x0")?;
            let y0 = super::number::<i32>(words.next(), "y0")?;
            let x1 = super::number::<i32>(words.next(), "x1")?;
            let y1 = super::number::<i32>(words.next(), "y1")?;
            let planes = words
                .next()
                .map_or(Ok(1), |w| super::number::<u8>(Some(w), "planes"))?;
            let width = u16::try_from(x1.max(x0) - x1.min(x0) + 1).map_err(|e| e.to_string())?;
            let height = u16::try_from(y1.max(y0) - y1.min(y0) + 1).map_err(|e| e.to_string())?;
            let base_plane = SourcePlane::new(app.interaction.view_plane())
                .ok_or("the viewed plane is invalid")?;
            let spec = CaptureSpec {
                origin: MapTile::new(x0.min(x1), y0.min(y1)),
                width,
                height,
                base_plane,
                planes,
                terrain: true,
                heights: true,
                objects: true,
            };
            let file = app.prefabs.capture(app.session.document(), spec, &name)?;
            let summary = app.prefabs.entry(&file).map(|e| e.prefab.summary());
            eprintln!("smoke: captured {file}: {summary:?}");
        }
        "place" => {
            let name = spoken(words.next().ok_or("a name is missing")?);
            let x = super::number::<i32>(words.next(), "x")?;
            let y = super::number::<i32>(words.next(), "y")?;
            let turns = words
                .next()
                .map_or(Ok(0), |w| super::number::<u8>(Some(w), "turns"))?;
            let replace = words.next() == Some("replace");
            let file = app
                .prefabs
                .entries()
                .iter()
                .find(|entry| entry.prefab.name == name)
                .map(|entry| entry.file.clone())
                .ok_or_else(|| format!("no prefab named {name:?}"))?;
            let armed = Armed {
                file,
                turns: turns & 3,
                terrain: true,
                heights: true,
                objects: true,
                replace_objects: replace,
            };
            let committed = crate::app::prefab_panel::place(app, &armed, (x, y));
            eprintln!("smoke: place {name:?} at ({x},{y}) turns {turns}: committed {committed}");
        }
        "build" => {
            let x = super::number::<i32>(words.next(), "x")?;
            let y = super::number::<i32>(words.next(), "y")?;
            let width = super::number::<u16>(words.next(), "width")?;
            let height = super::number::<u16>(words.next(), "height")?;
            let seed = super::number::<u64>(words.next(), "seed")?;
            let apply = words.next() == Some("apply");
            let build = &mut app.prefabs.build;
            build.form.width = width;
            build.form.height = height;
            build.form.seed = seed;
            build.centre_site_on((x, y), app.interaction.view_plane());
            let revision = app.session.map_revision();
            app.prefabs.build.refresh(app.session.document(), revision);
            match (&app.prefabs.build.preview, &app.prefabs.build.error) {
                (Some(preview), _) => {
                    eprintln!(
                        "smoke: building: {} objects, {} removed",
                        preview.objects, preview.removed
                    );
                    let mut by_plane = std::collections::BTreeMap::<(u8, u8), usize>::new();
                    for op in preview.building.ops() {
                        if let osrs_edit::MapOp::AddObject { placement } = op {
                            *by_plane
                                .entry((
                                    placement.position.plane.index().get(),
                                    placement.kind.get(),
                                ))
                                .or_default() += 1;
                        }
                    }
                    eprintln!("smoke: building (plane, kind): count {by_plane:?}");
                    if apply {
                        let patch = preview.apply.clone();
                        let committed =
                            crate::app::editing::commit_map_patch(app, patch, "Generated building");
                        eprintln!(
                            "smoke: building applied: {committed} ({:?})",
                            app.ui.notice()
                        );
                    }
                }
                (None, error) => eprintln!("smoke: building failed: {error:?}"),
            }
        }
        "page" => {
            app.prefabs.page = match words.next() {
                Some("build") => crate::app::prefab_runtime::Page::Build,
                Some("library") => crate::app::prefab_runtime::Page::Library,
                other => return Err(format!("unknown page {other:?}")),
            };
        }
        "list" => {
            for entry in app.prefabs.entries() {
                eprintln!(
                    "smoke: prefab {:?}: {}",
                    entry.prefab.name,
                    entry.prefab.summary()
                );
            }
        }
        other => return Err(format!("unknown prefab command {other:?}")),
    }
    Ok(())
}
