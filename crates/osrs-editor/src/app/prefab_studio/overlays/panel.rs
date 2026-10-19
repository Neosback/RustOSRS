//! "What belongs to this building": the window that explains and edits which ground tiles are
//! part of a saved building and which pieces go together. Plain words only; the marks it explains
//! are drawn in the scene by [`super::draw`].

use super::*;
use egui::RichText;

/// A coloured dot or line, then what it means.
fn legend(ui: &mut egui::Ui, color: Color32, text: &str) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
        ui.painter().rect_filled(rect.shrink(2.0), 3.0, color);
        ui.label(text);
    });
}

/// How two connected pieces are described, with the pieces' names.
fn sentence(kind: &str, from: &str, to: &str) -> String {
    match kind {
        "Continues" => format!("{from} carries on as {to} (one wall run)"),
        "CornerArm" => format!("{from} and {to} make a corner"),
        "Decorates" => format!("{from} is fixed to {to}"),
        "AgainstWall" => format!("{from} stands against {to}"),
        "RoofAbove" => format!("{from} is the roof over {to}"),
        "StairPair" => format!("{from} and {to} are the two ends of one stair"),
        "attached" => format!("{from} is attached to {to} (you linked them)"),
        other => format!("{from} and {other} {to}"),
    }
}

fn why(evidence: &str) -> &'static str {
    match evidence {
        "BoundaryIncident" => "the two meet at the same wall corner",
        "SameAnchorBoundary" => "they share the same spot on a wall",
        "FootprintInside" => "one stands inside the other's footprint",
        "AboveFloor" => "one sits directly above the other",
        "StairVerb" => "both have a Climb option",
        "human" => "you said so",
        _ => "the editor noticed it in the map",
    }
}

pub(in crate::app) fn panel(studio: &mut Studio, ctx: &egui::Context) {
    egui::Window::new("What belongs to this building")
        .default_width(380.0)
        .show(ctx, |ui| {
            let Some(b) = &mut studio.original.building else {
                ui.label(
                    "This is a plain rectangle of map, not a captured building, so there is \
                     nothing to say about what belongs to it. Capture a building (Prefabs, Capture) \
                     to get this.",
                );
                return;
            };
            ui.weak(
                "A saved building remembers which ground belongs to it and which pieces go \
                 together. When you place it on a map, only its own ground is replaced; the grass \
                 around it stays as it was. The marks in the scene show what is remembered.",
            );
            ui.add_space(4.0);
            ui.label(RichText::new("Ground").strong());
            ui.label(format!(
                "{} floor tiles belong to this building. The ground at {} corners is levelled to \
                 fit when you place it.",
                b.floors.len(),
                b.support.len()
            ));
            legend(ui, Color32::from_rgb(80, 180, 255), "Blue outline: a floor tile that belongs to it");
            legend(ui, Color32::ORANGE, "Orange dots: ground corners that are levelled to fit");
            legend(ui, Color32::LIGHT_GREEN, "Green ring: a piece inside the building");
            legend(ui, Color32::LIGHT_RED, "Red ring: a piece outside the walls that goes with it (a lamp or sign). The ground under it is not changed");
            legend(ui, Color32::from_rgb(200, 120, 255), "Purple ring: a roof piece");
            legend(ui, Color32::YELLOW, "Yellow line: the way in, and the step outside it");
            ui.checkbox(
                &mut studio.auto_own_painted_floors,
                "Floor I paint counts as part of the building",
            )
            .on_hover_text(
                "When you save, floor tiles you painted inside the building's area are added to it, \
                 and the building's area grows to cover them.",
            );
            let viewer = studio.stage.viewer();
            let specs = viewer.selected_specs();
            // Names for the pieces, before the overlay is borrowed.
            let pieces: Vec<(u32, String)> = viewer
                .prefabs
                .studio_overlay
                .as_ref()
                .map(|o| {
                    o.prefab
                        .objects
                        .iter()
                        .map(|object| {
                            let cache = viewer
                                .placer
                                .catalog_entry(object.id)
                                .map(|e| e.name.clone())
                                .unwrap_or_default();
                            let text = viewer.names.resolve(object.id, Some(&cache)).text;
                            (object.id, text)
                        })
                        .collect()
                })
                .unwrap_or_default();
            let tile = viewer.interaction.inspect_tile();
            let plane = viewer.interaction.view_plane();
            let Some(overlay) = viewer.prefabs.studio_overlay.as_mut() else {
                return;
            };
            ui.add_space(4.0);
            ui.label(RichText::new("About this building").strong());
            for (key, title, hint) in [
                ("human:archetype", "What it is", "house, inn, bank, church..."),
                ("human:quality", "Your note", "anything worth remembering"),
            ] {
                let mut value = b.labels.get(key).cloned().unwrap_or_default();
                ui.horizontal(|ui| {
                    ui.label(title);
                    if ui
                        .add(egui::TextEdit::singleline(&mut value).hint_text(hint))
                        .changed()
                    {
                        b.labels.insert(key.into(), value);
                        studio.session.metadata_changed();
                    }
                });
            }
            ui.add_space(4.0);
            ui.label(RichText::new("Change what counts as the building's floor").strong());
            match tile {
                None => {
                    ui.weak("Point at a tile with the Inspect tool, then come back here to say whether it belongs.");
                }
                Some((x, y)) => {
                    let (lx, ly) = (x - studio.origin.x, y - studio.origin.y);
                    let inside = lx >= 0
                        && ly >= 0
                        && lx < i32::from(studio.original.width)
                        && ly < i32::from(studio.original.height)
                        && plane < studio.original.planes;
                    if !inside {
                        ui.weak("The tile you are pointing at is outside this building's area.");
                    } else {
                        let cell = osrs_edit::PrefabCell { x: lx as u16, y: ly as u16, plane };
                        let mut owned = b.floors.contains(&cell);
                        if ui
                            .checkbox(
                                &mut owned,
                                format!("The tile you are pointing at ({}, {}) belongs to the building", cell.x, cell.y),
                            )
                            .changed()
                        {
                            set_floor(b, overlay, cell, owned);
                            studio.session.metadata_changed();
                        }
                    }
                }
            }
            ui.add_space(4.0);
            ui.label(RichText::new("Pieces that go together").strong());
            ui.weak(
                "The editor found these by looking at the building. Say \"Yes\" when a pair is \
                 right: confirmed pairs are what the AI is allowed to learn from. \"Forget\" only \
                 deletes the note; nothing in the building is removed.",
            );
            let unconfirmed = b.relations.iter().filter(|r| !r.human).count();
            if unconfirmed > 0
                && ui
                    .button(format!("Say yes to all {unconfirmed} guesses"))
                    .clicked()
            {
                for relation in &mut b.relations {
                    relation.human = true;
                }
                studio.session.metadata_changed();
            }
            if specs.len() == 2 {
                if link_pair(ui, studio.origin, b, overlay, &specs) {
                    studio.session.metadata_changed();
                }
            } else {
                ui.weak("To join two pieces yourself, select both in the scene.");
            }
            let name = |i: usize| pieces.get(i).map_or("a piece", |p| p.1.as_str());
            egui::ScrollArea::vertical()
                .max_height(200.0)
                .show(ui, |ui| {
                    if b.relations.is_empty() {
                        ui.weak("No pairs are remembered.");
                    }
                    for (i, r) in b.relations.iter().enumerate() {
                        let text = format!(
                            "{}  [{}]",
                            sentence(&r.kind, name(r.from), name(r.to)),
                            if r.human { "you said yes" } else { "guess" }
                        );
                        if ui.selectable_label(overlay.selected == Some(i), text).clicked() {
                            overlay.selected = Some(i);
                        }
                    }
                });
            if let Some(sel) = overlay.selected
                && sel < b.relations.len()
            {
                let (evidence, human) = (b.relations[sel].evidence.clone(), b.relations[sel].human);
                ui.separator();
                ui.label(format!("Why the editor thinks so: {}.", why(&evidence)));
                ui.horizontal(|ui| {
                    if ui
                        .button(if human { "Not sure after all" } else { "Yes, they go together" })
                        .clicked()
                    {
                        b.relations[sel].human = !human;
                        studio.session.metadata_changed();
                    }
                    if ui.button("Forget this note").clicked() {
                        b.relations.remove(sel);
                        overlay.selected = None;
                        studio.session.metadata_changed();
                    }
                });
            }
        });
}

/// Add or take away one floor tile, then recompute the corners that get levelled.
fn set_floor(
    b: &mut osrs_edit::PrefabBuilding,
    overlay: &Overlay,
    cell: osrs_edit::PrefabCell,
    owned: bool,
) {
    if owned {
        b.floors.push(cell);
    } else {
        b.floors.retain(|c| *c != cell);
    }
    b.floors.sort();
    b.floors.dedup();
    let mut support = std::collections::BTreeSet::new();
    for f in &b.floors {
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            support.insert(osrs_edit::PrefabCell {
                x: f.x + dx,
                y: f.y + dy,
                plane: f.plane,
            });
        }
    }
    for o in &overlay.prefab.objects {
        let (w, h) = b.object_sizes.get(&o.id).copied().unwrap_or((1, 1));
        let (w, h) = if o.orientation & 1 == 1 {
            (h, w)
        } else {
            (w, h)
        };
        for dx in 0..=w {
            for dy in 0..=h {
                support.insert(osrs_edit::PrefabCell {
                    x: o.x + dx,
                    y: o.y + dy,
                    plane: o.plane,
                });
            }
        }
    }
    b.support = support.into_iter().collect();
}

/// The button that joins the two selected pieces.
fn link_pair(
    ui: &mut egui::Ui,
    origin: MapTile,
    b: &mut osrs_edit::PrefabBuilding,
    overlay: &mut Overlay,
    specs: &[osrs_world::LocSpec],
) -> bool {
    let find = |s: &osrs_world::LocSpec| {
        let lx = s.tile.0 - origin.x;
        let ly = s.tile.1 - origin.y;
        (lx >= 0 && ly >= 0)
            .then(|| {
                overlay.prefab.objects.iter().position(|o| {
                    o.id == s.object_id
                        && o.x == lx as u16
                        && o.y == ly as u16
                        && o.plane == s.plane
                        && o.kind == s.loc_type
                        && o.orientation == s.orientation
                })
            })
            .flatten()
    };
    let (Some(from), Some(to)) = (find(&specs[0]), find(&specs[1])) else {
        ui.weak("Both selected pieces must belong to this building.");
        return false;
    };
    if from != to
        && ui.button("Join the two selected pieces").clicked()
        && !b
            .relations
            .iter()
            .any(|r| r.from == from && r.to == to && r.kind == "attached")
    {
        b.relations.push(osrs_edit::PrefabRelation {
            from,
            to,
            kind: "attached".into(),
            evidence: "human".into(),
            human: true,
        });
        overlay.selected = Some(b.relations.len() - 1);
        return true;
    }
    false
}
