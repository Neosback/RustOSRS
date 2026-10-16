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

mod editor_menu;
mod effects;
mod game_menu;

use effects::ph_copy;

use super::EditorApp;
use crate::hud::Destination;
use crate::icons;
use crate::interact;
use crate::pick::{self, ObjectPick, Pick};
use crate::settings::ContextMenuStyle;
use eframe::egui;

/// What an editor-menu entry does.
#[derive(Debug, Clone)]
enum MenuAction {
    Select(ObjectPick),
    Focus(ObjectPick),
    TeleportHere((i32, i32)),
    SetTool(crate::paint::Tool),
    Eyedrop((i32, i32)),
    ApplyBrush((i32, i32)),
    SelectTile((i32, i32)),
    FillSelection,
    HeightSelection,
    AddPathPoint((i32, i32)),
    LayPath,
    ClearPath,
    ClearSelection,
    /// Act on this object (it becomes the selection first).
    ObjectOp(ObjectPick, ObjectOp),
    /// Select every loaded object with this definition id.
    SelectSimilar(u32),
    /// Open the object's wiki page.
    Wiki(ObjectPick),
    ClearOverlay((i32, i32)),
}

/// What the menu can do to an object.
#[derive(Debug, Clone, Copy)]
enum ObjectOp {
    Turn,
    Duplicate,
    Delete,
    Move,
}

impl EditorApp {
    /// Draw the open context menu in the chosen style.
    pub(super) fn context_menu(&mut self, ctx: &egui::Context) {
        let Some(position) = self.ui.context_menu else {
            return;
        };
        let pick = self.interaction.context_pick().clone();
        let outcome = match self.settings.prefs.context_menu {
            ContextMenuStyle::Game => self.game_menu(ctx, position, &pick),
            ContextMenuStyle::Editor => self.editor_menu(ctx, position, &pick),
        };
        if outcome.close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.ui.context_menu = None;
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
            MenuAction::Select(object) => self.select_only(object),
            MenuAction::Focus(object) => {
                self.select_only(object);
                self.focus_selection();
            }
            MenuAction::SetTool(tool) => self.set_tool(tool),
            MenuAction::Eyedrop(tile) => {
                if let Some(values) =
                    pick::tile_values(self.world.infos(), tile, self.interaction.view_plane())
                {
                    self.eyedrop(values);
                }
            }
            MenuAction::ApplyBrush(tile) => self.apply_brush_at(tile),
            MenuAction::SelectTile(tile) => {
                self.interaction.add_tile(tile);
            }
            MenuAction::FillSelection => self.fill_selection(),
            MenuAction::HeightSelection => self.height_on_selection(),
            MenuAction::AddPathPoint(tile) => {
                self.interaction
                    .add_spline_point((tile.0 as f32 + 0.5, tile.1 as f32 + 0.5));
            }
            MenuAction::LayPath => self.commit_spline(),
            MenuAction::ClearPath => self.interaction.clear_spline_points(),
            MenuAction::ClearSelection => self.clear_selection(),
            MenuAction::ObjectOp(object, op) => {
                self.select_only(object);
                match op {
                    ObjectOp::Turn => self.rotate_selected_objects(1),
                    ObjectOp::Duplicate => self.duplicate_selected_objects(),
                    ObjectOp::Delete => self.delete_selected_objects(),
                    ObjectOp::Move => self.set_tool(crate::paint::Tool::Objects),
                }
            }
            MenuAction::SelectSimilar(id) => self.select_similar(id),
            MenuAction::Wiki(object) => {
                if let Some(loc) = self
                    .world
                    .infos()
                    .get(&object.region)
                    .and_then(|info| info.locs.get(object.index))
                {
                    let url = crate::wiki::lookup_url(
                        loc.definition.identity.id.get(),
                        loc.definition.name.as_deref().unwrap_or(""),
                        loc.tile,
                        loc.plane,
                    );
                    if let Err(error) = crate::platform::open_url(&url) {
                        self.ui
                            .set_status(format!("could not open the wiki: {error}"));
                    }
                }
            }
            MenuAction::ClearOverlay(tile) => self.clear_overlay_at(tile),
            MenuAction::TeleportHere((x, y)) => {
                let plane = self.interaction.view_plane();
                self.teleport_to(Destination::Tile { x, y, plane });
                self.ui.notify(format!("Teleported to {x}, {y}"));
            }
        }
    }
}

/// What a menu produced this frame.
#[derive(Default)]
struct Outcome {
    effect: Option<interact::Effect>,
    action: Option<MenuAction>,
    close: bool,
}
