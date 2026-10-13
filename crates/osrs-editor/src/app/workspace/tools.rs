//! Tool switching, the quick tools HUD, and brush controls.

use super::*;

/// The tool's icon.
pub fn tool_icon(tool: Tool) -> &'static str {
    match tool {
        Tool::Select => icons::TOOL_SELECT,
        Tool::Paint => icons::TOOL_PAINT,
        Tool::Spline => icons::TOOL_SPLINE,
        Tool::Height => icons::TOOL_HEIGHT,
        Tool::Objects => icons::TOOL_OBJECTS,
    }
}

pub fn select_mode_icon(mode: SelectMode) -> &'static str {
    match mode {
        SelectMode::Auto => icons::SELECT_AUTO,
        SelectMode::Objects => icons::SELECT_OBJECTS,
        SelectMode::Tiles => icons::SELECT_TILES,
    }
}

pub(super) fn quick_tool_button(
    ui: &mut egui::Ui,
    tool: Tool,
    selected: bool,
    key: Option<String>,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(60.0, 58.0), Sense::click());
    let visuals = ui.visuals();
    let fill = if selected {
        visuals.selection.bg_fill
    } else if response.hovered() {
        visuals.widgets.hovered.weak_bg_fill
    } else {
        visuals.widgets.inactive.bg_fill
    };
    ui.painter().rect_filled(rect, 8.0, fill);
    let tint = if selected {
        visuals.selection.stroke.color
    } else {
        visuals.text_color()
    };
    ui.painter().text(
        rect.center() - Vec2::new(0.0, 8.0),
        egui::Align2::CENTER_CENTER,
        tool_icon(tool),
        egui::FontId::proportional(21.0),
        tint,
    );
    ui.painter().text(
        rect.center() + Vec2::new(0.0, 14.0),
        egui::Align2::CENTER_CENTER,
        tool.label(),
        egui::FontId::proportional(12.0),
        tint,
    );
    if let Some(key) = key {
        ui.painter().text(
            rect.right_top() + Vec2::new(-6.0, 7.0),
            egui::Align2::RIGHT_CENTER,
            key,
            egui::FontId::monospace(10.0),
            visuals.weak_text_color(),
        );
    }
    response
}

impl EditorApp {
    /// Fill the brush from a tile (`I`, Alt + click, or the tile's context menu).
    pub(in crate::app) fn eyedrop(&mut self, values: TileValues) {
        self.prefs.brush.sample(values);
        if self.prefs.tool == Tool::Select {
            self.set_tool(Tool::Paint);
        }
        self.notice = Some((
            format!(
                "Brush: overlay {}, underlay {}",
                values.overlay, values.underlay
            ),
            Instant::now(),
        ));
    }

    pub(in crate::app) fn set_tool(&mut self, tool: Tool) {
        if self.prefs.tool == tool {
            return;
        }
        self.stroke = None;
        self.marquee = None;
        self.prefs.tool = tool;
        if tool == Tool::Objects {
            // The Objects tool works on objects: its picker is the object picker.
            self.prefs.select_mode = crate::paint::SelectMode::Objects;
        }
        let tabs = tool.tabs();
        if let Some(first) = tabs.first() {
            if !tabs.contains(&self.prefs.drawer_tab) {
                self.prefs.drawer_tab = *first;
            }
            self.prefs.drawer_open = true;
        }
    }

    /// Space: a small panel at the cursor with what the active tool can use.
    pub(in crate::app) fn quick_tools_hud(&mut self, ctx: &egui::Context) {
        let Some(origin) = self.quick_open else {
            return;
        };
        let screen = ctx.content_rect();
        let mut close = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let width = 292.0;
        let position = Pos2::new(
            (origin.x - width / 2.0)
                .clamp(screen.left() + 8.0, (screen.right() - width - 8.0).max(8.0)),
            (origin.y - 40.0).clamp(screen.top() + 40.0, (screen.bottom() - 300.0).max(48.0)),
        );
        let area = egui::Area::new(egui::Id::new("quick_tools"))
            .order(egui::Order::Foreground)
            .fixed_pos(position)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .inner_margin(egui::Margin::same(10))
                    .show(ui, |ui| {
                        ui.set_width(width - 20.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Quick tools").strong());
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.weak("Esc closes");
                                },
                            );
                        });
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            for tool in Tool::ALL {
                                let key = self.keymap.binding(tool.action()).map(|b| b.describe());
                                if quick_tool_button(ui, tool, self.prefs.tool == tool, key)
                                    .clicked()
                                {
                                    self.set_tool(tool);
                                    close = true;
                                }
                            }
                        });
                        self.quick_tool_options(ui);
                        crate::theme::section_heading(ui, "View");
                        ui.horizontal(|ui| {
                            ui.weak("Plane");
                            for plane in 0..4_u8 {
                                ui.selectable_value(&mut self.view_plane, plane, plane.to_string());
                            }
                            ui.separator();
                            ui.selectable_value(
                                &mut self.camera.mode,
                                crate::camera::CameraMode::Fly,
                                "Fly",
                            );
                            ui.selectable_value(
                                &mut self.camera.mode,
                                crate::camera::CameraMode::Orbit,
                                "Orbit",
                            );
                        });
                    });
            });
        // A click outside closes it.
        if ctx.input(|i| i.pointer.any_pressed())
            && let Some(pointer) = ctx.input(|i| i.pointer.interact_pos())
            && !area.response.rect.expand(4.0).contains(pointer)
        {
            close = true;
        }
        if close {
            self.quick_open = None;
        }
    }

    /// The quick HUD's section for the active tool.
    pub(super) fn quick_tool_options(&mut self, ui: &mut egui::Ui) {
        match self.prefs.tool {
            Tool::Select => {
                crate::theme::section_heading(ui, "Select");
                ui.horizontal(|ui| {
                    for mode in SelectMode::ALL {
                        let text = format!("{}  {}", select_mode_icon(mode), mode.label());
                        ui.selectable_value(&mut self.prefs.select_mode, mode, text);
                    }
                });
            }
            Tool::Paint => {
                crate::theme::section_heading(ui, "Brush paints");
                ui.horizontal_wrapped(|ui| {
                    for tab in Tool::Paint.tabs() {
                        let mut armed = self.prefs.brush.is_armed(*tab);
                        if ui.toggle_value(&mut armed, tab.label()).changed() {
                            self.prefs.brush.set_armed(*tab, armed);
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.weak("Brush");
                    self.brush_controls(ui, true, false);
                });
            }
            Tool::Spline => {
                crate::theme::section_heading(ui, "Path");
                ui.horizontal(|ui| {
                    ui.weak("Width");
                    ui.add(
                        egui::DragValue::new(&mut self.prefs.spline.width)
                            .range(0.5..=24.0)
                            .speed(0.1)
                            .suffix(" tiles"),
                    );
                    ui.weak("Edge");
                    ui.add(
                        egui::DragValue::new(&mut self.prefs.spline.edge_weight)
                            .range(0.0..=2.0)
                            .speed(0.05),
                    );
                });
            }
            Tool::Height => {
                crate::theme::section_heading(ui, "Height");
                let current = self.prefs.height_tool.op;
                let options: Vec<_> = crate::heights::HeightOp::ALL
                    .iter()
                    .map(|op| (*op, op.label()))
                    .collect();
                if let Some(op) = crate::app::widgets::chips(ui, current, &options) {
                    self.prefs.height_tool.op = op;
                }
                ui.horizontal(|ui| {
                    ui.weak("Brush");
                    self.brush_controls(ui, true, false);
                });
            }
            Tool::Objects => {
                crate::theme::section_heading(ui, "Objects");
                self.object_drawer(ui);
            }
        }
    }

    /// A held tool key returns to the previous tool when released (called every frame, even
    /// while a text field has focus, so a held key can never strand the tool).
    pub(in crate::app) fn tool_release(&mut self, ctx: &egui::Context) {
        let Some((previous, key, since)) = self.peek else {
            return;
        };
        if ctx.input(|i| i.key_down(key)) {
            return;
        }
        self.peek = None;
        if since.elapsed() > PEEK_AFTER {
            self.set_tool(previous);
        }
    }

    /// Tool, drawer and brush shortcuts.
    ///
    /// A tool key *taps* (switches and stays; pressing the same key again returns to Select) or
    /// *holds* (peeks at the tool and returns to the previous one on release).
    pub(in crate::app) fn tool_actions(&mut self, ctx: &egui::Context) {
        let pressed = |id: &str| ctx.input(|i| self.keymap.pressed(i, id));
        let tool = Tool::ALL.into_iter().find(|tool| pressed(tool.action()));
        let quick = pressed("quick.tools");
        let drawer = pressed("drawer.toggle");
        let smaller = pressed("brush.smaller");
        let bigger = pressed("brush.bigger");
        let eyedrop = pressed("tool.eyedrop");
        if let Some(tool) = tool {
            let key = self.keymap.binding(tool.action()).map(|b| b.key);
            if tool == self.prefs.tool && tool != Tool::Select {
                self.set_tool(Tool::Select);
                self.peek = None;
            } else {
                let previous = self.prefs.tool;
                self.set_tool(tool);
                self.peek = key.map(|key| (previous, key, Instant::now()));
            }
        }
        if quick {
            self.quick_open = match self.quick_open {
                Some(_) => None,
                None => Some(
                    ctx.input(|i| i.pointer.hover_pos())
                        .unwrap_or_else(|| ctx.content_rect().center()),
                ),
            };
        }
        if drawer && self.prefs.tool.has_drawer() {
            self.prefs.drawer_open = !self.prefs.drawer_open;
        }
        if smaller {
            self.prefs.brush.radius = self.prefs.brush.radius.saturating_sub(1);
        }
        if bigger {
            self.prefs.brush.radius = (self.prefs.brush.radius + 1).min(16);
        }
        if eyedrop
            && let Some(tile) = self.hover.tile.map(|pick| pick.tile)
            && let Some(values) = self.tile_now(tile)
        {
            self.eyedrop(values);
        }
    }

    /// Esc: back to Select; in Select it clears the selection.
    pub(in crate::app) fn escape_action(&mut self) {
        if self.prefs.tool != Tool::Select {
            self.spline.points.clear();
            self.set_tool(Tool::Select);
        } else {
            self.clear_selection();
        }
    }

    /// Brush size and footprint (in the drawer header and the quick HUD). `right_to_left` is for
    /// a right-aligned header, where widgets are added from the right.
    pub(in crate::app) fn brush_controls(
        &mut self,
        ui: &mut egui::Ui,
        compact: bool,
        right_to_left: bool,
    ) {
        #[derive(Clone, Copy)]
        enum Piece {
            Label,
            Smaller,
            Size,
            Bigger,
            Footprint(crate::paint::BrushShape),
        }
        let mut pieces = Vec::new();
        if !compact {
            pieces.push(Piece::Label);
        }
        pieces.extend([
            Piece::Smaller,
            Piece::Size,
            Piece::Bigger,
            Piece::Footprint(crate::paint::BrushShape::Square),
            Piece::Footprint(crate::paint::BrushShape::Circle),
        ]);
        if right_to_left {
            pieces.reverse();
        }
        let brush = &mut self.prefs.brush;
        for piece in pieces {
            match piece {
                Piece::Label => {
                    ui.weak("Brush");
                }
                Piece::Smaller => {
                    if ui
                        .small_button("\u{2212}")
                        .on_hover_text("Smaller ([)")
                        .clicked()
                    {
                        brush.radius = brush.radius.saturating_sub(1);
                    }
                }
                Piece::Size => {
                    let side = u32::from(brush.radius) * 2 + 1;
                    ui.label(format!("{side}\u{d7}{side}"))
                        .on_hover_text("Brush footprint in tiles");
                }
                Piece::Bigger => {
                    if ui.small_button("+").on_hover_text("Bigger (])").clicked() {
                        brush.radius = (brush.radius + 1).min(16);
                    }
                }
                Piece::Footprint(shape) => {
                    let (icon, tip) = match shape {
                        crate::paint::BrushShape::Square => {
                            (icons::BRUSH_SQUARE, "Square footprint")
                        }
                        crate::paint::BrushShape::Circle => {
                            (icons::BRUSH_CIRCLE, "Round footprint")
                        }
                    };
                    if ui
                        .selectable_label(brush.footprint == shape, icon)
                        .on_hover_text(tip)
                        .clicked()
                    {
                        brush.footprint = shape;
                    }
                }
            }
        }
    }
}
