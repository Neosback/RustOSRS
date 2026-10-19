//! The world map's legend of map elements.

use super::*;

impl EditorApp {
    /// The key: every kind of icon in this area with a switch, plus search results.
    pub(super) fn world_map_key(&mut self, ui: &mut egui::Ui, catalog: &Catalog) {
        let area = self.ui.world_map.area;
        let Some(icons_in_area) = self.assets.icons(area) else {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.weak("Reading icons...");
            });
            return;
        };
        let names = self.assets.icon_names(area).unwrap_or_default();
        // One key row per category present in the area (the client's key is by category).
        let mut kinds: Vec<(i32, String, i32)> = Vec::new();
        let mut seen = HashSet::new();
        for icon in icons_in_area.iter() {
            let Some(element) = catalog.elements.get(&icon.element) else {
                continue;
            };
            if element.sprite < 0 || !seen.insert(element.category) {
                continue;
            }
            if let Some(name) = element_name(catalog, &names, icon.element) {
                kinds.push((element.category, name, element.sprite));
            }
        }
        kinds.sort_by(|a, b| a.1.cmp(&b.1));

        let query = self.ui.world_map.search.trim().to_lowercase();
        let mut jump_coord = None;
        if !query.is_empty() {
            ui.strong("Search results");
            let mut shown = 0;
            let mut seen_names = HashSet::new();
            for icon in icons_in_area.iter() {
                let Some(name) = element_name(catalog, &names, icon.element) else {
                    continue;
                };
                if !name.to_lowercase().contains(&query) || !seen_names.insert(name.clone()) {
                    continue;
                }
                if ui.selectable_label(false, name.as_str()).clicked() {
                    jump_coord = Some((f64::from(icon.x), f64::from(icon.y)));
                }
                shown += 1;
                if shown >= 14 {
                    break;
                }
            }
            if shown < 14 {
                for label in crate::app::map_labels::all_labels() {
                    let clean = label.clean_name();
                    if clean.to_lowercase().contains(&query) && seen_names.insert(clean.clone()) {
                        if ui.selectable_label(false, format!("📍 {clean}")).clicked() {
                            jump_coord = Some((label.world_x as f64, label.world_y as f64));
                        }
                        shown += 1;
                        if shown >= 14 {
                            break;
                        }
                    }
                }
            }
            if shown == 0 {
                ui.weak("No matches");
            }
            ui.separator();
        }
        if let Some((x, y)) = jump_coord {
            self.ui.world_map.center = (x, y);
            self.ui.world_map.zoom_target = 4.0;
        }

        ui.strong("Key");
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (id, name, sprite) in kinds {
                ui.horizontal(|ui| {
                    let mut shown = !self.ui.world_map.hidden.contains(&id);
                    if ui.checkbox(&mut shown, "").changed() {
                        if shown {
                            self.ui.world_map.hidden.remove(&id);
                        } else {
                            self.ui.world_map.hidden.insert(id);
                        }
                    }
                    if sprite >= 0
                        && let Some(texture) = self.assets.sprite(sprite as u32)
                    {
                        ui.image((texture.id(), texture.size_vec2()));
                    }
                    ui.label(name);
                });
            }
        });
    }
}
