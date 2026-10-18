//! The gallery's list: search, filters, sorting and the cards.

use super::{Sort, SourceFilter};
use crate::app::{EditorApp, prefab_panel::preview};
use eframe::egui;
use osrs_edit::{Prefab, PrefabSource};
use std::collections::BTreeSet;

const CARD: f32 = 150.0;
const GAP: f32 = 8.0;

fn source_of(prefab: &Prefab) -> SourceFilter {
    match prefab.source {
        PrefabSource::Captured { .. } => SourceFilter::Captured,
        PrefabSource::Generated { .. } => SourceFilter::Generated,
        PrefabSource::Authored => SourceFilter::Authored,
    }
}

fn label(source: SourceFilter) -> &'static str {
    match source {
        SourceFilter::All => "All",
        SourceFilter::Captured => "Captured",
        SourceFilter::Generated => "Generated",
        SourceFilter::Authored => "By hand",
    }
}

pub(super) fn show(app: &mut EditorApp, ui: &mut egui::Ui) {
    let width = ui.available_width();
    ui.add_sized(
        [width, 24.0],
        egui::TextEdit::singleline(&mut app.prefabs.filter).hint_text("Search name, tag or note"),
    );
    ui.horizontal_wrapped(|ui| {
        let gallery = &mut app.prefabs.gallery;
        for source in [
            SourceFilter::All,
            SourceFilter::Captured,
            SourceFilter::Generated,
            SourceFilter::Authored,
        ] {
            ui.selectable_value(&mut gallery.source, source, label(source));
        }
    });
    let tags: BTreeSet<String> = app
        .prefabs
        .entries()
        .iter()
        .flat_map(|entry| entry.prefab.tags.iter().cloned())
        .collect();
    if !tags.is_empty() {
        ui.horizontal_wrapped(|ui| {
            for tag in tags {
                let on = app.prefabs.gallery.tag.as_deref() == Some(tag.as_str());
                if ui.selectable_label(on, &tag).clicked() {
                    app.prefabs.gallery.tag = if on { None } else { Some(tag) };
                }
            }
        });
    }
    ui.horizontal(|ui| {
        ui.label("Sort");
        let gallery = &mut app.prefabs.gallery;
        ui.selectable_value(&mut gallery.sort, Sort::Name, "Name");
        ui.selectable_value(&mut gallery.sort, Sort::Size, "Size");
        ui.selectable_value(&mut gallery.sort, Sort::Objects, "Objects");
    });
    ui.horizontal(|ui| {
        let missing = app
            .prefabs
            .entries()
            .iter()
            .filter(|entry| !super::has_still(app, &entry.file))
            .count();
        let rendering = app.prefabs.gallery.queue.len();
        if rendering > 0 {
            ui.spinner();
            ui.weak(format!("Rendering previews, {rendering} to go"));
            if ui.small_button("Stop").clicked() {
                app.prefabs.gallery.queue.clear();
            }
        } else if missing > 0
            && ui
                .button(format!("Render {missing} preview(s)"))
                .on_hover_text("Shows each prefab on the stage once and keeps a picture of it")
                .clicked()
        {
            super::queue_missing_stills(app);
        }
    });
    ui.separator();

    let (source, tag, sort) = {
        let gallery = &app.prefabs.gallery;
        (gallery.source, gallery.tag.clone(), gallery.sort)
    };
    let mut shown: Vec<(String, String, Prefab)> = app
        .prefabs
        .visible()
        .into_iter()
        .filter(|entry| source == SourceFilter::All || source_of(&entry.prefab) == source)
        .filter(|entry| {
            tag.as_ref()
                .is_none_or(|tag| entry.prefab.tags.contains(tag))
        })
        .map(|entry| {
            (
                entry.file.clone(),
                entry.prefab.name.clone(),
                entry.prefab.clone(),
            )
        })
        .collect();
    match sort {
        Sort::Name => {}
        Sort::Size => shown
            .sort_by_key(|(_, _, p)| std::cmp::Reverse(u32::from(p.width) * u32::from(p.height))),
        Sort::Objects => shown.sort_by_key(|(_, _, p)| std::cmp::Reverse(p.objects.len())),
    }
    if shown.is_empty() {
        ui.weak(if app.prefabs.entries().is_empty() {
            "The library is empty. Capture a building from the map, or generate one on the Build page."
        } else {
            "Nothing matches."
        });
        return;
    }
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let columns = (((ui.available_width() + GAP) / (CARD + GAP)) as usize).max(1);
            for row in shown.chunks(columns) {
                ui.horizontal_top(|ui| {
                    for (file, name, prefab) in row {
                        card(app, ui, file, name, prefab);
                    }
                });
                ui.add_space(GAP);
            }
        });
}

fn card(app: &mut EditorApp, ui: &mut egui::Ui, file: &str, name: &str, prefab: &Prefab) {
    let selected = app.prefabs.selected.as_deref() == Some(file);
    let thumb = super::still_of(app, file);
    let frame = egui::Frame::new()
        .inner_margin(egui::Margin::same(4))
        .corner_radius(5.0)
        .stroke(if selected {
            ui.visuals().selection.stroke
        } else {
            egui::Stroke::NONE
        })
        .fill(if selected {
            ui.visuals().selection.bg_fill.gamma_multiply(0.35)
        } else {
            ui.visuals().faint_bg_color
        });
    let response = frame
        .show(ui, |ui| {
            ui.set_width(CARD - 8.0);
            ui.vertical(|ui| {
                match &thumb {
                    Some(texture) => {
                        let size = egui::vec2(CARD - 8.0, (CARD - 8.0) * 0.75);
                        ui.add(egui::Image::new((texture.id(), size)).corner_radius(3.0));
                    }
                    None => {
                        preview::show(ui, prefab, CARD - 8.0);
                    }
                }
                ui.add(egui::Label::new(egui::RichText::new(name).strong()).truncate());
                ui.add(
                    egui::Label::new(egui::RichText::new(prefab.summary()).weak().small())
                        .truncate(),
                );
            });
        })
        .response
        .interact(egui::Sense::click());
    if response.clicked() {
        app.prefabs.selected = Some(file.to_owned());
        app.prefabs.gallery.turns = 0;
    }
    if response.double_clicked() {
        crate::app::prefab_panel::arm(app, file);
    }
}
