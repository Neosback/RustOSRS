//! A path lesson as a lab task: the place with its paths taken off (a blank canvas that keeps the
//! ground, heights and objects) and the real thing as the hidden target. A learner lays paths with
//! the same path tool the editor has; the lab scores what it painted against the real paths.

use super::lesson::PathLesson;
use super::model::PathModel;
use super::route::{Tile, waypoints};
use crate::Action;
use crate::task::{LabTask, PaletteEntry, Snapshot};
use osrs_edit::{PrefabHeight, PrefabObject, PrefabOverlay, PrefabTile};
use std::collections::{BTreeMap, BTreeSet};

/// The blank canvas and the real place as two snapshots.
fn snapshots(lesson: &PathLesson, mask: &[bool]) -> (Snapshot, Snapshot) {
    let (w, h) = (usize::from(lesson.width), usize::from(lesson.height));
    let have_underlay = lesson.underlay.len() == w * h;
    let have_rotation = lesson.rotation.len() == w * h;
    let mut blank = Snapshot::default();
    let mut real = Snapshot::default();
    for y in 0..h {
        for x in 0..w {
            let at = y * w + x;
            let rotation = if have_rotation {
                lesson.rotation[at]
            } else {
                0
            };
            let underlay = if have_underlay {
                lesson.underlay[at]
            } else {
                0
            };
            let tile = |overlay: Option<PrefabOverlay>, underlay: u16| PrefabTile {
                x: x as u16,
                y: y as u16,
                plane: 0,
                underlay: (underlay != 0).then_some(u32::from(underlay)),
                overlay,
                flags: lesson.flags[at],
            };
            let overlay = (lesson.overlay[at] != 0).then(|| PrefabOverlay {
                id: u32::from(lesson.overlay[at]),
                shape: lesson.shape[at].min(11),
                rotation: rotation & 3,
            });
            real.tiles.push(tile(overlay.clone(), underlay));
            if mask[at] {
                // The floor that was there before the path: what is around it.
                let mut around: BTreeMap<u16, u32> = BTreeMap::new();
                if have_underlay {
                    for (nx, ny) in super::grid::neighbours8(x, y, w, h) {
                        let n = ny * w + nx;
                        if !mask[n] && lesson.underlay[n] != 0 {
                            *around.entry(lesson.underlay[n]).or_default() += 1;
                        }
                    }
                }
                let ground = around
                    .into_iter()
                    .max_by_key(|(_, c)| *c)
                    .map_or(underlay, |(u, _)| u);
                blank.tiles.push(tile(None, ground));
            } else {
                blank.tiles.push(tile(overlay, underlay));
            }
        }
    }
    for y in 0..=h {
        for x in 0..=w {
            let value = lesson.heights[y.min(h - 1) * w + x.min(w - 1)];
            let corner = PrefabHeight {
                x: x as u16,
                y: y as u16,
                plane: 0,
                value,
            };
            blank.heights.push(corner.clone());
            real.heights.push(corner);
        }
    }
    for o in &lesson.objects {
        let object = PrefabObject {
            id: o[0],
            kind: o[1] as u8,
            orientation: o[2] as u8,
            x: o[3] as u16,
            y: o[4] as u16,
            plane: 0,
        };
        blank.objects.push(object.clone());
        real.objects.push(object);
    }
    (blank, real)
}

/// The lesson as a task: blank canvas in, real place as the hidden target.
pub fn path_task(lesson: &PathLesson) -> Result<LabTask, String> {
    lesson.validate()?;
    let scene = super::sense::Scene::of(lesson);
    let (initial, target) = snapshots(lesson, &scene.mask);
    let mut palette: BTreeMap<(u32, u8), PaletteEntry> = BTreeMap::new();
    for o in &target.objects {
        palette
            .entry((o.id, o.kind))
            .or_insert_with(|| PaletteEntry {
                id: o.id,
                kind: o.kind,
                size: lesson
                    .names
                    .get(&o.id)
                    .map_or([1, 1], |e| [e.size[0].max(1), e.size[1].max(1)]),
            });
    }
    let overlays: BTreeSet<u32> = target
        .tiles
        .iter()
        .filter_map(|t| t.overlay.as_ref().map(|o| o.id))
        .collect();
    let underlays: BTreeSet<u32> = target.tiles.iter().filter_map(|t| t.underlay).collect();
    Ok(LabTask {
        locks: vec![],
        object_names: Default::default(),
        format: "rustosrs-lab-task".into(),
        version: 1,
        name: format!("Lay the paths of {}", lesson.name),
        source: lesson.name.clone(),
        group: format!("paths:{}", lesson.group),
        width: lesson.width,
        height: lesson.height,
        planes: 1,
        budget: 512,
        initial,
        target,
        palette: palette.into_values().collect(),
        overlays: overlays.into_iter().collect(),
        underlays: underlays.into_iter().collect(),
        dont_care_tiles: Vec::new(),
        dont_care_heights: Vec::new(),
    })
}

/// The path-tool actions that lay `routes` with the model's usual width and overlay.
pub fn lay_actions(model: &PathModel, routes: &[Vec<Tile>]) -> Vec<Action> {
    routes
        .iter()
        .filter(|r| r.len() >= 2)
        .map(|route| Action::PathLay {
            plane: 0,
            points: waypoints(route, 1.2),
            width: f64::from(model.width),
            edge_weight: 1.5,
            smooth_curve: true,
            overlay_id: u32::from(model.overlay),
            companion_underlay: None,
            smooth_heights: None,
        })
        .collect()
}
