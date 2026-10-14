//! Source/project replacement policy and unsaved-work guards.

use super::{ProjectState, plural};
use crate::app::EditorApp;
use crate::session::RegionDiff;
use eframe::egui;
use osrs_cache::source::CacheSource;
use std::path::PathBuf;
use web_time::Instant;

pub(super) enum LifecycleTransition {
    Source {
        source: CacheSource,
        label: PathBuf,
    },
    Project {
        path: PathBuf,
        diffs: Vec<RegionDiff>,
    },
}

impl LifecycleTransition {
    fn description(&self) -> &'static str {
        match self {
            Self::Source { .. } => "open another cache",
            Self::Project { .. } => "open another project",
        }
    }
}

impl ProjectState {
    /// Clear every piece of state whose meaning is tied to the current cache/source.
    ///
    /// Source replacement is deliberately blocked while a save is running, so dropping the
    /// receiver here would be a lifecycle bug rather than a cancellation mechanism.
    pub(super) fn reset_for_source_change(&mut self) {
        debug_assert!(self.save.is_none());
        *self = Self::default();
    }
}

impl EditorApp {
    /// Route a cache/source replacement through the same unsaved-work lifecycle as project open.
    pub(in crate::app) fn request_source_change(
        &mut self,
        source: CacheSource,
        label: PathBuf,
        ctx: &egui::Context,
    ) {
        if self.project.save.is_some() {
            self.notice = Some((
                "Finish the current project save before switching caches.".to_owned(),
                Instant::now(),
            ));
            return;
        }
        self.request_transition(LifecycleTransition::Source { source, label }, ctx);
    }

    pub(super) fn request_transition(
        &mut self,
        transition: LifecycleTransition,
        ctx: &egui::Context,
    ) {
        if self.is_unsaved() {
            self.project.transition = Some(transition);
            self.project.transition_prompt = true;
        } else {
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
            LifecycleTransition::Project { path, diffs } => {
                let count = diffs.len();
                self.activate_project(path, diffs);
                self.notice = Some((
                    format!("Opening project: {count} edited region{}", plural(count)),
                    Instant::now(),
                ));
            }
        }
    }

    /// Show and resolve Save / Discard / Cancel for cache/project replacement.
    /// Returns true while the lifecycle prompt owns the frame's modal interaction.
    fn transition_guard(&mut self, ctx: &egui::Context) -> bool {
        if self.project.transition_after_save && self.project.save.is_none() {
            self.project.transition_after_save = false;
            if !self.is_unsaved() {
                if let Some(transition) = self.project.transition.take() {
                    self.execute_transition(transition, ctx);
                }
                self.project.transition_prompt = false;
                return false;
            }
            self.project.transition_prompt = true;
        }

        if !self.project.transition_prompt {
            return false;
        }
        let action = self
            .project
            .transition
            .as_ref()
            .map(LifecycleTransition::description)
            .unwrap_or("replace the current session");
        let (choice, closed) =
            super::super::modal::Modal::new("replace-unsaved", "Unsaved changes")
                .size(420.0, 0.0)
                .show(ctx, |ui| {
                    ui.label(format!(
                        "Your map edits have not been saved. Save them before you {action}?"
                    ));
                    ui.add_space(8.0);
                    let mut choice = None;
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() {
                            choice = Some(TransitionChoice::Save);
                        }
                        if ui.button("Discard").clicked() {
                            choice = Some(TransitionChoice::Discard);
                        }
                        if ui.button("Cancel").clicked() {
                            choice = Some(TransitionChoice::Cancel);
                        }
                    });
                    choice
                });
        match choice {
            Some(TransitionChoice::Save) => {
                self.project.transition_prompt = false;
                if self.save_project(false) {
                    self.project.transition_after_save = true;
                } else {
                    self.project.transition_prompt = true;
                }
            }
            Some(TransitionChoice::Discard) => {
                self.project.transition_prompt = false;
                if let Some(transition) = self.project.transition.take() {
                    self.execute_transition(transition, ctx);
                }
            }
            Some(TransitionChoice::Cancel) => {
                self.project.transition_prompt = false;
                self.project.transition_after_save = false;
                self.project.transition = None;
            }
            None if closed => {
                self.project.transition_prompt = false;
                self.project.transition_after_save = false;
                self.project.transition = None;
            }
            None => {}
        }
        true
    }

    /// Intercept closing the window while edits are unsaved, and finish a save-then-quit.
    pub(in crate::app) fn quit_guard(&mut self, ctx: &egui::Context) {
        let smoke = self.exit_after_seconds.is_some() || self.screenshot.is_some();
        if !smoke
            && ctx.input(|i| i.viewport().close_requested())
            && self.is_unsaved()
            && !self.project.discard_confirmed
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            // Closing the application supersedes an in-progress source/project replacement choice.
            self.project.transition = None;
            self.project.transition_prompt = false;
            self.project.transition_after_save = false;
            self.project.quit_prompt = true;
        }
        if self.project.close_when_saved && self.project.save.is_none() {
            self.project.close_when_saved = false;
            if !self.is_unsaved() {
                self.project.discard_confirmed = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        if !self.project.quit_prompt && self.transition_guard(ctx) {
            return;
        }
        if !self.project.quit_prompt {
            return;
        }
        let (choice, closed) = super::super::modal::Modal::new("quit-unsaved", "Unsaved changes")
            .size(380.0, 0.0)
            .show(ctx, |ui| {
                ui.label("Your map edits have not been saved to a project.");
                ui.add_space(8.0);
                let mut choice = None;
                ui.horizontal(|ui| {
                    if ui.button("Save and quit").clicked() {
                        choice = Some(QuitChoice::Save);
                    }
                    if ui.button("Quit without saving").clicked() {
                        choice = Some(QuitChoice::Discard);
                    }
                    if ui.button("Cancel").clicked() {
                        choice = Some(QuitChoice::Cancel);
                    }
                });
                choice
            });
        match choice {
            Some(QuitChoice::Save) => {
                self.project.quit_prompt = false;
                if self.cache_build.is_some()
                    && self.project.save.is_none()
                    && self.save_project(false)
                {
                    self.project.close_when_saved = true;
                }
            }
            Some(QuitChoice::Discard) => {
                self.project.quit_prompt = false;
                self.project.discard_confirmed = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Some(QuitChoice::Cancel) => self.project.quit_prompt = false,
            None if closed => self.project.quit_prompt = false,
            None => {}
        }
    }
}

#[derive(Clone, Copy)]
enum QuitChoice {
    Save,
    Discard,
    Cancel,
}

#[derive(Clone, Copy)]
enum TransitionChoice {
    Save,
    Discard,
    Cancel,
}
