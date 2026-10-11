//! The viewport context menu (Shift + right click) in two styles, chosen in Settings > Interface:
//!
//! * **Game**: exactly what the client would show for the thing under the cursor (`Choose
//!   Option`, the object's actions, `Examine`, `Walk here`, `Cancel`), built from the same data
//!   as the mouse tooltip (`interact.rs`). Nothing of the editor is in it.
//! * **Editor**: the editor's own menu (select, focus, teleport, copy). Editing actions (paint
//!   here, move, delete ...) join this menu when their tools exist; they never go in the game menu.
//!
//! Both menus produce the same two kinds of result: a game [`interact::Effect`] or an editor
//! [`MenuAction`], applied after the menu closes.

use super::EditorApp;
use crate::hud::Destination;
use crate::icons;
use crate::interact;
use crate::pick::{self, ObjectPick, Pick};
use crate::settings::ContextMenuStyle;
use eframe::egui;
use std::time::Instant;

/// What an editor-menu entry does.
#[derive(Debug, Clone)]
enum MenuAction {
    Select(ObjectPick),
    Focus(ObjectPick),
    TeleportHere((i32, i32)),
    SetTool(crate::paint::Tool),
    Eyedrop((i32, i32)),
    ApplyBrush((i32, i32)),
}

impl EditorApp {
    /// Draw the open context menu in the chosen style.
    pub(super) fn context_menu(&mut self, ctx: &egui::Context) {
        let Some(position) = self.context_menu else {
            return;
        };
        let pick = self.context_pick.clone();
        let outcome = match self.prefs.context_menu {
            ContextMenuStyle::Game => self.game_menu(ctx, position, &pick),
            ContextMenuStyle::Editor => self.editor_menu(ctx, position, &pick),
        };
        if outcome.close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.context_menu = None;
        }
        if let Some(effect) = outcome.effect {
            self.apply_effect(effect);
        }
        if let Some(action) = outcome.action {
            self.apply_menu_action(action);
        }
    }

    fn apply_menu_action(&mut self, action: MenuAction) {
        match action {
            MenuAction::Select(object) => self.selected = Some(object),
            MenuAction::Focus(object) => {
                self.selected = Some(object);
                self.focus_selection();
            }
            MenuAction::SetTool(tool) => self.set_tool(tool),
            MenuAction::Eyedrop(tile) => {
                if let Some(values) = pick::tile_values(&self.infos, tile, self.view_plane) {
                    self.eyedrop(values);
                }
            }
            MenuAction::ApplyBrush(tile) => self.apply_brush_at(tile),
            MenuAction::TeleportHere((x, y)) => {
                let plane = self.view_plane;
                self.teleport_to(Destination::Tile { x, y, plane });
                self.notice = Some((format!("Teleported to {x}, {y}"), Instant::now()));
            }
        }
    }

    /// The game's menu: `Choose Option`, entries, `Cancel`.
    fn game_menu(&self, ctx: &egui::Context, position: egui::Pos2, pick: &Pick) -> Outcome {
        let entries = interact::entries(&self.infos, pick);
        let mut outcome = Outcome::default();
        let brown = egui::Color32::from_rgb(93, 84, 71);
        let header = egui::Color32::from_rgb(38, 34, 28);
        let area = egui::Area::new(egui::Id::new("viewport_context_menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(position)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(brown)
                    .stroke(egui::Stroke::new(1.0, egui::Color32::BLACK))
                    .inner_margin(egui::Margin::same(2))
                    .show(ui, |ui| {
                        ui.set_width(210.0);
                        ui.spacing_mut().item_spacing.y = 0.0;
                        egui::Frame::new()
                            .fill(header)
                            .inner_margin(egui::Margin::symmetric(6, 3))
                            .show(ui, |ui| {
                                ui.set_min_width(ui.available_width());
                                ui.label(
                                    egui::RichText::new("Choose Option")
                                        .color(egui::Color32::from_rgb(220, 205, 170))
                                        .size(13.0),
                                );
                            });
                        let line = |ui: &mut egui::Ui, job: egui::text::LayoutJob| -> bool {
                            let galley = ui.painter().layout_job(job);
                            let size = egui::vec2(
                                ui.available_width().max(galley.size().x + 12.0),
                                galley.size().y + 6.0,
                            );
                            let (rect, response) =
                                ui.allocate_exact_size(size, egui::Sense::click());
                            if response.hovered() {
                                ui.painter().rect_filled(
                                    rect,
                                    0.0,
                                    egui::Color32::from_white_alpha(46),
                                );
                            }
                            ui.painter().galley(
                                rect.min + egui::vec2(6.0, 3.0),
                                galley,
                                egui::Color32::WHITE,
                            );
                            response.clicked()
                        };
                        for entry in &entries {
                            if line(ui, interact::entry_job(entry, 13.5, None)) {
                                outcome.effect = Some(entry.effect.clone());
                                outcome.close = true;
                            }
                        }
                        let cancel = interact::Entry {
                            action: "Cancel".to_owned(),
                            target: None,
                            effect: interact::Effect::Cancel,
                        };
                        if line(ui, interact::entry_job(&cancel, 13.5, None)) {
                            outcome.close = true;
                        }
                    });
            });
        outcome.close |=
            ctx.input(|i| i.pointer.any_pressed()) && !area.response.contains_pointer();
        outcome
    }

    /// The editor's menu.
    fn editor_menu(&self, ctx: &egui::Context, position: egui::Pos2, pick: &Pick) -> Outcome {
        let mut outcome = Outcome::default();
        let loc = pick
            .object
            .as_ref()
            .and_then(|object| self.infos.get(&object.region)?.locs.get(object.index));
        let area = egui::Area::new(egui::Id::new("viewport_context_menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(position)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_min_width(220.0);
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.visuals_mut().widgets.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;
                    ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
                        // Quick tool switch: the tools as icons, the active one filled.
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 3.0;
                            for tool in crate::paint::Tool::ALL {
                                let active = self.prefs.tool == tool;
                                if ui
                                    .selectable_label(active, super::workspace::tool_icon(tool))
                                    .on_hover_text(tool.label())
                                    .clicked()
                                {
                                    outcome.action = Some(MenuAction::SetTool(tool));
                                    outcome.close = true;
                                }
                            }
                            ui.weak(self.prefs.tool.label());
                        });
                        ui.separator();
                        // What the menu is about.
                        if let (Some(object), Some(loc)) = (&pick.object, loc) {
                            let name = loc
                                .definition
                                .name
                                .as_deref()
                                .filter(|name| !name.is_empty() && *name != "null")
                                .unwrap_or("(unnamed object)");
                            ui.strong(name);
                            ui.weak(format!(
                                "object {} · tile {}, {} · plane {}",
                                loc.definition.identity.id.get(),
                                loc.tile.0,
                                loc.tile.1,
                                loc.plane
                            ));
                            ui.separator();
                            let already = self.selected.as_ref().is_some_and(|s| {
                                s.region == object.region && s.index == object.index
                            });
                            if ui
                                .add_enabled(
                                    !already,
                                    egui::Button::new(format!("{}  Select", icons::SELECT)),
                                )
                                .clicked()
                            {
                                outcome.action = Some(MenuAction::Select(object.clone()));
                                outcome.close = true;
                            }
                            if ui
                                .button(format!("{}  Focus camera", icons::CROSSHAIR))
                                .clicked()
                            {
                                outcome.action = Some(MenuAction::Focus(object.clone()));
                                outcome.close = true;
                            }
                        } else if let Some(tile) = &pick.tile {
                            ui.strong(format!("Tile {}, {}", tile.tile.0, tile.tile.1));
                            ui.weak(format!("plane {}", self.view_plane));
                            ui.separator();
                        }
                        if let Some(tile) = &pick.tile
                            && self.prefs.tool != crate::paint::Tool::Select
                        {
                            let verb = match self.prefs.tool {
                                crate::paint::Tool::Paint => "Paint",
                                crate::paint::Tool::Height => "Sculpt",
                                _ => "Set flags",
                            };
                            let armed = self.prefs.brush.armed_channels().join(", ");
                            if ui
                                .button(format!(
                                    "{}  {verb} here",
                                    super::workspace::tool_icon(self.prefs.tool)
                                ))
                                .on_hover_text(if armed.is_empty() {
                                    "Nothing armed: arm a channel in the drawer".to_owned()
                                } else {
                                    format!("Applies: {armed}")
                                })
                                .clicked()
                            {
                                outcome.action = Some(MenuAction::ApplyBrush(tile.tile));
                                outcome.close = true;
                            }
                            if ui
                                .button(format!("{}  Eyedrop tile", icons::EYEDROPPER))
                                .on_hover_text("Copy this tile's values into the brush (I)")
                                .clicked()
                            {
                                outcome.action = Some(MenuAction::Eyedrop(tile.tile));
                                outcome.close = true;
                            }
                        }
                        if let Some(tile) = &pick.tile
                            && ui
                                .button(format!("{}  Teleport here", icons::TELEPORT))
                                .clicked()
                        {
                            outcome.action = Some(MenuAction::TeleportHere(tile.tile));
                            outcome.close = true;
                        }
                        ui.menu_button(format!("{}  Copy", ph_copy()), |ui| {
                            outcome.close |= self.copy_menu(ui, pick);
                        });
                        if pick.object.is_none() && pick.tile.is_none() {
                            ui.weak("nothing under the cursor");
                        }
                    });
                });
            });
        outcome.close |= ctx.input(|i| i.pointer.any_pressed())
            && !area.response.contains_pointer()
            && !egui::Popup::is_any_open(ctx);
        outcome
    }
}

fn ph_copy() -> &'static str {
    egui_phosphor::regular::COPY
}

/// What a menu produced this frame.
#[derive(Default)]
struct Outcome {
    effect: Option<interact::Effect>,
    action: Option<MenuAction>,
    close: bool,
}

impl EditorApp {
    /// Carry out a chosen menu entry. Game actions are inert until the editor talks to a server;
    /// walking here moves the camera there.
    pub(super) fn apply_effect(&mut self, effect: interact::Effect) {
        let object_name = |app: &Self, object: &ObjectPick| {
            app.infos
                .get(&object.region)
                .and_then(|info| info.locs.get(object.index))
                .map(|loc| {
                    (
                        loc.definition.name.clone().unwrap_or_default(),
                        loc.definition.identity.id.get(),
                    )
                })
        };
        match effect {
            interact::Effect::Cancel => {}
            interact::Effect::WalkHere((x, y)) => {
                let plane = self.view_plane;
                self.teleport_to(Destination::Tile { x, y, plane });
            }
            interact::Effect::Examine(object) => {
                if let Some((name, id)) = object_name(self, &object) {
                    self.notice = Some((format!("{name} (object {id})"), Instant::now()));
                }
            }
            interact::Effect::ObjectAction { object, slot } => {
                if let Some((name, id)) = object_name(self, &object) {
                    let action = self
                        .infos
                        .get(&object.region)
                        .and_then(|info| info.locs.get(object.index))
                        .and_then(|loc| loc.definition.actions[slot].clone())
                        .unwrap_or_default();
                    self.notice = Some((
                        format!(
                            "{action} {name} (object {id}): game actions need a server connection"
                        ),
                        Instant::now(),
                    ));
                }
            }
        }
    }

    /// "Copy ..." actions for an object and/or tile (context menu and the Selection section).
    pub(super) fn copy_menu(&self, ui: &mut egui::Ui, pick: &Pick) -> bool {
        let mut taken = false;
        let loc = pick
            .object
            .as_ref()
            .and_then(|object| self.infos.get(&object.region)?.locs.get(object.index));
        let center = loc.map(|loc| loc.tile).or(pick.tile.map(|tile| tile.tile));
        let plane = loc.map_or(usize::from(self.view_plane), |loc| usize::from(loc.plane));
        let mut copy = |ui: &mut egui::Ui, label: &str, text: String| {
            if ui.button(label).clicked() {
                ui.ctx().copy_text(text);
                ui.close();
                taken = true;
            }
        };
        if let Some(object) = &pick.object {
            copy(
                ui,
                "Copy object",
                pick::describe_object(&self.infos, object),
            );
        }
        if let Some(tile) = center {
            copy(ui, "Copy tile", pick::describe_tile(&self.infos, tile));
            copy(
                ui,
                "Copy neighbourhood (3x3)",
                pick::describe_neighbourhood(&self.infos, tile, plane, 1),
            );
            copy(
                ui,
                "Copy neighbourhood (5x5)",
                pick::describe_neighbourhood(&self.infos, tile, plane, 2),
            );
            if let Some(object) = &pick.object {
                let mut text = pick::describe_object(&self.infos, object);
                text.push_str(&pick::describe_neighbourhood(&self.infos, tile, plane, 1));
                copy(ui, "Copy object + neighbourhood", text);
            }
        } else {
            ui.label("nothing under the cursor");
        }
        taken
    }
}
