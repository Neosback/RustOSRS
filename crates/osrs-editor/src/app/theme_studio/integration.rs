//! Frame-boundary orchestration. Widgets only emit actions; EditRuntime owns commits.
use super::{Action, ThemeStudio, files};
use crate::app::{EditorApp, edit_runtime::EditExecution, editing::EditStep};
use eframe::egui;
use osrs_hd::theme::ThemePatch;
use std::task::Poll;

pub(in crate::app) fn frame(app: &mut EditorApp, ctx: &egui::Context) {
    if matches!(app.ui.themes.action, Some(Action::CancelPreparation)) {
        app.ui.themes.action = None;
        app.ui.themes.preparing = None;
        app.renderer.cancel_theme_preparation();
        app.ui.themes.settled();
        return;
    }
    if matches!(
        app.ui.themes.action,
        Some(Action::Apply(_) | Action::Import | Action::LoadLibrary(_))
    ) {
        // A newer selection supersedes the old candidate before it can activate this frame.
        app.ui.themes.preparing = None;
        app.renderer.cancel_theme_preparation();
    }
    if let Some((pack, source, revision, commit)) = app.ui.themes.preparing.take() {
        if app.project.generation() != source || app.session.edit_revision() != revision {
            app.renderer.cancel_theme_preparation();
            app.ui.themes.error =
                Some("Appearance preparation was cancelled because the project changed".into());
        } else {
            match app.renderer.poll_theme_preparation() {
                Poll::Pending => {
                    app.ui.themes.preparing = Some((pack, source, revision, commit));
                    ctx.request_repaint();
                }
                Poll::Ready(Err(error)) => app.ui.themes.error = Some(error),
                Poll::Ready(Ok(())) if !commit => match app.renderer.install_prepared_theme() {
                    Err(error) => app.ui.themes.error = Some(error),
                    Ok(()) => app.ui.themes.settled(),
                },
                Poll::Ready(Ok(())) => {
                    let result =
                        ThemePatch::plan(app.session.appearance(), pack).and_then(|patch| {
                            app.session
                                .execute_appearance(patch, |_| {
                                    app.renderer.install_prepared_theme()
                                })
                                .map_err(|e| format!("{e:?}"))
                        });
                    settle(app, result);
                }
            }
        }
    }
    if app.ui.themes.preparing.is_none()
        && app.ui.themes.action.is_none()
        && app
            .renderer
            .theme_needs_restore(&app.session.appearance().definition.shaders)
    {
        let pack = app.session.appearance().clone();
        match app.renderer.prepare_theme(&pack.definition.shaders) {
            Ok(()) => {
                app.ui.themes.preparing = Some((
                    pack,
                    app.project.generation(),
                    app.session.edit_revision(),
                    false,
                ));
                ctx.request_repaint();
            }
            Err(error) => app.ui.themes.error = Some(error),
        }
    }
    let Some(action) = app.ui.themes.action.take() else {
        return;
    };
    let result = match action {
        Action::CancelPreparation => Ok(()),
        Action::Apply(pack) => begin(app, *pack),
        Action::Import => files::import().and_then(|pack| match pack {
            Some(pack) => begin(app, pack),
            None => Ok(()),
        }),
        Action::Export => files::export(app.session.appearance()),
        Action::SaveLibrary => files::save_library(app.session.appearance()).map(|()| {
            app.ui.themes.library = files::load_library();
        }),
        Action::LoadLibrary(index) => match app.ui.themes.library.get(index).cloned() {
            Some(pack) => begin(app, pack),
            None => Err("Theme library selection is no longer available".into()),
        },
    };
    if let Err(error) = result {
        app.ui.themes.error = Some(error);
    }
}
fn begin(app: &mut EditorApp, mut pack: osrs_hd::theme::ThemePack) -> Result<(), String> {
    crate::session::validate_appearance_pack(&pack)?;
    if pack.definition.id == "stock-117hd" && pack != osrs_hd::theme::ThemePack::default() {
        pack.definition = pack
            .definition
            .duplicate(super::custom_id(), pack.definition.name.clone());
    }
    let previous = &app.session.appearance().definition;
    if pack.definition.id == previous.id && pack != *app.session.appearance() {
        pack.definition.revision = previous
            .revision
            .checked_add(1)
            .ok_or("Theme revision limit reached")?;
    }
    app.ui.themes.error = None;
    let patch = ThemePatch::plan(app.session.appearance(), pack.clone())?;
    if patch.is_empty() {
        if !app.renderer.theme_shaders_match(&pack.definition.shaders) {
            app.renderer.prepare_theme(&pack.definition.shaders)?;
            app.ui.themes.preparing = Some((
                pack,
                app.project.generation(),
                app.session.edit_revision(),
                false,
            ));
            return Ok(());
        }
        app.settings.prefs.render_profile = crate::settings::ProfileChoice::Hd;
        app.ui.themes.settled();
        return Ok(());
    }
    if pack.definition.shaders != app.session.appearance().definition.shaders {
        app.renderer.prepare_theme(&pack.definition.shaders)?;
        app.ui.themes.preparing = Some((
            pack,
            app.project.generation(),
            app.session.edit_revision(),
            true,
        ));
        return Ok(());
    }
    let result = app
        .session
        .execute_appearance(patch, |_| Ok::<_, String>(()))
        .map_err(|e| format!("{e:?}"));
    settle(app, result);
    Ok(())
}
fn settle(app: &mut EditorApp, result: Result<EditExecution<()>, String>) {
    match result {
        Ok(EditExecution::Committed { .. }) => {
            app.push_step(EditStep::appearance());
            app.settings.prefs.render_profile = crate::settings::ProfileChoice::Hd;
            app.ui.themes.settled();
        }
        Ok(EditExecution::NoChange) => app.ui.themes.settled(),
        Err(error) => app.ui.themes.error = Some(error),
    }
}

pub(in crate::app) fn panel(app: &mut EditorApp, ui: &mut egui::Ui) {
    let origin = app.renderer.render_origin();
    let camera = app.interaction.camera();
    let pivot = if matches!(
        camera.mode,
        crate::camera::CameraMode::Orbit | crate::camera::CameraMode::Game
    ) {
        camera.pivot()
    } else {
        [camera.x, camera.y, camera.z]
    };
    let tile = osrs_core::coords::MapTile::new(
        ((f64::from(origin.0) + f64::from(pivot[0])) / 128.0).floor() as i32,
        ((f64::from(origin.1) + f64::from(pivot[2])) / 128.0).floor() as i32,
    );
    let plane = osrs_core::coords::PlaneIndex::new(app.interaction.view_plane())
        .unwrap_or_else(|| unreachable!("validated plane"));
    app.ui
        .themes
        .draw(ui, app.session.appearance(), tile, plane);
}

impl ThemeStudio {
    pub(in crate::app) fn values(
        &self,
        current: &osrs_hd::theme::ThemePack,
        query: &osrs_hd::theme::AppearanceQuery,
    ) -> osrs_hd::theme::ThemeOverrides {
        self.preview()
            .unwrap_or(current)
            .definition
            .resolve(&Default::default(), query)
            .values
    }
}
