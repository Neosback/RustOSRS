//! Full-frame workspace navigation. Tools reuse the existing authoring controllers and gateway.
use super::{EditorApp, catalog::CatalogView};
use eframe::egui;

#[derive(Default, PartialEq, Eq)]
enum Page {
    #[default]
    Library,
    Builder,
    Wfc,
}
pub(in crate::app) struct Workspace {
    pub(in crate::app) open: bool,
    page: Page,
    catalog: CatalogView,
    seed: u64,
}
impl Default for Workspace {
    fn default() -> Self {
        Self {
            open: true,
            page: Page::Library,
            catalog: CatalogView::default(),
            seed: 0,
        }
    }
}
impl Workspace {
    pub(super) fn show_builder(&mut self) {
        self.page = Page::Builder;
    }

    pub(super) fn show(
        &mut self,
        viewer: &mut EditorApp,
        current: &str,
        original: &osrs_edit::Prefab,
        ui: &mut egui::Ui,
        library: &mut dyn FnMut(&mut egui::Ui),
    ) {
        if !self.open {
            return;
        }
        egui::Panel::left("prefab-workspace-tools")
            .default_size(300.0).size_range(240.0..=520.0).resizable(true)
            .show(ui, |ui| {
                ui.strong("This building in 3D");
                ui.weak("Change objects, floors and heights with the scene tools. Nothing is kept until you press Save.");
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Frame scene").clicked() { frame_scene(viewer); }
                    if ui.button("Top view").clicked() {
                        viewer.interaction.set_camera_pose(0.0, 1.5, None);
                    }
                    if ui.button("3D view").clicked() {
                        viewer.interaction.set_camera_pose(0.6, 0.75, None);
                    }
                });
                ui.weak("Right-drag: orbit · wheel: zoom. Plane and roof controls are above the scene.");
                ui.separator();
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.page, Page::Library, "Library");
                    ui.selectable_value(&mut self.page, Page::Builder, "Build new");
                    ui.selectable_value(&mut self.page, Page::Wfc, "Floor patterns (advanced)");
                });
                ui.separator();
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    match self.page {
                        Page::Library => {
                            viewer.prefabs.page = crate::app::prefab_runtime::Page::Library;
                            let _ = current;
                            library(ui);
                        },
                        Page::Builder => {
                            super::blueprint::controls(viewer, original, ui);
                            viewer.prefabs.page = crate::app::prefab_runtime::Page::Build;
                            if ui.button("Use scene footprint").clicked()
                                && let Some(prefab) = viewer.prefabs.entry(current).map(|e| e.prefab.clone()) {
                                viewer.prefabs.build.form.width = prefab.width.clamp(6, crate::app::prefab_runtime::MAX_SITE);
                                viewer.prefabs.build.form.height = prefab.height.clamp(6, crate::app::prefab_runtime::MAX_SITE);
                                viewer.prefabs.build.site = Some(osrs_core::coords::MapTile::new(
                                    super::CENTRE.0 - i32::from(prefab.width)/2,
                                    super::CENTRE.1 - i32::from(prefab.height)/2));
                                viewer.prefabs.build.site_plane = viewer.interaction.view_plane();
                            }
                            crate::app::prefab_panel::builder(viewer, ui);
                        },
                        Page::Wfc => {
                            viewer.prefabs.page = crate::app::prefab_runtime::Page::Library;
                            ui.group(|ui| {
                                ui.label("Want to know what the floor-pattern builder (WFC) makes of this building?");
                                if ui.button("Show me what WFC sees (Floor patterns tab)").clicked() {
                                    viewer.prefabs.studio_command = Some(super::SceneCommand::Workshop(crate::app::workshop::Tab::Wfc));
                                }
                                ui.weak("It runs WFC on this building in the Workshop and shows its tile kinds, what it ignores, and a test.");
                            });
                            ui.label("Pattern seed");
                            ui.add(egui::DragValue::new(&mut self.seed));
                            self.catalog.show(ui, self.seed, viewer, current);
                        }
                    }
                });
            });
    }
}
fn frame_scene(viewer: &mut EditorApp) {
    let objects = viewer.session.document().object_entries();
    let (cx, cy, side) = if let Some(first) = objects.first() {
        let mut min = first.position.tile;
        let mut max = min;
        for o in &objects {
            let t = o.position.tile;
            min.x = min.x.min(t.x);
            min.y = min.y.min(t.y);
            max.x = max.x.max(t.x);
            max.y = max.y.max(t.y);
        }
        (
            (min.x + max.x) / 2,
            (min.y + max.y) / 2,
            (max.x - min.x + 1).max(max.y - min.y + 1) as f32,
        )
    } else {
        (super::CENTRE.0, super::CENTRE.1, 12.0)
    };
    let origin = viewer.renderer.render_origin();
    viewer.interaction.focus_camera(
        [
            (cx * 128 + 64 - origin.0) as f32,
            -(side * 40.0).min(900.0),
            (cy * 128 + 64 - origin.1) as f32,
        ],
        (side * 128.0 * 1.5).clamp(900.0, 9000.0),
    );
}
