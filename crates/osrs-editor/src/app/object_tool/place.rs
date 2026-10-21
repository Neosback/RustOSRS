//! Placing new objects from the cache: search the object catalog, arm one, click tiles to place.
//!
//! Placement is just another object action: the click builds one `LocSpec` and goes through
//! `edit_objects`, so it commits once to `EditorSession`, projects to the world worker, and is one
//! undo step like every other object edit.

use super::*;
use crate::interact;
use eframe::egui::{Color32, Stroke};
use osrs_world::{ObjectCatalog, ObjectEntry};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};

/// How many search hits the drawer lists.
const MAX_RESULTS: usize = 40;

/// What a click on the map places.
pub(in crate::app) struct PlaceTarget {
    entry: ObjectEntry,
    loc_type: u8,
    orientation: u8,
}

#[derive(Default)]
pub(in crate::app) struct PlaceState {
    catalog: Option<Arc<ObjectCatalog>>,
    loading: Option<Receiver<Result<ObjectCatalog, String>>>,
    error: Option<String>,
    search: String,
    target: Option<PlaceTarget>,
    symbols_applied: bool,
    /// The name-book generation the catalog's custom names were taken from.
    names_generation: Option<u64>,
}

/// The client's name for a loc type, for the type picker.
fn loc_type_label(loc_type: u8) -> &'static str {
    match loc_type {
        0 => "Wall",
        1 => "Wall corner",
        2 => "Wall L-corner",
        3 => "Wall corner (inner)",
        4..=8 => "Wall decoration",
        9 => "Diagonal wall",
        10 => "Scenery",
        11 => "Diagonal scenery",
        12..=21 => "Roof",
        22 => "Floor decoration",
        _ => "Other",
    }
}

impl PlaceState {
    pub(in crate::app) fn is_armed(&self) -> bool {
        self.target.is_some()
    }

    /// The catalog's entry for an object (None until the catalog has loaded).
    pub(in crate::app) fn catalog_entry(&self, id: u32) -> Option<&osrs_world::ObjectEntry> {
        self.catalog.as_ref().and_then(|catalog| catalog.get(id))
    }

    /// An object's base size in tiles from the catalog; one tile when the catalog has not loaded
    /// or does not list it (unnamed boundary pieces and decoration are one tile anyway).
    pub(in crate::app) fn object_size(&self, id: u32) -> (u8, u8) {
        self.known_object_size(id).unwrap_or((1, 1))
    }
    pub(in crate::app) fn known_object_size(&self, id: u32) -> Option<(u8, u8)> {
        self.catalog.as_ref()?.definition_size(id)
    }

    /// Cancel placing. Returns whether anything was armed.
    pub(in crate::app) fn disarm(&mut self) -> bool {
        self.target.take().is_some()
    }

    fn arm(&mut self, entry: &ObjectEntry) {
        let orientation = self.target.as_ref().map_or(0, |target| target.orientation);
        self.target = Some(PlaceTarget {
            loc_type: entry.default_loc_type(),
            entry: entry.clone(),
            orientation,
        });
    }
}

impl EditorApp {
    /// Read the object catalog off the UI thread (the browser build has no worker threads yet).
    #[cfg(not(target_arch = "wasm32"))]
    pub(in crate::app) fn start_catalog_loader(&mut self, source: osrs_cache::source::CacheSource) {
        self.placer = PlaceState::default();
        let (sender, receiver) = std::sync::mpsc::channel();
        self.placer.loading = Some(receiver);
        std::thread::spawn(move || {
            let result = osrs_world::load_object_catalog(source).map_err(|error| error.to_string());
            let _ = sender.send(result);
        });
    }

    #[cfg(target_arch = "wasm32")]
    pub(in crate::app) fn start_catalog_loader(
        &mut self,
        _source: osrs_cache::source::CacheSource,
    ) {
        self.placer = PlaceState::default();
        self.placer.error = Some("Object search is not available in the browser yet.".to_owned());
    }

    pub(in crate::app) fn poll_catalog(&mut self) {
        if let Some(receiver) = &self.placer.loading {
            match receiver.try_recv() {
                Ok(Ok(catalog)) => {
                    self.placer.catalog = Some(Arc::new(catalog));
                    self.placer.symbols_applied = false;
                    self.placer.names_generation = None;
                    self.placer.loading = None;
                }
                Ok(Err(error)) => {
                    self.placer.error = Some(error);
                    self.placer.loading = None;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    self.placer.error = Some("The object list stopped loading.".to_owned());
                    self.placer.loading = None;
                }
            }
        }
        if !self.placer.symbols_applied
            && let (Some(catalog), Some(book)) = (&mut self.placer.catalog, self.names.book())
        {
            Arc::make_mut(catalog).apply_symbols(|id| book.object(id).map(str::to_owned));
            self.placer.symbols_applied = true;
        }
        // Names people gave objects: again whenever the name book changes.
        let generation = self.names.custom().generation();
        if self.placer.names_generation != Some(generation)
            && let Some(catalog) = &mut self.placer.catalog
        {
            let book = self.names.custom().book();
            Arc::make_mut(catalog).apply_custom(|id| {
                book.get(id).map_or((None, None), |entry| {
                    (entry.name.clone(), entry.role.clone())
                })
            });
            self.placer.names_generation = Some(generation);
        }
    }

    /// The drawer's "Place object" group: search, pick, and the armed object's controls.
    pub(super) fn place_group(&mut self, ui: &mut egui::Ui) {
        ui.set_width((ui.ctx().content_rect().width() - 32.0).clamp(120.0, 250.0));
        if let Some(error) = &self.placer.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
            return;
        }
        let Some(catalog) = self.placer.catalog.clone() else {
            ui.horizontal_wrapped(|ui| {
                ui.spinner();
                ui.weak("Loading objects...");
            });
            return;
        };
        ui.add(
            egui::TextEdit::singleline(&mut self.placer.search)
                .hint_text("Search name, id, symbol, slot:wall…")
                .desired_width(f32::INFINITY),
        );
        let hits = catalog.search(&self.placer.search, MAX_RESULTS);
        if !hits.is_empty() {
            egui::ScrollArea::vertical()
                .id_salt("place_results")
                .max_height(86.0)
                .show(ui, |ui| {
                    for entry in hits {
                        let armed = self
                            .placer
                            .target
                            .as_ref()
                            .is_some_and(|target| target.entry.id == entry.id);
                        if ui
                            .selectable_label(
                                armed,
                                format!("{}  ({})", entry.display_name(), entry.id),
                            )
                            .clicked()
                        {
                            self.placer.arm(entry);
                        }
                    }
                });
        }
        self.place_target_controls(ui);
    }

    fn place_target_controls(&mut self, ui: &mut egui::Ui) {
        let Some(target) = &mut self.placer.target else {
            return;
        };
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(target.entry.display_name()).strong());
            if target.entry.loc_types.len() > 1 {
                egui::ComboBox::from_id_salt("place_loc_type")
                    .selected_text(loc_type_label(target.loc_type))
                    .show_ui(ui, |ui| {
                        for &loc_type in &target.entry.loc_types {
                            ui.selectable_value(
                                &mut target.loc_type,
                                loc_type,
                                format!("{} ({loc_type})", loc_type_label(loc_type)),
                            );
                        }
                    });
            }
        });
        let mut stop = false;
        ui.horizontal_wrapped(|ui| {
            if ui
                .button(egui_phosphor::regular::ARROW_COUNTER_CLOCKWISE)
                .clicked()
            {
                target.orientation = (target.orientation + 3) & 3;
            }
            if ui.button(crate::icons::ROTATE).clicked() {
                target.orientation = (target.orientation + 1) & 3;
            }
            ui.weak(format!("facing {}", target.orientation));
            stop = ui.button("Stop").clicked();
        });
        if stop {
            self.placer.target = None;
        } else {
            ui.weak("Click a tile to place. Esc stops.");
        }
    }

    /// Turn the armed object a quarter turn. Returns whether one was armed.
    pub(in crate::app) fn rotate_placement(&mut self, quarters: i8) -> bool {
        let Some(target) = &mut self.placer.target else {
            return false;
        };
        target.orientation = (target.orientation + quarters.rem_euclid(4) as u8) & 3;
        true
    }

    /// Place the armed object on the hovered tile (one undo step). Stays armed for the next one.
    pub(in crate::app) fn place_armed_object(&mut self) {
        let Some(target) = &self.placer.target else {
            return;
        };
        let Some(pick) = self.interaction.hover().tile else {
            return;
        };
        let (entry, loc_type, orientation) =
            (target.entry.clone(), target.loc_type, target.orientation);
        self.place_object(&entry, loc_type, orientation, pick.tile);
    }

    /// Commit one new object at `tile` on the viewed plane.
    fn place_object(
        &mut self,
        entry: &ObjectEntry,
        loc_type: u8,
        orientation: u8,
        tile: (i32, i32),
    ) {
        let spec = LocSpec {
            object_id: entry.id,
            tile,
            plane: self.interaction.view_plane(),
            loc_type,
            orientation,
        };
        self.edit_objects(
            Vec::new(),
            vec![spec],
            &format!("Placed {}", entry.display_name()),
        );
    }

    /// `place ID X Y [ORIENTATION]` from the script language. `Ok(false)` while the catalog is
    /// still loading.
    pub(in crate::app) fn script_place(
        &mut self,
        id: u32,
        tile: (i32, i32),
        orientation: u8,
    ) -> Result<bool, String> {
        if let Some(error) = &self.placer.error {
            return Err(error.clone());
        }
        let Some(catalog) = self.placer.catalog.clone() else {
            return Ok(false);
        };
        let entry = catalog
            .get(id)
            .ok_or_else(|| format!("object {id} is not a placeable object"))?;
        self.place_object(entry, entry.default_loc_type(), orientation & 3, tile);
        Ok(true)
    }

    /// The footprint of the armed object under the cursor.
    pub(in crate::app) fn draw_place_ghost(
        &self,
        painter: &egui::Painter,
        viewport: egui::Rect,
        pixels_per_point: f32,
        width: u32,
        height: u32,
    ) {
        let (Some(target), Some(pick)) = (&self.placer.target, self.interaction.hover().tile)
        else {
            return;
        };
        let plane = self.interaction.view_plane();
        let (size_x, size_y) = target.entry.footprint(target.orientation);
        let fill = Color32::from_rgba_unmultiplied(120, 220, 140, 70);
        let outline = Stroke::new(1.5, Color32::from_rgb(120, 220, 140));
        for dx in 0..i32::from(size_x) {
            for dy in 0..i32::from(size_y) {
                let tile = (pick.tile.0 + dx, pick.tile.1 + dy);
                let Some(corners) = interact::tile_corners(self.world.infos(), tile, plane) else {
                    continue;
                };
                let points: Vec<egui::Pos2> = corners
                    .iter()
                    .filter_map(|c| {
                        self.screen_point(*c, viewport, pixels_per_point, width, height)
                    })
                    .collect();
                if points.len() == 4 {
                    painter.add(egui::Shape::convex_polygon(points, fill, outline));
                }
            }
        }
    }
}
