//! Initial clipping follows source placements even when a requested model type draws nothing.

use osrs_core::{
    coords::{MapTile, SourcePlane},
    definitions::ObjectModels,
};
use osrs_scene::CollisionSideEffect;
use osrs_world::{
    LocEdit, LocSpec, SceneWindow, TerrainPresentation, WorldDefinitions, build_world_scene,
};

#[test]
#[ignore = "requires the pinned build-241 cache (RUSTOSRS_TARGET_CACHE_DIR)"]
fn invisible_placement_contributes_clipping_and_undo_rebuild_restores_cells()
-> Result<(), Box<dyn std::error::Error>> {
    let mut definitions = WorldDefinitions::open(std::env::var("RUSTOSRS_TARGET_CACHE_DIR")?)?;
    let window = SceneWindow::new(3176, 3176).ok_or("window")?;
    let before = build_world_scene(&mut definitions, window, TerrainPresentation::default())?;
    let mut candidate = None;
    for loc in &before.locs {
        let Some(definition) = definitions.object(loc.object_id)? else {
            continue;
        };
        if definition.placement.interact_type != 0
            && definition.animation.is_none()
            && definition.morphs.is_none()
            && matches!(definition.models, Some(ObjectModels::Untyped(_)))
        {
            candidate = Some(loc.object_id.get());
            break;
        }
    }
    let spec = LocSpec {
        object_id: candidate.ok_or("static untyped blocking definition")?,
        tile: (3210, 3210),
        plane: 0,
        loc_type: 12,
        orientation: 1,
    };
    definitions.apply_loc_edits(&[LocEdit::Add(spec)]);
    let changed = build_world_scene(&mut definitions, window, TerrainPresentation::default())?;
    let tile = MapTile::new(spec.tile.0, spec.tile.1);
    let plane = SourcePlane::new(0).ok_or("plane")?.index();
    let loc = changed
        .locs
        .iter()
        .find(|loc| {
            loc.object_id.get() == spec.object_id
                && loc.loc_type == 12
                && loc.tile.to_map(MapTile::new(window.base_x, window.base_y)) == Some(tile)
        })
        .ok_or("new invisible loc")?;
    assert!(
        loc.renderables.is_empty(),
        "test must exercise actual model omission"
    );
    assert!(
        changed
            .collision
            .objects_at(plane, tile)
            .any(|(object, _)| object.object_id.get() == spec.object_id
                && object.tile == tile
                && matches!(object.effect, CollisionSideEffect::GameObject { .. }))
    );
    assert_eq!(changed.collision.unresolved_definitions(), 0);
    definitions.apply_loc_edits(&[LocEdit::Remove(spec)]);
    let restored = build_world_scene(&mut definitions, window, TerrainPresentation::default())?;
    for plane in (0..4).filter_map(SourcePlane::new).map(SourcePlane::index) {
        for x in 0..104 {
            for y in 0..104 {
                let tile = MapTile::new(window.base_x + x, window.base_y + y);
                assert_eq!(
                    before.collision.cell(plane, tile),
                    restored.collision.cell(plane, tile),
                    "{plane:?} {tile:?}"
                );
            }
        }
    }
    Ok(())
}
