//! Tool switching, the quick tools HUD, and brush controls.
use web_time::Instant;

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
        self.settings.prefs.brush.sample(values);
        if self.settings.prefs.tool == Tool::Select {
            self.set_tool(Tool::Paint);
        }
        self.ui.notify(format!(
            "Brush: overlay {}, underlay {}",
            values.overlay, values.underlay
        ));
    }

    pub(in crate::app) fn set_tool(&mut self, tool: Tool) {
        if self.settings.prefs.tool == tool {
            return;
        }
        self.cancel_stroke();
        self.interaction.cancel_marquee();
        self.settings.prefs.tool = tool;
        if tool == Tool::Objects {
            // The Objects tool works on objects: its picker is the object picker.
            self.settings.prefs.select_mode = crate::paint::SelectMode::Objects;
        }
        let tabs = tool.tabs();
        if let Some(first) = tabs.first() {
            if !tabs.contains(&self.settings.prefs.drawer_tab) {
                self.settings.prefs.drawer_tab = *first;
            }
            self.settings.prefs.drawer_open = true;
        }
    }

    /// Space: a small panel at the cursor with what the active tool can use.
    pub(in crate::app) fn quick_tools_hud(&mut self, ctx: &egui::Context) {
        let Some(origin) = self.ui.quick_open else {
            return;
        };
        let screen = ctx.content_rect();
        let mut close = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        // The HUD never outgrows the window: it narrows on small screens and scrolls when its
        // content (the Objects tool's drawer) is taller than the space below the cursor.
        let width = 292.0_f32.min((screen.width() - 16.0).max(160.0));
        let max_height = (screen.height() - 96.0).max(120.0);
        let position = Pos2::new(
            (origin.x - width / 2.0)
                .clamp(screen.left() + 8.0, (screen.right() - width - 8.0).max(8.0)),
            (origin.y - 40.0).clamp(
                screen.top() + 40.0,
                (screen.bottom() - max_height.min(300.0) - 8.0).max(48.0),
            ),
        );
        let area = egui::Area::new(egui::Id::new("quick_tools"))
            .order(egui::Order::Foreground)
            .fixed_pos(position)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .inner_margin(egui::Margin::same(10))
                    .show(ui, |ui| {
                        ui.set_width(width - 20.0);
                        egui::ScrollArea::vertical()
                            .id_salt("quick_tools_scroll")
                            .max_height(max_height)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
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
                                ui.horizontal_wrapped(|ui| {
                                    ui.spacing_mut().item_spacing.x = 6.0;
                                    for tool in Tool::ALL {
                                        let key = self
                                            .settings
                                            .keymap
                                            .binding(tool.action())
                                            .map(|b| b.describe());
                                        if quick_tool_button(
                                            ui,
                                            tool,
                                            self.settings.prefs.tool == tool,
                                            key,
                                        )
                                        .clicked()
                                        {
                                            self.set_tool(tool);
                                            close = true;
                                        }
                                    }
                                });
                                self.quick_tool_options(ui);
                                crate::theme::section_heading(ui, "View");
                                ui.horizontal_wrapped(|ui| {
                                    let mut view_plane = self.interaction.view_plane();
                                    let mut camera_mode = self.interaction.camera().mode;
                                    ui.weak("Plane");
                                    for plane in 0..4_u8 {
                                        ui.selectable_value(
                                            &mut view_plane,
                                            plane,
                                            plane.to_string(),
                                        );
                                    }
                                    ui.separator();
                                    ui.selectable_value(
                                        &mut camera_mode,
                                        crate::camera::CameraMode::Fly,
                                        "Fly",
                                    );
                                    ui.selectable_value(
                                        &mut camera_mode,
                                        crate::camera::CameraMode::Orbit,
                                        "Orbit",
                                    );
                                    self.interaction.set_camera_mode(camera_mode);
                                    self.interaction.set_view_plane(view_plane);
                                });
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
            self.ui.quick_open = None;
        }
    }

    /// The quick HUD's section for the active tool.
    pub(super) fn quick_tool_options(&mut self, ui: &mut egui::Ui) {
        match self.settings.prefs.tool {
            Tool::Select => {
                crate::theme::section_heading(ui, "Select");
                ui.horizontal_wrapped(|ui| {
                    for mode in SelectMode::ALL {
                        let text = format!("{}  {}", select_mode_icon(mode), mode.label());
                        ui.selectable_value(&mut self.settings.prefs.select_mode, mode, text);
                    }
                });
            }
            Tool::Paint => {
                crate::theme::section_heading(ui, "Brush paints");
                ui.horizontal_wrapped(|ui| {
                    for tab in Tool::Paint.tabs() {
                        let mut armed = self.settings.prefs.brush.is_armed(*tab);
                        if ui.toggle_value(&mut armed, tab.label()).changed() {
                            self.settings.prefs.brush.set_armed(*tab, armed);
                        }
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    ui.weak("Brush");
                    self.brush_controls(ui, true, false);
                });
            }
            Tool::Spline => {
                self.spline_drawer(
                    ui,
                    crate::app::spline_tool::sections::SplineSections::COMPACT,
                );
            }
            Tool::Height => {
                crate::theme::section_heading(ui, "Height");
                let current = self.settings.prefs.height_tool.op;
                let options: Vec<_> = crate::heights::HeightOp::ALL
                    .iter()
                    .map(|op| (*op, op.label()))
                    .collect();
                if let Some(op) = crate::app::widgets::chips(ui, current, &options) {
                    self.settings.prefs.height_tool.op = op;
                }
                ui.horizontal_wrapped(|ui| {
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
        let Some(key) = self.interaction.peek_key() else {
            return;
        };
        let key_down = ctx.input(|i| i.key_down(key));
        if let Some(previous) =
            self.interaction
                .release_tool_key(key_down, Instant::now(), PEEK_AFTER)
        {
            self.set_tool(previous);
        }
    }

    /// Tool, drawer and brush shortcuts.
    ///
    /// A tool key *taps* (switches and stays; pressing the same key again returns to Select) or
    /// *holds* (peeks at the tool and returns to the previous one on release).
    pub(in crate::app) fn tool_actions(&mut self, ctx: &egui::Context) {
        let pressed = |id: &str| ctx.input(|i| self.settings.keymap.pressed(i, id));
        let tool = Tool::ALL.into_iter().find(|tool| pressed(tool.action()));
        let quick = pressed("quick.tools");
        let drawer = pressed("drawer.toggle");
        let smaller = pressed("brush.smaller");
        let bigger = pressed("brush.bigger");
        let eyedrop = pressed("tool.eyedrop");
        if let Some(tool) = tool {
            let key = self.settings.keymap.binding(tool.action()).map(|b| b.key);
            let target = self.interaction.press_tool_key(
                self.settings.prefs.tool,
                tool,
                key,
                Instant::now(),
            );
            self.set_tool(target);
        }
        if quick {
            self.ui.quick_open = match self.ui.quick_open {
                Some(_) => None,
                None => Some(
                    ctx.input(|i| i.pointer.hover_pos())
                        .unwrap_or_else(|| ctx.content_rect().center()),
                ),
            };
        }
        if drawer && self.settings.prefs.tool.has_drawer() {
            self.settings.prefs.drawer_open = !self.settings.prefs.drawer_open;
        }
        if smaller {
            self.settings.prefs.brush.radius = self.settings.prefs.brush.radius.saturating_sub(1);
        }
        if bigger {
            self.settings.prefs.brush.radius = (self.settings.prefs.brush.radius + 1).min(16);
        }
        if eyedrop
            && let Some(tile) = self.interaction.hover().tile.map(|pick| pick.tile)
            && let Some(values) = self.tile_now(tile)
        {
            self.eyedrop(values);
        }
    }

    /// Esc: back to Select; in Select it clears the selection.
    pub(in crate::app) fn escape_action(&mut self) {
        if self.placer.disarm() {
            return;
        }
        if self.settings.prefs.tool != Tool::Select {
            self.interaction.clear_spline_points();
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
        let brush = &mut self.settings.prefs.brush;
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
