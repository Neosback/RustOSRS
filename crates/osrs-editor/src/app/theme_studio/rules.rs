//! Ordered area rules edit canonical tiles and validated plane values.
use eframe::egui;
use osrs_core::coords::{MapTile, PlaneIndex};
use osrs_hd::theme::{ThemeDefinition, ThemeOverrides, ThemeRule, ThemeTarget};
pub(super) fn draw(
    ui: &mut egui::Ui,
    theme: &mut ThemeDefinition,
    tile: MapTile,
    plane: PlaneIndex,
) {
    egui::CollapsingHeader::new("Local rules").show(ui, |ui| {
        ui.small("Later rules win for the controls they override. Lighting and atmosphere follow the camera's focal tile.");
        let mut remove = None;
        let mut swap = None;
        let len = theme.rules.len();
        for (index, rule) in theme.rules.iter_mut().enumerate() {
            ui.push_id(&rule.id, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.checkbox(&mut rule.enabled, "Enabled");
                    if ui.add_enabled(index > 0, egui::Button::new("Up")).clicked() { swap = Some((index, index - 1)); }
                    if ui.add_enabled(index + 1 < len, egui::Button::new("Down")).clicked() { swap = Some((index, index + 1)); }
                    if ui.button("Remove").clicked() { remove = Some(index); }
                });
                ui.text_edit_singleline(&mut rule.name);
                if let ThemeTarget::Area { min, max, min_plane, max_plane } = &mut rule.target {
                    egui::Grid::new("area-bounds").num_columns(3).show(ui, |ui| {
                        ui.label("Bounds"); ui.label("From"); ui.label("Through"); ui.end_row();
                        ui.label("Tile X"); ui.add(egui::DragValue::new(&mut min.x)); ui.add(egui::DragValue::new(&mut max.x)); ui.end_row();
                        ui.label("Tile Y"); ui.add(egui::DragValue::new(&mut min.y)); ui.add(egui::DragValue::new(&mut max.y)); ui.end_row();
                        let mut lo = min_plane.get(); let mut hi = max_plane.get();
                        ui.label("Plane"); ui.add(egui::DragValue::new(&mut lo).range(0..=3)); ui.add(egui::DragValue::new(&mut hi).range(0..=3)); ui.end_row();
                        if let Some(value) = PlaneIndex::new(lo) { *min_plane = value; }
                        if let Some(value) = PlaneIndex::new(hi) { *max_plane = value; }
                    });
                }
                for section in ["Lighting", "Sky & Fog", "Water", "Effects"] {
                    egui::CollapsingHeader::new(section).show(ui, |ui| super::controls::parameters(ui, &mut rule.values, section));
                }
                ui.separator();
            });
        }
        if let Some(index) = remove { theme.rules.remove(index); }
        else if let Some((a,b)) = swap { theme.rules.swap(a,b); }
        if ui.button("Add area around camera").clicked() {
            theme.rules.push(ThemeRule { id: super::custom_id(), name: format!("Area {}", len + 1), enabled: true,
                target: ThemeTarget::Area { min: MapTile::new(tile.x.saturating_sub(4), tile.y.saturating_sub(4)),
                    max: MapTile::new(tile.x.saturating_add(4), tile.y.saturating_add(4)), min_plane: plane, max_plane: plane },
                values: ThemeOverrides::default() });
        }
    });
}
