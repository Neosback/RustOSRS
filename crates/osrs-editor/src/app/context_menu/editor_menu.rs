//! The editor-style context menu (Shift + right click).

use super::*;

impl EditorApp {
    /// The editor's menu.
    pub(super) fn editor_menu(
        &self,
        ctx: &egui::Context,
        position: egui::Pos2,
        pick: &Pick,
    ) -> Outcome {
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
                                    .selectable_label(
                                        active,
                                        crate::app::workspace::tool_icon(tool),
                                    )
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
                            let already = self.interaction.selected.as_ref().is_some_and(|s| {
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
                            if ui
                                .button(format!(
                                    "{}  Select all of this kind",
                                    icons::SELECT_OBJECTS
                                ))
                                .on_hover_text("Every loaded object with this id")
                                .clicked()
                            {
                                outcome.action = Some(MenuAction::SelectSimilar(
                                    loc.definition.identity.id.get(),
                                ));
                                outcome.close = true;
                            }
                            ui.separator();
                            let mut op =
                                |ui: &mut egui::Ui, icon: &str, text: &str, tip: &str, what| {
                                    if ui
                                        .button(format!("{icon}  {text}"))
                                        .on_hover_text(tip)
                                        .clicked()
                                    {
                                        outcome.action =
                                            Some(MenuAction::ObjectOp(object.clone(), what));
                                        outcome.close = true;
                                    }
                                };
                            op(
                                ui,
                                icons::TOOL_OBJECTS,
                                "Move",
                                "Select it and switch to the Objects tool: drag to move",
                                ObjectOp::Move,
                            );
                            op(ui, icons::ROTATE, "Turn", "A quarter turn", ObjectOp::Turn);
                            op(
                                ui,
                                icons::DUPLICATE,
                                "Duplicate",
                                "A copy one tile east",
                                ObjectOp::Duplicate,
                            );
                            op(
                                ui,
                                icons::DELETE,
                                "Delete",
                                "Remove it from the map (Ctrl+Z brings it back)",
                                ObjectOp::Delete,
                            );
                            ui.separator();
                            // Objects that change with a varbit or varp (doors, quest objects).
                            if let Some(morphs) = &loc.definition.morphs {
                                let source =
                                    match (&morphs.transform_varbit, &morphs.transform_varp) {
                                        (Some(varbit), _) => format!("varbit {}", varbit.get()),
                                        (None, Some(varp)) => format!("varp {}", varp.get()),
                                        (None, None) => "a variable".to_owned(),
                                    };
                                ui.menu_button(
                                    format!("{}  Changes with {source}", icons::LAYERS),
                                    |ui| {
                                        ui.weak(format!(
                                            "{} states; the cache's default is the first match",
                                            morphs.transforms.len()
                                        ));
                                        for (state, target) in
                                            morphs.transforms.iter().enumerate().take(24)
                                        {
                                            match target {
                                                Some(id) => ui
                                                    .label(format!("{state}: object {}", id.get())),
                                                None => ui.weak(format!("{state}: nothing")),
                                            };
                                        }
                                        if let Some(fallback) = morphs.fallback {
                                            ui.label(format!(
                                                "otherwise: object {}",
                                                fallback.get()
                                            ));
                                        }
                                    },
                                );
                            }
                            if ui.button(format!("{}  Wiki page", icons::WIKI)).clicked() {
                                outcome.action = Some(MenuAction::Wiki(object.clone()));
                                outcome.close = true;
                            }
                        } else if let Some(tile) = &pick.tile {
                            ui.strong(format!("Tile {}, {}", tile.tile.0, tile.tile.1));
                            ui.weak(format!("plane {}", self.interaction.view_plane));
                            ui.separator();
                        }
                        let tool = self.prefs.tool;
                        if let Some(tile) = pick.tile.map(|p| p.tile) {
                            let item = |ui: &mut egui::Ui, text: String, tip: &str| {
                                ui.button(text).on_hover_text(tip).clicked()
                            };
                            match tool {
                                crate::paint::Tool::Select => {
                                    if item(
                                        ui,
                                        format!("{}  Select tile", icons::SELECT_TILES),
                                        "Add this tile to the selection",
                                    ) {
                                        outcome.action = Some(MenuAction::SelectTile(tile));
                                        outcome.close = true;
                                    }
                                    if item(
                                        ui,
                                        format!("{}  Use as brush", icons::EYEDROPPER),
                                        "Take this tile's floor, shape and height into the brush",
                                    ) {
                                        outcome.action = Some(MenuAction::Eyedrop(tile));
                                        outcome.close = true;
                                    }
                                    if self.tile_now(tile).is_some_and(|v| v.overlay != 0)
                                        && item(
                                            ui,
                                            format!("{}  Clear overlay", icons::DELETE),
                                            "Remove the overlay (path, floor) from this tile",
                                        )
                                    {
                                        outcome.action = Some(MenuAction::ClearOverlay(tile));
                                        outcome.close = true;
                                    }
                                }
                                crate::paint::Tool::Paint => {
                                    let empty = self.prefs.brush.is_empty();
                                    if ui
                                        .add_enabled(
                                            !empty,
                                            egui::Button::new(format!(
                                                "{}  Paint here",
                                                crate::app::workspace::tool_icon(tool)
                                            )),
                                        )
                                        .on_hover_text(
                                            "Apply the brush to the tile under the cursor",
                                        )
                                        .clicked()
                                    {
                                        outcome.action = Some(MenuAction::ApplyBrush(tile));
                                        outcome.close = true;
                                    }
                                    let selected = self.interaction.selection_tiles.len();
                                    if selected > 0
                                        && !empty
                                        && item(
                                            ui,
                                            format!(
                                                "{}  Fill {selected} selected tiles",
                                                icons::SELECT_TILES
                                            ),
                                            "Apply the brush to every selected tile",
                                        )
                                    {
                                        outcome.action = Some(MenuAction::FillSelection);
                                        outcome.close = true;
                                    }
                                }
                                crate::paint::Tool::Spline => {
                                    if item(
                                        ui,
                                        format!(
                                            "{}  Add path point here",
                                            crate::app::workspace::tool_icon(tool)
                                        ),
                                        "Add a control point to the path",
                                    ) {
                                        outcome.action = Some(MenuAction::AddPathPoint(tile));
                                        outcome.close = true;
                                    }
                                    if !self.interaction.spline.points.is_empty() {
                                        if item(
                                            ui,
                                            "Lay path (Enter)".to_owned(),
                                            "Lay the previewed path",
                                        ) {
                                            outcome.action = Some(MenuAction::LayPath);
                                            outcome.close = true;
                                        }
                                        if item(
                                            ui,
                                            "Clear path (Esc)".to_owned(),
                                            "Remove all control points",
                                        ) {
                                            outcome.action = Some(MenuAction::ClearPath);
                                            outcome.close = true;
                                        }
                                    }
                                }
                                crate::paint::Tool::Objects => {}
                                crate::paint::Tool::Height => {
                                    if item(
                                        ui,
                                        format!(
                                            "{}  {} here",
                                            crate::app::workspace::tool_icon(tool),
                                            self.prefs.height_tool.op.label()
                                        ),
                                        "Apply the height brush once at the tile under the cursor",
                                    ) {
                                        outcome.action = Some(MenuAction::ApplyBrush(tile));
                                        outcome.close = true;
                                    }
                                    let selected = self.interaction.selection_tiles.len();
                                    if selected > 0
                                        && item(
                                            ui,
                                            format!(
                                                "{}  {} {selected} selected tiles",
                                                icons::SELECT_TILES,
                                                self.prefs.height_tool.op.label()
                                            ),
                                            "Apply the height tool to every selected tile",
                                        )
                                    {
                                        outcome.action = Some(MenuAction::HeightSelection);
                                        outcome.close = true;
                                    }
                                }
                            }
                            if tool != crate::paint::Tool::Select
                                && item(
                                    ui,
                                    format!("{}  Copy tile to brush", icons::EYEDROPPER),
                                    "Take this tile's floor, shape and height into the brush (I)",
                                )
                            {
                                outcome.action = Some(MenuAction::Eyedrop(tile));
                                outcome.close = true;
                            }
                        }
                        if tool == crate::paint::Tool::Select
                            && (!self.interaction.selection_objects.is_empty()
                                || !self.interaction.selection_tiles.is_empty())
                            && ui.button("Clear selection").clicked()
                        {
                            outcome.action = Some(MenuAction::ClearSelection);
                            outcome.close = true;
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
