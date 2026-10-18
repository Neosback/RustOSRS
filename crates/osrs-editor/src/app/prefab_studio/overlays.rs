//! Relationship projections explain persisted ownership; they never decide editing membership.
use super::*;
use egui::{Color32, Pos2, Stroke};
pub(in crate::app) struct Overlay {
    pub(super) prefab: Prefab,
    pub(super) origin: MapTile,
    pub(super) selected: Option<usize>,
}
pub(super) fn panel(studio: &mut Studio, ctx: &egui::Context) {
    egui::Window::new("Building relationships")
        .default_width(340.0)
        .show(ctx, |ui| {
            let Some(b) = &mut studio.original.building else {
                ui.label("Rectangle prefab: capture a reviewed building to add ownership evidence.");
                return;
            };
            ui.label(format!(
                "{} owned floors · {} support vertices",
                b.floors.len(),
                b.support.len()
            ));
            ui.label("Blue: floors · orange: support · red: outside anchors · purple: roof · yellow: attachment/approach");
            ui.label("Outside anchors do not own the terrain beneath them.");
            ui.checkbox(
                &mut studio.auto_own_painted_floors,
                "Auto-own painted floors & expand bounds on save",
            );
            let viewer = studio.stage.viewer();
            let specs = viewer.selected_specs();
            let Some(overlay) = viewer.prefabs.studio_overlay.as_mut() else {
                return;
            };
            for (key, title) in [
                ("human:archetype", "Archetype"),
                ("human:quality", "Quality / review note"),
            ] {
                let mut value = b.labels.get(key).cloned().unwrap_or_default();
                ui.label(title);
                if ui.text_edit_singleline(&mut value).changed() {
                    b.labels.insert(key.into(), value);
                    studio.session.metadata_changed();
                }
            }
            if let Some((x, y)) = viewer.interaction.inspect_tile() {
                let local = (
                    x - studio.origin.x,
                    y - studio.origin.y,
                    viewer.interaction.view_plane(),
                );
                if local.0 >= 0
                    && local.1 >= 0
                    && local.0 < i32::from(studio.original.width)
                    && local.1 < i32::from(studio.original.height)
                    && local.2 < studio.original.planes
                {
                    let cell = osrs_edit::PrefabCell {
                        x: local.0 as u16,
                        y: local.1 as u16,
                        plane: local.2,
                    };
                    let mut owned = b.floors.contains(&cell);
                    if ui
                        .checkbox(
                            &mut owned,
                            format!("Own inspected floor {},{} p{}", cell.x, cell.y, cell.plane),
                        )
                        .changed()
                    {
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
                        studio.session.metadata_changed();
                    }
                }
            }
            if specs.len() == 2 {
                let find_idx = |s: &osrs_world::LocSpec| {
                    let lx = s.tile.0 - studio.origin.x;
                    let ly = s.tile.1 - studio.origin.y;
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
                if let (Some(from), Some(to)) = (find_idx(&specs[0]), find_idx(&specs[1]))
                    && from != to
                    && ui
                        .button(format!("Link selected pair ({from} → {to})"))
                        .clicked()
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
                    studio.session.metadata_changed();
                }
            }
            egui::ScrollArea::vertical()
                .max_height(220.0)
                .show(ui, |ui| {
                    for (i, r) in b.relations.iter().enumerate() {
                        if ui
                            .selectable_label(
                                overlay.selected == Some(i),
                                format!(
                                    "Piece {} → {}: {}{}",
                                    r.from,
                                    r.to,
                                    r.kind,
                                    if r.human { " (confirmed)" } else { "" }
                                ),
                            )
                            .clicked()
                        {
                            overlay.selected = Some(i);
                        }
                    }
                });
            if let Some(sel) = overlay.selected
                && sel < b.relations.len()
            {
                let (evidence, human) = {
                    let r = &b.relations[sel];
                    (r.evidence.clone(), r.human)
                };
                ui.label(format!("Evidence: {evidence}"));
                ui.label(if human {
                    "Confirmed by a person"
                } else {
                    "Inferred relationship; review before reuse"
                });
                ui.horizontal(|ui| {
                    if ui
                        .button(if human {
                            "Mark inferred"
                        } else {
                            "Confirm relation"
                        })
                        .clicked()
                    {
                        b.relations[sel].human = !human;
                        studio.session.metadata_changed();
                    }
                    if ui.button("Remove relation").clicked() {
                        b.relations.remove(sel);
                        overlay.selected = None;
                        studio.session.metadata_changed();
                    }
                });
            }
        });
}

pub(in crate::app) fn draw(
    app: &EditorApp,
    painter: &egui::Painter,
    rect: egui::Rect,
    scale: f32,
    width: u32,
    height: u32,
) {
    let Some(overlay) = &app.prefabs.studio_overlay else {
        return;
    };
    let Some(b) = &overlay.prefab.building else {
        return;
    };
    let plane = app.interaction.view_plane();
    let project = |x: f64, y: f64, p: u8| -> Option<Pos2> {
        let source = SourcePlane::new(p)?;
        let tile = MapTile::new(
            overlay.origin.x + x.floor() as i32,
            overlay.origin.y + y.floor() as i32,
        );
        let h = app
            .session
            .document()
            .height_at(osrs_edit::HeightVertexCoord::new(tile, source))?;
        app.screen_point(
            [
                (f64::from(overlay.origin.x) + x) * 128.0,
                f64::from(h) - 12.0,
                (f64::from(overlay.origin.y) + y) * 128.0,
            ],
            rect,
            scale,
            width,
            height,
        )
    };
    let line = |a: (f64, f64), c: (f64, f64), p: u8, color: Color32| {
        if let (Some(a), Some(c)) = (project(a.0, a.1, p), project(c.0, c.1, p)) {
            painter.line_segment([a, c], Stroke::new(2.0, color));
        }
    };
    for f in b.floors.iter().filter(|f| f.plane == plane) {
        let (x, y) = (f64::from(f.x), f64::from(f.y));
        for (a, c) in [
            ((x, y), (x + 1.0, y)),
            ((x + 1.0, y), (x + 1.0, y + 1.0)),
            ((x + 1.0, y + 1.0), (x, y + 1.0)),
            ((x, y + 1.0), (x, y)),
        ] {
            line(
                a,
                c,
                plane,
                Color32::from_rgba_unmultiplied(80, 180, 255, 140),
            );
        }
    }
    for s in b.support.iter().filter(|s| s.plane == plane) {
        if let Some(p) = project(f64::from(s.x), f64::from(s.y), plane) {
            painter.circle_stroke(p, 2.0, Stroke::new(1.0, Color32::ORANGE));
        }
    }
    for (i, o) in overlay
        .prefab
        .objects
        .iter()
        .enumerate()
        .filter(|(_, o)| o.plane == plane)
    {
        let (x, y) = (f64::from(o.x), f64::from(o.y));
        let outside = !b
            .floors
            .iter()
            .any(|f| (f.x, f.y, f.plane) == (o.x, o.y, o.plane));
        let color = if matches!(o.kind, 12..=21) {
            Color32::from_rgb(200, 120, 255)
        } else if outside {
            Color32::LIGHT_RED
        } else {
            Color32::LIGHT_GREEN
        };
        if let Some(p) = project(x + 0.5, y + 0.5, plane) {
            painter.circle_stroke(p, 4.0, Stroke::new(2.0, color));
        }
        let edge = |orientation: u8| match orientation {
            0 => ((x, y), (x, y + 1.0)),
            1 => ((x, y + 1.0), (x + 1.0, y + 1.0)),
            2 => ((x + 1.0, y), (x + 1.0, y + 1.0)),
            _ => ((x, y), (x + 1.0, y)),
        };
        if matches!(o.kind, 0 | 2) {
            let (a, c) = edge(o.orientation);
            line(a, c, plane, color);
            if o.kind == 2 {
                let (a, c) = edge((o.orientation + 1) & 3);
                line(a, c, plane, color);
            }
        }
        let door = b
            .labels
            .get(&format!("inferred:member:{i}:role"))
            .is_some_and(|s| s == "Door");
        if door {
            let (dx, dy) = match o.orientation {
                0 => (-1.0, 0.0),
                1 => (0.0, 1.0),
                2 => (1.0, 0.0),
                _ => (0.0, -1.0),
            };
            line(
                (x + 0.5 - dx * 0.5, y + 0.5 - dy * 0.5),
                (x + 0.5 + dx * 1.5, y + 0.5 + dy * 1.5),
                plane,
                Color32::YELLOW,
            );
        }
    }
    for (i, r) in b.relations.iter().enumerate() {
        if let (Some(a), Some(c)) = (
            overlay.prefab.objects.get(r.from),
            overlay.prefab.objects.get(r.to),
        ) {
            if a.plane != plane {
                continue;
            }
            if let (Some(a), Some(c)) = (
                project(f64::from(a.x) + 0.5, f64::from(a.y) + 0.5, a.plane),
                project(f64::from(c.x) + 0.5, f64::from(c.y) + 0.5, c.plane),
            ) {
                painter.line_segment(
                    [a, c],
                    Stroke::new(
                        if overlay.selected == Some(i) {
                            3.0
                        } else {
                            1.0
                        },
                        if overlay.selected == Some(i) {
                            Color32::WHITE
                        } else {
                            Color32::from_rgba_unmultiplied(255, 220, 80, 100)
                        },
                    ),
                );
            }
        }
    }
}
