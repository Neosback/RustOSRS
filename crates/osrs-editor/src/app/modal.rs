//! Every window is a modal: the rest of the editor is dimmed and cannot be used until it is closed.
//!
//! One helper draws them all, so they look and behave the same: a title row with a close button, a
//! dimmed backdrop that swallows clicks and scrolls, Esc to close (only the topmost one, and not
//! while a combo box or menu is open).

use super::EditorApp;
use crate::icons;
use eframe::egui;

/// What a modal asks of its owner.
pub struct Modal {
    pub id: &'static str,
    pub title: String,
    /// Content size in points; 0 on an axis means as big as the content is.
    pub size: [f32; 2],
    /// Whether the title row has a close button (the cache welcome card has none).
    pub closable: bool,
    /// Whether Esc closes it (off while a key binding is being captured).
    pub esc_closes: bool,
}

impl Modal {
    pub fn new(id: &'static str, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            size: [0.0, 0.0],
            closable: true,
            esc_closes: true,
        }
    }

    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.size = [width, height];
        self
    }

    pub fn closable(mut self, closable: bool) -> Self {
        self.closable = closable;
        self
    }

    pub fn esc_closes(mut self, esc: bool) -> Self {
        self.esc_closes = esc;
        self
    }

    /// Draw it. Returns the body's result and whether the user asked to close it.
    pub fn show<R>(self, ctx: &egui::Context, body: impl FnOnce(&mut egui::Ui) -> R) -> (R, bool) {
        let screen = ctx.content_rect().size();
        let [width, height] = self.size;
        let mut close = false;
        let response = egui::Modal::new(egui::Id::new(self.id))
            .backdrop_color(egui::Color32::from_black_alpha(150))
            .show(ctx, |ui| {
                if width > 0.0 {
                    ui.set_width(width.min(screen.x - 48.0));
                }
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&self.title).strong().size(16.0));
                    if self.closable {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add(egui::Button::new(icons::CLOSE).frame(false))
                                .on_hover_text("Close (Esc)")
                                .clicked()
                            {
                                close = true;
                            }
                        });
                    }
                });
                ui.separator();
                if height > 0.0 {
                    // A child of a fixed size, so a scroll area inside it has bounds to fill.
                    let size = egui::vec2(ui.available_width(), height.min(screen.y - 120.0));
                    ui.allocate_ui(size, |ui| {
                        ui.set_min_size(size);
                        body(ui)
                    })
                    .inner
                } else {
                    body(ui)
                }
            });
        if self.closable
            && self.esc_closes
            && response.is_top_modal
            && !response.any_popup_open
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            close = true;
        }
        (response.inner, close)
    }
}

impl EditorApp {
    /// Whether a modal window is up (the editor behind it takes no keys or tools).
    pub(super) fn modal_open(&self) -> bool {
        self.settings_open
            || self.script_open
            || self.plugins_open
            || self.tool_draft.is_some()
            || self.teleport.open
            || self.world_map.open
            || self.cache_error.is_some()
    }

    /// Whether the editor's own key bindings should stay quiet: a text field has focus, a binding
    /// is being captured or a modal is up.
    pub(super) fn keys_blocked(&self, ctx: &egui::Context) -> bool {
        ctx.egui_wants_keyboard_input() || self.capturing_binding.is_some() || self.modal_open()
    }
}
