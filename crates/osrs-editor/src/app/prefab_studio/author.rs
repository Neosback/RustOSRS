//! Room labels and sparse ownership are prefab metadata; ordinary tools still own map edits.
use super::*;
use osrs_edit::{PrefabCell, PrefabRoom};
use std::collections::BTreeSet;

pub(super) fn show(studio: &mut Studio, ui: &mut egui::Ui) {
    let viewer = studio.stage.viewer();
    let plane = viewer.interaction.view_plane();
    let selected: BTreeSet<_> = viewer
        .interaction
        .selection_tiles()
        .iter()
        .filter_map(|&(x, y)| {
            Some(PrefabCell {
                x: u16::try_from(x - studio.origin.x).ok()?,
                y: u16::try_from(y - studio.origin.y).ok()?,
                plane,
            })
        })
        .collect();
    let inspected = viewer.interaction.inspect_tile().and_then(|(x, y)| {
        Some(PrefabCell {
            x: u16::try_from(x - studio.origin.x).ok()?,
            y: u16::try_from(y - studio.origin.y).ok()?,
            plane,
        })
    });
    let Some(building) = &mut studio.original.building else {
        return;
    };
    let before = building.clone();
    ui.collapsing("Rooms, roles and generation locks", |ui| {
        ui.label("Select floor tiles in the scene, then assign or split a room. Room labels do not move walls.");
        if building.semantics.rooms.is_empty() && ui.button("Start room labels from owned floors").clicked() {
            for p in 0..studio.original.planes {
                let cells: Vec<_> = building.floors.iter().filter(|c| c.plane == p).copied().collect();
                if !cells.is_empty() { building.semantics.rooms.push(PrefabRoom { id:u16::from(p),role:"general".into(),cells,human:false }); }
            }
        }
        let mut split = None;
        let mut assign = None;
        for room in &mut building.semantics.rooms {
            ui.horizontal(|ui| {
                ui.label(format!("Room {} · {} tiles",room.id,room.cells.len()));
                if ui.text_edit_singleline(&mut room.role).changed() { room.human = true; }
            });
            ui.horizontal(|ui| {
                if ui.add_enabled(!selected.is_empty(),egui::Button::new("Assign selection")).clicked() { assign = Some(room.id); }
                if ui.add_enabled(!selected.is_empty(),egui::Button::new("Split selection")).clicked() { split = Some(room.id); }
                ui.weak(if room.human { "Human label" } else { "Inferred / unreviewed label" });
            });
        }
        if let Some(id) = split
            && let Err(e) = building.semantics.split(id,&selected) { studio.problem = Some(e.to_string()); }
        if let Some(id) = assign {
            let cells: Vec<_> = selected.iter().filter(|c| building.floors.contains(c)).copied().collect();
            for r in &mut building.semantics.rooms { r.cells.retain(|c| !cells.contains(c)); if r.id == id { r.cells.extend(cells.iter().copied()); r.human = true; } }
            building.semantics.rooms.retain(|r| !r.cells.is_empty());
            building.semantics.portals.clear();
        }
        if building.semantics.rooms.len() >= 2 && ui.button("Merge rooms touched by selection").clicked() {
            let ids: Vec<_> = building.semantics.rooms.iter().filter(|r| r.cells.iter().any(|c| selected.contains(c))).map(|r|r.id).collect();
            if let Some(&first) = ids.first() { for &second in &ids[1..] { if let Err(e) = building.semantics.merge(first,second) { studio.problem = Some(e.to_string()); } } }
        }
        ui.separator();
        if ui.button("Use inspected tile as placement anchor").clicked() {
            if let Some(c) = inspected.filter(|c| c.x < studio.original.width && c.y < studio.original.height) { building.semantics.anchor = Some(c); }
            else { studio.problem = Some("Inspect a tile inside this prefab first".into()); }
        }
        if ui.button("Use centered placement").clicked() { building.semantics.anchor = None; }
        if let Some(c) = building.semantics.anchor { ui.label(format!("Placement anchor {},{} · floor {}",c.x,c.y,c.plane)); }
        ui.weak("Generation locks preserve these occurrences when completing or repairing. Unlock to replace them.");
        let specs = viewer.selected_specs();
        for (i,o) in studio.original.objects.iter().enumerate() {
            let selected = specs.iter().any(|p| p.object_id == o.id && p.loc_type == o.kind && p.orientation == o.orientation && p.tile == (studio.origin.x+i32::from(o.x),studio.origin.y+i32::from(o.y)) && p.plane == o.plane);
            if selected {
                let mut locked = building.semantics.locked_parts.contains(&i);
                if ui.checkbox(&mut locked,format!("Keep object {} at {},{}",o.id,o.x,o.y)).changed() {
                    building.semantics.locked_parts.retain(|&n| n != i);
                    if locked { building.semantics.locked_parts.push(i); }
                }
            }
        }
    });
    if *building != before {
        building.reviewed = false;
        studio.session.metadata_changed();
    }
    if ui
        .checkbox(
            &mut building.reviewed,
            "I reviewed room ownership, roles and included pieces",
        )
        .changed()
    {
        studio.session.metadata_changed();
    }
}
