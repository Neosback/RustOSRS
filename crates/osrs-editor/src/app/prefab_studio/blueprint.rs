//! Bind the author's sparse footprint to the generator, without copying any example placements.
use super::*;
use osrs_edit::{BuildingSketch, PrefabCell};

pub(super) fn controls(viewer: &mut EditorApp, original: &Prefab, ui: &mut egui::Ui) {
    if ui.button("Use drawn rooms and footprint").clicked() {
        match sketch(viewer, original) {
            Ok((sketch, w, h)) => {
                viewer.prefabs.build.site = Some(sketch.origin);
                viewer.prefabs.build.site_plane = viewer.interaction.view_plane();
                viewer.prefabs.build.form.width = w;
                viewer.prefabs.build.form.height = h;
                viewer.prefabs.build.form.floors = 1;
                viewer.prefabs.build.form.sketch = Some(sketch);
                if let Some(building) = &original.building {
                    let origin = MapTile::new(
                        super::CENTRE.0 - i32::from(original.width) / 2,
                        super::CENTRE.1 - i32::from(original.height) / 2,
                    );
                    viewer.prefabs.build.form.locks = building
                        .semantics
                        .locked_parts
                        .iter()
                        .filter_map(|i| {
                            let p = original.objects.get(*i)?;
                            Some(osrs_edit::ObjectPlacement {
                                object_id: osrs_core::ids::ObjectId::new(p.id),
                                kind: osrs_edit::LocKind::new(p.kind)?,
                                orientation: osrs_edit::ObjectOrientation::new(p.orientation)?,
                                position: osrs_edit::MapPosition::new(
                                    MapTile::new(
                                        origin.x + i32::from(p.x),
                                        origin.y + i32::from(p.y),
                                    ),
                                    SourcePlane::new(p.plane)?,
                                ),
                            })
                        })
                        .collect();
                }
            }
            Err(e) => viewer.prefabs.say(e),
        }
    }
    if ui.button("Use automatic room layout").clicked() {
        viewer.prefabs.build.form.sketch = None;
        viewer.prefabs.build.form.stage = None;
    }
    if viewer.prefabs.build.form.sketch.is_some() {
        ui.weak(
            "Drawn rooms are fixed. Reroll stages or change the kit without moving their walls.",
        );
    }
    if ui
        .button("Keep selected pieces during generation")
        .clicked()
    {
        viewer.prefabs.build.form.locks = viewer
            .selected_specs()
            .iter()
            .filter_map(|p| {
                Some(osrs_edit::ObjectPlacement {
                    object_id: osrs_core::ids::ObjectId::new(p.object_id),
                    kind: osrs_edit::LocKind::new(p.loc_type)?,
                    orientation: osrs_edit::ObjectOrientation::new(p.orientation)?,
                    position: osrs_edit::MapPosition::new(
                        MapTile::new(p.tile.0, p.tile.1),
                        SourcePlane::new(p.plane)?,
                    ),
                })
            })
            .collect();
    }
    if ui.button("Clear generation locks").clicked() {
        viewer.prefabs.build.form.locks.clear();
    }
    ui.label(format!(
        "{} locked occurrences",
        viewer.prefabs.build.form.locks.len()
    ));
    if ui.button("Use selected tiles as repair area").clicked() {
        let tiles = viewer.interaction.selection_tiles();
        if let (Some(x0), Some(y0), Some(x1), Some(y1)) = (
            tiles.iter().map(|c| c.0).min(),
            tiles.iter().map(|c| c.1).min(),
            tiles.iter().map(|c| c.0).max(),
            tiles.iter().map(|c| c.1).max(),
        ) {
            viewer.prefabs.build.form.region = osrs_edit::RectSite::new(
                MapTile::new(x0, y0),
                (x1 - x0 + 1).min(128) as u16,
                (y1 - y0 + 1).min(128) as u16,
            );
        }
    }
}
fn sketch(viewer: &EditorApp, original: &Prefab) -> Result<(BuildingSketch, u16, u16), String> {
    let original_origin = MapTile::new(
        super::CENTRE.0 - i32::from(original.width) / 2,
        super::CENTRE.1 - i32::from(original.height) / 2,
    );
    let building = original
        .building
        .as_ref()
        .ok_or("Review floor ownership in Rooms & parts first")?;
    let floor: Vec<_> = if viewer.interaction.selection_tiles().is_empty() {
        building
            .floors
            .iter()
            .filter(|c| c.plane == 0)
            .map(|c| {
                (
                    original_origin.x + i32::from(c.x),
                    original_origin.y + i32::from(c.y),
                )
            })
            .collect()
    } else {
        viewer
            .interaction
            .selection_tiles()
            .iter()
            .copied()
            .collect()
    };
    let min_x = floor
        .iter()
        .map(|c| c.0)
        .min()
        .ok_or("Select or draw floor tiles first")?;
    let min_y = floor.iter().map(|c| c.1).min().ok_or("No floor")?;
    let max_x = floor.iter().map(|c| c.0).max().ok_or("No floor")?;
    let max_y = floor.iter().map(|c| c.1).max().ok_or("No floor")?;
    let (w, h) = (max_x - min_x + 3, max_y - min_y + 3);
    if w > i32::from(crate::app::prefab_runtime::MAX_SITE)
        || h > i32::from(crate::app::prefab_runtime::MAX_SITE)
    {
        return Err("Drawn footprint exceeds the generator's 48-tile side limit".into());
    }
    let origin = MapTile::new(min_x - 1, min_y - 1);
    let floors: Vec<_> = floor
        .iter()
        .map(|&(x, y)| PrefabCell {
            x: (x - origin.x) as u16,
            y: (y - origin.y) as u16,
            plane: 0,
        })
        .collect();
    let mut semantics = building.semantics.clone();
    let remap = |c: PrefabCell| -> Option<PrefabCell> {
        let (x, y) = (
            original_origin.x + i32::from(c.x) - origin.x,
            original_origin.y + i32::from(c.y) - origin.y,
        );
        (x >= 0 && y >= 0 && x < w && y < h).then_some(PrefabCell {
            x: x as u16,
            y: y as u16,
            plane: c.plane,
        })
    };
    for room in &mut semantics.rooms {
        room.cells = room
            .cells
            .iter()
            .filter_map(|c| remap(*c))
            .filter(|c| floors.contains(c))
            .collect();
    }
    semantics.rooms.retain(|r| !r.cells.is_empty());
    semantics.portals = semantics
        .portals
        .into_iter()
        .filter_map(|mut p| {
            p.approaches = [remap(p.approaches[0])?, remap(p.approaches[1])?];
            Some(p)
        })
        .collect();
    semantics.anchor = None;
    Ok((
        BuildingSketch {
            floors,
            semantics,
            origin,
        },
        w as u16,
        h as u16,
    ))
}
