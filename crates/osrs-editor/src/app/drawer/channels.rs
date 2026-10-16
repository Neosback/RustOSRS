//! Per-channel option panels: overlay shape, height, flags, and the Height tool's settings.

use super::*;

impl EditorApp {
    pub(super) fn shape_picker(&mut self, ui: &mut egui::Ui) {
        // Fixed, contrasting colours: the thumbnails show geometry, not the chosen floor.
        let under = Color32::from_rgb(58, 98, 66);
        let over = OverlayFill::Color(Color32::from_rgb(214, 196, 150));
        let (mut shape, mut rotation) = (
            self.settings.prefs.brush.shape.shape,
            self.settings.prefs.brush.shape.rotation,
        );
        let mut picked = false;
        ui.horizontal_wrapped(|ui| {
            ui.vertical(|ui| {
                ui.weak("Shape");
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::splat(5.0);
                    ui.set_max_width(ui.available_width().min(330.0));
                    for id in 0..=MAX_SHAPE {
                        if swatch(ui, 40.0, shape == id, |painter, rect| {
                            paint_tile(painter, rect, id, rotation, Some(under), Some(over));
                        })
                        .on_hover_text(if id == 0 {
                            "Shape 0: full tile".to_owned()
                        } else {
                            format!("Shape {id}")
                        })
                        .clicked()
                        {
                            shape = id;
                            picked = true;
                        }
                    }
                });
            });
            ui.add_space(16.0);
            ui.vertical(|ui| {
                ui.weak("Rotation");
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::splat(5.0);
                    for (index, name) in ["North", "East", "South", "West"].into_iter().enumerate()
                    {
                        let index = index as u8;
                        ui.vertical(|ui| {
                            if swatch(ui, 40.0, rotation == index, |painter, rect| {
                                paint_tile(
                                    painter,
                                    rect,
                                    if shape == 0 { 3 } else { shape },
                                    index,
                                    Some(under),
                                    Some(over),
                                );
                            })
                            .clicked()
                            {
                                rotation = index;
                                picked = true;
                            }
                            ui.label(egui::RichText::new(name).size(11.0));
                        });
                    }
                });
            });
        });

        self.settings.prefs.brush.shape.shape = shape;
        self.settings.prefs.brush.shape.rotation = rotation;
        if picked {
            self.settings.prefs.brush.shape.armed = true;
        }
    }

    /// The Paint tool's height channel: one height for every tile the brush paints.
    pub(super) fn height_channel(&mut self, ui: &mut egui::Ui) {
        let channel = &mut self.settings.prefs.brush.height;
        let mut value = elevation(channel.value);
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.weak("Height");
            let slider = ui.add(
                egui::Slider::new(&mut value, 0..=MAX_ELEVATION)
                    .step_by(8.0)
                    .show_value(false),
            );
            let field = ui.add(
                egui::DragValue::new(&mut value)
                    .range(0..=MAX_ELEVATION)
                    .speed(1.0),
            );
            if slider.changed() || field.changed() {
                channel.value = height_for_elevation(value);
                channel.armed = true;
            }
            if ui.button("Sea level").clicked() {
                channel.value = 0;
                channel.armed = true;
            }
        });
        ui.add_space(4.0);
        ui.weak("Every tile the brush paints gets this height. Untick the channel to keep each tile's own.");
    }

    /// The Paint tool's mask channel: the flags every painted tile gets.
    pub(super) fn flag_options(&mut self, ui: &mut egui::Ui) {
        let flags = &mut self.settings.prefs.brush.flags;
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            for (bit, name, tip) in FLAG_BITS {
                let mut on = flags.bits & bit != 0;
                if ui.checkbox(&mut on, name).on_hover_text(tip).changed() {
                    flags.bits = if on {
                        flags.bits | bit
                    } else {
                        flags.bits & !bit
                    };
                    flags.armed = true;
                }
            }
        });
        ui.add_space(4.0);
        ui.weak("Every tile the brush paints gets exactly these flags. Untick the channel to keep each tile's own.");
    }

    /// The Height tool's options.
    pub(super) fn height_tool_options(&mut self, ui: &mut egui::Ui) {
        let mut tool = self.settings.prefs.height_tool.clone();
        ui.add_space(2.0);
        ui.horizontal_wrapped(|ui| {
            for op in HeightOp::ALL {
                if ui
                    .selectable_label(tool.op == op, op.label())
                    .on_hover_text(op.hint())
                    .clicked()
                {
                    tool.op = op;
                }
            }
        });
        if tool.op == HeightOp::Flatten {
            ui.horizontal_wrapped(|ui| {
                ui.weak("Level to");
                if ui
                    .selectable_label(tool.flatten_to.is_none(), "Height under the cursor")
                    .clicked()
                {
                    tool.flatten_to = None;
                }
                if ui
                    .selectable_label(tool.flatten_to.is_some(), "A height")
                    .clicked()
                    && tool.flatten_to.is_none()
                {
                    tool.flatten_to = Some(0);
                }
                if let Some(target) = &mut tool.flatten_to {
                    let mut value = elevation(*target);
                    if ui
                        .add(
                            egui::DragValue::new(&mut value)
                                .range(0..=MAX_ELEVATION)
                                .speed(1.0),
                        )
                        .changed()
                    {
                        *target = height_for_elevation(value);
                    }
                }
            });
        }
        let op = tool.op;
        self.settings.prefs.height_tool = tool;
        let selected = self.interaction.selection_tiles().len();
        ui.horizontal_wrapped(|ui| {
            ui.weak(op.hint());
            if selected > 0
                && ui
                    .button(format!(
                        "{}  Apply to {selected} selected tiles",
                        icons::TOOL_HEIGHT
                    ))
                    .clicked()
            {
                self.height_on_selection();
            }
        });
    }
}
