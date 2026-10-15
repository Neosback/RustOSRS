//! The Objects tool's drawer panel.

use super::*;

/// A captioned cluster of drawer controls.
pub(super) fn group(ui: &mut egui::Ui, caption: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.vertical(|ui| {
        ui.label(
            egui::RichText::new(caption.to_uppercase())
                .size(10.5)
                .strong()
                .color(ui.visuals().weak_text_color()),
        );
        add(ui);
    });
    ui.add_space(10.0);
}

impl EditorApp {
    /// The Objects tool's drawer: what it can do to the selected objects, in groups.
    pub(in crate::app) fn object_drawer(&mut self, ui: &mut egui::Ui) {
        let count = self
            .interaction
            .selection_objects()
            .len()
            .max(usize::from(self.interaction.selected().is_some()));
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("Objects").strong().size(15.0));
            ui.label(match count {
                0 => "Click an object (Shift adds, drag a box for several)".to_owned(),
                1 => "1 selected. Drag its arrows to move it, its ring to turn it.".to_owned(),
                n => format!("{n} selected. Drag the arrows to move them all."),
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                self.drawer_collapse(ui);
            });
        });
        ui.add_space(6.0);
        let any = count > 0;
        let key = |action: &str| {
            self.settings
                .keymap
                .binding(action)
                .map(|b| b.describe())
                .unwrap_or_default()
        };
        let (rotate_key, duplicate_key, delete_key) = (
            key("object.rotate"),
            key("object.duplicate"),
            key("object.delete"),
        );
        let mut step = None;
        let mut floor = 0;
        let mut turn = 0;
        let (mut copy, mut delete) = (false, false);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(5.0, 5.0);
            ui.spacing_mut().button_padding = egui::vec2(10.0, 7.0);
            group(ui, "Place object", |ui| self.place_group(ui));
            group(ui, "Move one tile", |ui| {
                ui.horizontal(|ui| {
                    for (icon, tip, delta) in [
                        (ph::ARROW_LEFT, "West", (-1, 0)),
                        (ph::ARROW_UP, "North", (0, 1)),
                        (ph::ARROW_DOWN, "South", (0, -1)),
                        (ph::ARROW_RIGHT, "East", (1, 0)),
                    ] {
                        if ui
                            .add_enabled(any, egui::Button::new(icon))
                            .on_hover_text(format!("Move one tile {tip}"))
                            .clicked()
                        {
                            step = Some(delta);
                        }
                    }
                });
            });
            group(ui, "Floor", |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(any, egui::Button::new(format!("{}  Up", ph::ARROW_LINE_UP)))
                        .on_hover_text("Move up one floor (the one above, z + 1)")
                        .clicked()
                    {
                        floor = 1;
                    }
                    if ui
                        .add_enabled(
                            any,
                            egui::Button::new(format!("{}  Down", ph::ARROW_LINE_DOWN)),
                        )
                        .on_hover_text("Move down one floor (z - 1)")
                        .clicked()
                    {
                        floor = -1;
                    }
                });
            });
            group(ui, "Turn", |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(any, egui::Button::new(ph::ARROW_COUNTER_CLOCKWISE))
                        .on_hover_text("Quarter turn anticlockwise (Shift + the turn key)")
                        .clicked()
                    {
                        turn = -1;
                    }
                    if ui
                        .add_enabled(any, egui::Button::new(icons::ROTATE))
                        .on_hover_text(format!("Quarter turn clockwise ({rotate_key})"))
                        .clicked()
                    {
                        turn = 1;
                    }
                });
            });
            group(ui, "Edit", |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            any,
                            egui::Button::new(format!("{}  Copy", icons::DUPLICATE)),
                        )
                        .on_hover_text(format!(
                            "Copy one tile east ({duplicate_key}); then drag the copy"
                        ))
                        .clicked()
                    {
                        copy = true;
                    }
                    let danger = ui.visuals().error_fg_color;
                    if ui
                        .add_enabled(
                            any,
                            egui::Button::new(
                                egui::RichText::new(format!("{}  Delete", icons::DELETE))
                                    .color(danger),
                            ),
                        )
                        .on_hover_text(format!("Delete ({delete_key})"))
                        .clicked()
                    {
                        delete = true;
                    }
                });
            });
        });
        ui.add_space(2.0);
        ui.weak(
            "Objects only move across the ground (the green square on the gizmo) and between \
             floors; the map has no height setting for them. Ctrl+Z undoes any of these.",
        );
        if let Some(delta) = step {
            let specs = self.selected_specs();
            self.move_objects(specs, delta);
        }
        if floor != 0 {
            self.shift_floor(floor);
        }
        if turn != 0 {
            self.rotate_selected_objects(turn);
        }
        if copy {
            self.duplicate_selected_objects();
        }
        if delete {
            self.delete_selected_objects();
        }
    }
}
