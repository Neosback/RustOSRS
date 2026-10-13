//! Applying interaction effects and the shared copy submenu.

use super::*;

pub(super) fn ph_copy() -> &'static str {
    egui_phosphor::regular::COPY
}

impl EditorApp {
    /// Carry out a chosen menu entry. Game actions are inert until the editor talks to a server;
    /// walking here moves the camera there.
    pub(in crate::app) fn apply_effect(&mut self, effect: interact::Effect) {
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
    pub(in crate::app) fn copy_menu(&self, ui: &mut egui::Ui, pick: &Pick) -> bool {
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
