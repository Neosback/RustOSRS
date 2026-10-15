//! The user's own tools on the floating rail: the buttons, running one, and the window that makes or
//! edits one. See `crate::user_tools` for the data and `script.rs` for what scripts can say.

use super::EditorApp;
use crate::hud::Destination;
use crate::icons;
use crate::paint::Tool;
use crate::user_tools::{self, ToolDraft, UserTool, UserToolKind};
use eframe::egui;

impl EditorApp {
    /// Press a user tool's button.
    pub(super) fn run_user_tool(&mut self, index: usize) {
        let Some(tool) = self.settings.prefs.user_tools.get(index).cloned() else {
            return;
        };
        match tool.kind {
            UserToolKind::Preset {
                tool,
                brush,
                spline,
            } => {
                self.settings.prefs.brush = brush;
                self.settings.prefs.spline = spline;
                self.set_tool(tool);
            }
            UserToolKind::Bookmark { x, y, plane } => {
                self.teleport_to(Destination::Tile { x, y, plane });
            }
            UserToolKind::Link { url } => {
                let url =
                    user_tools::fill_link(&url, self.camera_tile(), self.interaction.view_plane());
                if let Err(error) = crate::platform::open_url(&url) {
                    self.ui.set_status(format!("could not open {url}: {error}"));
                }
            }
            UserToolKind::Script { text } => self.run_script(&text),
        }
    }

    /// The user tools as rail buttons, with the "+" that makes a new one.
    pub(super) fn user_tool_buttons(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        let mut run = None;
        let mut edit = None;
        let mut delete = None;
        for (index, tool) in self.settings.prefs.user_tools.iter().enumerate() {
            let response = super::floating::picker_button(
                ui,
                user_tools::glyph(&tool.icon),
                false,
                &format!("{} ({})", tool.name, tool.kind.label()),
            );
            if response.clicked() {
                run = Some(index);
            }
            response.context_menu(|ui| {
                if ui.button("Edit").clicked() {
                    edit = Some(index);
                    ui.close();
                }
                if ui.button("Delete").clicked() {
                    delete = Some(index);
                    ui.close();
                }
            });
        }
        if super::floating::picker_button(
            ui,
            icons::ADD,
            false,
            "Add a tool: a brush preset, a bookmark, a link or a script",
        )
        .clicked()
        {
            self.ui.tool_draft = Some(self.new_draft());
        }
        if let Some(index) = run {
            // The tool acts on the map, so the window gets out of the way first.
            self.ui.plugins_open = false;
            self.run_user_tool(index);
        }
        if let Some(index) = edit {
            self.ui.tool_draft = Some(ToolDraft {
                index: Some(index),
                tool: self.settings.prefs.user_tools[index].clone(),
            });
        }
        if let Some(index) = delete {
            self.settings.prefs.user_tools.remove(index);
        }
    }

    /// A new tool, starting as the current brush as a preset when a brush tool is active.
    fn new_draft(&self) -> ToolDraft {
        let mut draft = ToolDraft::new_script();
        if self.settings.prefs.tool != Tool::Select {
            draft.tool.name = format!("{} preset", self.settings.prefs.tool.label());
            draft.tool.kind = UserToolKind::Preset {
                tool: self.settings.prefs.tool,
                brush: self.settings.prefs.brush.clone(),
                spline: self.settings.prefs.spline.clone(),
            };
        }
        draft
    }

    /// The Add tool / Edit tool window.
    pub(super) fn tool_draft_window(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.ui.tool_draft.take() else {
            return;
        };
        let mut save = false;
        let mut cancel = false;
        let current_tile = self.camera_tile();
        let current_plane = self.interaction.view_plane();
        let (tool_now, brush_now, spline_now) = (
            self.settings.prefs.tool,
            self.settings.prefs.brush.clone(),
            self.settings.prefs.spline.clone(),
        );
        let title = if draft.index.is_some() {
            "Edit tool"
        } else {
            "Add a tool"
        };
        let ((), closed) = super::modal::Modal::new("tool_draft_window", title)
            .size(400.0, 0.0)
            .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Name");
                ui.add(egui::TextEdit::singleline(&mut draft.tool.name).desired_width(200.0));
            });
            ui.add_space(4.0);
            ui.label("Icon");
            ui.horizontal_wrapped(|ui| {
                for (key, glyph) in user_tools::ICONS {
                    if ui
                        .selectable_label(draft.tool.icon == *key, *glyph)
                        .on_hover_text(*key)
                        .clicked()
                    {
                        (*key).clone_into(&mut draft.tool.icon);
                    }
                }
            });
            ui.add_space(6.0);
            ui.label("What it does");
            ui.horizontal(|ui| {
                let current = draft.tool.kind.label();
                let is = |label: &str| current == label;
                if ui.selectable_label(is("Brush preset"), "Brush preset").clicked() {
                    draft.tool.kind = UserToolKind::Preset {
                        tool: if tool_now == Tool::Select { Tool::Paint } else { tool_now },
                        brush: brush_now.clone(),
                        spline: spline_now.clone(),
                    };
                }
                if ui.selectable_label(is("Bookmark"), "Bookmark").clicked() {
                    draft.tool.kind = UserToolKind::Bookmark {
                        x: current_tile.0,
                        y: current_tile.1,
                        plane: current_plane,
                    };
                }
                if ui.selectable_label(is("Link"), "Link").clicked() {
                    draft.tool.kind = UserToolKind::Link {
                        url: "https://oldschool.runescape.wiki/".to_owned(),
                    };
                }
                if ui.selectable_label(is("Script"), "Script").clicked() {
                    draft.tool.kind = UserToolKind::Script {
                        text: "tool paint\nradius 2".to_owned(),
                    };
                }
            });
            ui.add_space(4.0);
            match &mut draft.tool.kind {
                UserToolKind::Preset { tool, brush, spline } => {
                    ui.weak(format!(
                        "Saves the {} tool with the brush as it is now: channels, floor, shape, size.",
                        tool.label()
                    ));
                    if ui.button("Take the current brush again").clicked() {
                        *tool = if tool_now == Tool::Select { *tool } else { tool_now };
                        *brush = brush_now.clone();
                        *spline = spline_now.clone();
                    }
                }
                UserToolKind::Bookmark { x, y, plane } => {
                    ui.horizontal(|ui| {
                        ui.label("x");
                        ui.add(egui::DragValue::new(x));
                        ui.label("y");
                        ui.add(egui::DragValue::new(y));
                        ui.label("plane");
                        ui.add(egui::DragValue::new(plane).range(0..=3));
                        if ui.button("Here").on_hover_text("The camera's tile").clicked() {
                            (*x, *y) = current_tile;
                            *plane = current_plane;
                        }
                    });
                }
                UserToolKind::Link { url } => {
                    ui.add(egui::TextEdit::singleline(url).desired_width(360.0));
                    ui.weak("{x} {y} {plane} {region} become the camera's tile.");
                }
                UserToolKind::Script { text } => {
                    ui.add(
                        egui::TextEdit::multiline(text)
                            .code_editor()
                            .desired_rows(6)
                            .desired_width(360.0),
                    );
                    ui.weak("One command per line; the Script window (View menu) lists them.");
                }
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui
                    .add(crate::theme::primary_button("Save").enabled(!draft.tool.name.trim().is_empty()))
                    .clicked()
                {
                    save = true;
                }
                if ui.add(crate::theme::outline_button("Cancel")).clicked() {
                    cancel = true;
                }
            });
        });
        if save {
            let tool: UserTool = draft.tool;
            match draft.index {
                Some(index) if index < self.settings.prefs.user_tools.len() => {
                    self.settings.prefs.user_tools[index] = tool;
                }
                _ => self.settings.prefs.user_tools.push(tool),
            }
        } else if !closed && !cancel {
            self.ui.tool_draft = Some(draft);
        }
    }

    /// The Plugins window's "My tools" section: the same tools as a list.
    pub(super) fn user_tool_list(&mut self, ui: &mut egui::Ui) {
        crate::theme::section_heading(ui, "My tools");
        if self.settings.prefs.user_tools.is_empty() {
            ui.weak("None yet. Use the + on the picker rail to add a brush preset, bookmark, link or script.");
        }
        let mut run = None;
        let mut edit = None;
        let mut delete = None;
        for (index, tool) in self.settings.prefs.user_tools.iter().enumerate() {
            ui.horizontal(|ui| {
                if ui
                    .button(format!("{}  {}", user_tools::glyph(&tool.icon), tool.name))
                    .on_hover_text(tool.kind.label())
                    .clicked()
                {
                    run = Some(index);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .small_button(icons::DELETE)
                        .on_hover_text("Delete")
                        .clicked()
                    {
                        delete = Some(index);
                    }
                    if ui.small_button("Edit").clicked() {
                        edit = Some(index);
                    }
                });
            });
        }
        if ui
            .add(crate::theme::primary_button(format!(
                "{}  Add a tool",
                icons::ADD
            )))
            .clicked()
        {
            self.ui.tool_draft = Some(self.new_draft());
        }
        if ui
            .add(crate::theme::outline_button(format!(
                "{}  Script window",
                icons::SCRIPT
            )))
            .clicked()
        {
            self.ui.script_open = true;
        }
        if let Some(index) = run {
            self.run_user_tool(index);
        }
        if let Some(index) = edit {
            self.ui.tool_draft = Some(ToolDraft {
                index: Some(index),
                tool: self.settings.prefs.user_tools[index].clone(),
            });
        }
        if let Some(index) = delete {
            self.settings.prefs.user_tools.remove(index);
        }
    }
}
