//! UI choices and cross-runtime effects for native project/source replacement.
//! Save/quit/replacement policy is owned by `ProjectRuntime`, not by these dialogs.

use crate::app::EditorApp;
use crate::app::project_runtime::{LifecycleChoice, LifecycleTransition};
use eframe::egui;
use osrs_cache::source::CacheSource;
use std::path::PathBuf;

impl EditorApp {
    pub(in crate::app) fn request_source_change(
        &mut self,
        source: CacheSource,
        label: PathBuf,
        ctx: &egui::Context,
    ) {
        if self.project.save_running() {
            self.ui
                .notify("Finish the current project save before switching caches.".to_owned());
            return;
        }
        self.request_transition(LifecycleTransition::Source { source, label }, ctx);
    }

    pub(super) fn request_transition(
        &mut self,
        transition: LifecycleTransition,
        ctx: &egui::Context,
    ) {
        if let Some(transition) = self.project.stage_transition(transition, self.is_unsaved()) {
            self.execute_transition(transition, ctx);
        }
    }

    fn execute_transition(&mut self, transition: LifecycleTransition, ctx: &egui::Context) {
        match transition {
            LifecycleTransition::Source { source, label } => {
                self.project.reset_for_source_change();
                self.session.reset();
                self.open_source(source, label, ctx);
            }
            LifecycleTransition::Project {
                path,
                diffs,
                appearance,
            } => {
                let count = diffs.len();
                self.activate_project(path, diffs, *appearance);
                self.ui.notify(format!(
                    "Opening project: {count} edited region{}",
                    super::plural(count)
                ));
            }
        }
    }

    fn transition_guard(&mut self, ctx: &egui::Context) -> bool {
        if let Some(transition) = self.project.ready_transition(self.is_unsaved()) {
            self.execute_transition(transition, ctx);
            return false;
        }
        let Some(action) = self.project.transition_description() else {
            return false;
        };
        let choice = lifecycle_dialog(
            ctx,
            "replace-unsaved",
            420.0,
            &format!("Your map edits have not been saved. Save them before you {action}?"),
            "Save",
            "Discard",
        );
        if let Some(choice) = choice {
            if let Some(transition) = self.project.choose_transition(choice) {
                self.execute_transition(transition, ctx);
            }
            if matches!(choice, LifecycleChoice::Save) {
                let started = self.save_project(false);
                self.project.wait_transition_save(started);
            }
        }
        true
    }

    pub(in crate::app) fn quit_guard(&mut self, ctx: &egui::Context) {
        let smoke = self.diagnostics.smoke_active();
        if !smoke
            && ctx.input(|i| i.viewport().close_requested())
            && self.is_unsaved()
            && !self.project.discard_confirmed()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.project.request_quit();
        }
        if self.project.ready_to_close(self.is_unsaved()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if !self.project.quit_prompt() && self.transition_guard(ctx) {
            return;
        }
        if !self.project.quit_prompt() {
            return;
        }
        let choice = lifecycle_dialog(
            ctx,
            "quit-unsaved",
            380.0,
            "Your map edits have not been saved to a project.",
            "Save and quit",
            "Quit without saving",
        );
        if let Some(choice) = choice {
            if self.project.choose_quit(choice) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            if matches!(choice, LifecycleChoice::Save) {
                let started = self.project.cache_build().is_some()
                    && !self.project.save_running()
                    && self.save_project(false);
                self.project.wait_quit_save(started);
            }
        }
    }
}

/// Drawing only emits a choice; no project, edit, worker or render state is mutable here.
fn lifecycle_dialog(
    ctx: &egui::Context,
    id: &'static str,
    width: f32,
    text: &str,
    save: &str,
    discard: &str,
) -> Option<LifecycleChoice> {
    let (choice, closed) = crate::app::modal::Modal::new(id, "Unsaved changes")
        .size(width, 0.0)
        .show(ctx, |ui| {
            ui.label(text);
            ui.add_space(8.0);
            let mut choice = None;
            ui.horizontal(|ui| {
                if ui.button(save).clicked() {
                    choice = Some(LifecycleChoice::Save);
                }
                if ui.button(discard).clicked() {
                    choice = Some(LifecycleChoice::Discard);
                }
                if ui.button("Cancel").clicked() {
                    choice = Some(LifecycleChoice::Cancel);
                }
            });
            choice
        });
    choice.or_else(|| closed.then_some(LifecycleChoice::Cancel))
}
