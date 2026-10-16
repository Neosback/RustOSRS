//! Read-only clipping explanation. No widget mutates scene, collision or map authority.

use crate::app::world_runtime::CollisionInspection;
use eframe::egui;
use osrs_core::coords::{MapTile, PlaneIndex};
use osrs_scene::collision::{FLOOR, FLOOR_DECORATION, OBJECT, PROJECTILE_OBJECT, WALL_DIRECTIONS};

pub(super) fn show(ui: &mut egui::Ui, view: CollisionInspection<'_>, tile: MapTile, plane: u8) {
    egui::CollapsingHeader::new("Collision and blockers")
        .id_salt("tile_collision")
        .show(ui, |ui| body(ui, view, tile, plane));
}

fn body(ui: &mut egui::Ui, view: CollisionInspection<'_>, tile: MapTile, plane: u8) {
    let snapshot = match view {
        CollisionInspection::Ready(snapshot) => snapshot,
        CollisionInspection::Updating => {
            ui.label("Updating collision after map changes…");
            return;
        }
        CollisionInspection::Unavailable => {
            ui.label("Collision is unavailable for this region.");
            return;
        }
    };
    let Some(plane) = PlaneIndex::new(plane) else {
        return;
    };
    let Some(cell) = snapshot.cell(plane, tile) else {
        ui.label("This tile is outside the collision snapshot.");
        return;
    };
    ui.label(format!("Collision plane {}", plane.get()));
    ui.add(
        egui::Label::new("Collision planes can differ from source and draw planes on bridges.")
            .wrap(),
    );
    if !cell.loaded {
        ui.label("Land is missing or outside the usable scene. Traversability is unknown.");
        return;
    }
    if snapshot.unresolved_definitions() != 0 {
        ui.add(
            egui::Label::new(format!(
                "Incomplete: {} object definitions are unavailable in this scene.",
                snapshot.unresolved_definitions()
            ))
            .wrap(),
        );
    }
    let mut any = false;
    for (mask, label) in [
        (OBJECT, "Object footprint blocks walking"),
        (PROJECTILE_OBJECT, "Object footprint blocks projectiles"),
        (FLOOR, "Terrain blocks walking"),
        (FLOOR_DECORATION, "Floor decoration blocks walking"),
    ] {
        if cell.flags & mask != 0 {
            ui.add(egui::Label::new(label).wrap());
            any = true;
        }
    }
    for (mask, direction) in WALL_DIRECTIONS {
        let walk = cell.flags & mask != 0;
        let projectile = cell.flags & (mask << 9) != 0;
        if walk || projectile {
            let blocks = match (walk, projectile) {
                (true, true) => "walking and projectiles",
                (true, false) => "walking",
                _ => "projectiles",
            };
            ui.add(egui::Label::new(format!("{direction} wall blocks {blocks}")).wrap());
            any = true;
        }
    }
    if !any {
        ui.label("No clipping flags on this cell.");
    }
    ui.add(egui::Label::new("These are tile clipping contributions; a route also depends on neighbouring tiles and actor size.").wrap());
    egui::CollapsingHeader::new("Contributors and raw flags")
        .show(ui, |ui| contributors(ui, snapshot, plane, tile, cell));
}

fn contributors(
    ui: &mut egui::Ui,
    snapshot: &osrs_scene::collision::CollisionSnapshot,
    plane: PlaneIndex,
    tile: MapTile,
    cell: osrs_scene::collision::CollisionCell,
) {
    ui.monospace(format!("Flags: {:#010x}", cell.flags));
    for source in 0..4 {
        if cell.terrain_source_planes & (1 << source) != 0 {
            ui.label(format!("Terrain source plane {source}"));
        }
    }
    for (object, flags) in snapshot.objects_at(plane, tile) {
        ui.separator();
        ui.add(
            egui::Label::new(format!(
                "Object {} · type {} · facing {}",
                object.object_id.get(),
                object.loc_type.get(),
                object.orientation
            ))
            .wrap(),
        );
        ui.add(
            egui::Label::new(format!(
                "Anchor {}, {} · source plane {}",
                object.tile.x,
                object.tile.y,
                object.source_plane.index().get()
            ))
            .wrap(),
        );
        ui.monospace(format!("Contributes {flags:#010x}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use osrs_core::{
        coords::{RegionCoord, SourcePlane},
        definitions::LocType,
        ids::ObjectId,
    };
    use osrs_scene::{
        CollisionSideEffect,
        collision::{CollisionObject, CollisionSnapshotBuilder},
    };

    #[test]
    fn clipping_explanations_and_provenance_fit_narrow_panels()
    -> Result<(), Box<dyn std::error::Error>> {
        let tile = MapTile::new(-1, -1);
        let mut builder = CollisionSnapshotBuilder::reference(MapTile::new(-32, -32));
        builder.load_region(RegionCoord::new(-1, -1));
        let source = SourcePlane::new(0).ok_or("plane")?;
        builder.terrain(source, tile, 1, false);
        builder.object(
            CollisionObject {
                object_id: ObjectId::new(u32::MAX),
                tile,
                source_plane: source,
                loc_type: LocType::new(2),
                orientation: 0,
                effect: CollisionSideEffect::Boundary {
                    loc_type: LocType::new(2),
                    orientation: 0,
                    blocks_projectiles: true,
                },
            },
            false,
        )?;
        builder.missing_definition();
        let snapshot = builder.build();
        let cell = snapshot.cell(source.index(), tile).ok_or("cell")?;
        for width in [240.0, 320.0, 480.0] {
            for scale in [1.0, 2.0] {
                let ctx = crate::theme::layout_test_context(scale);
                let input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 2000.0),
                    )),
                    ..Default::default()
                };
                let mut output = ctx.run_ui(input, |ui| {
                    let right = ui.max_rect().right();
                    let response = ui.scope(|ui| {
                        body(ui, CollisionInspection::Ready(&snapshot), tile, 0);
                        contributors(ui, &snapshot, source.index(), tile, cell);
                        body(ui, CollisionInspection::Updating, tile, 0);
                        body(ui, CollisionInspection::Unavailable, tile, 0);
                    });
                    assert!(
                        response.response.rect.right() <= right + 0.5,
                        "width {width}, scale {scale}: {:?}",
                        response.response.rect
                    );
                });
                output.textures_delta.clear();
            }
        }
        Ok(())
    }
}
