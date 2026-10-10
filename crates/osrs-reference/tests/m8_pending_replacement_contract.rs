use osrs_core::{
    coords::{SceneTile, StoragePlane},
    definitions::LocType,
    ids::ObjectId,
};
use osrs_scene::{
    PendingInsertion, PendingRemoval, PendingReplacementPlanError, PendingSceneCategory,
    PendingSceneMutation, PlacementInput, SceneGrid, SceneLayer, apply_pending_replacement,
    plan_pending_replacement, plan_placement,
};
use std::{convert::Infallible, error::Error};

#[derive(Debug, Default)]
struct TraceMutation {
    actions: Vec<&'static str>,
    remove_result: bool,
    insert_result: bool,
}

impl PendingSceneMutation for TraceMutation {
    type Error = Infallible;

    fn remove_pending(&mut self, _removal: PendingRemoval) -> Result<bool, Self::Error> {
        self.actions.push("remove");
        Ok(self.remove_result)
    }

    fn insert_pending(&mut self, _insertion: PendingInsertion) -> Result<bool, Self::Error> {
        self.actions.push("insert");
        Ok(self.insert_result)
    }
}

fn plane_zero() -> Result<StoragePlane, Box<dyn Error>> {
    StoragePlane::new(0).ok_or_else(|| "storage plane zero must be valid".into())
}

fn placement(tile: SceneTile, loc_type: u8) -> Result<osrs_scene::PlacementPlan, Box<dyn Error>> {
    Ok(plan_placement(PlacementInput {
        loc_type: LocType::new(loc_type),
        orientation: 0,
        tile,
        size_x: 2,
        size_y: 1,
        sampled_height: 0,
        existing_wall_displacement: None,
    })?)
}

#[test]
fn reference_pending_categories_map_to_exact_scene_layers() -> Result<(), Box<dyn Error>> {
    let expected = [
        (0, PendingSceneCategory::Boundary, SceneLayer::Boundary),
        (
            1,
            PendingSceneCategory::WallDecoration,
            SceneLayer::WallDecoration,
        ),
        (2, PendingSceneCategory::GameObject, SceneLayer::GameObject),
        (
            3,
            PendingSceneCategory::FloorDecoration,
            SceneLayer::FloorDecoration,
        ),
    ];

    for (raw, category, layer) in expected {
        let decoded = PendingSceneCategory::from_reference_type(raw)?;
        assert_eq!(decoded, category);
        assert_eq!(decoded.layer(), layer);
    }
    assert_eq!(
        PendingSceneCategory::from_reference_type(4),
        Err(PendingReplacementPlanError::InvalidSceneCategory(4))
    );
    Ok(())
}

#[test]
fn pending_replacement_removes_before_inserting() -> Result<(), Box<dyn Error>> {
    let plane = plane_zero()?;
    let tile = SceneTile::new(20, 30);
    let plan = plan_pending_replacement(
        plane,
        tile,
        2,
        Some((ObjectId::new(200), placement(tile, 10)?)),
    )?;
    let mut mutation = TraceMutation {
        actions: Vec::new(),
        remove_result: true,
        insert_result: true,
    };

    let report = apply_pending_replacement(&mut mutation, plan)?;

    assert_eq!(mutation.actions, vec!["remove", "insert"]);
    assert!(report.removed);
    assert!(report.inserted);
    Ok(())
}

#[test]
fn rejected_replacement_does_not_roll_back_prior_removal() -> Result<(), Box<dyn Error>> {
    let plane = plane_zero()?;
    let tile = SceneTile::new(40, 50);
    let plan = plan_pending_replacement(
        plane,
        tile,
        2,
        Some((ObjectId::new(201), placement(tile, 10)?)),
    )?;
    let mut mutation = TraceMutation {
        actions: Vec::new(),
        remove_result: true,
        insert_result: false,
    };

    let report = apply_pending_replacement(&mut mutation, plan)?;

    assert_eq!(mutation.actions, vec!["remove", "insert"]);
    assert!(report.removed);
    assert!(!report.inserted);
    Ok(())
}

#[test]
fn deletion_only_update_stops_after_removal() -> Result<(), Box<dyn Error>> {
    let plane = plane_zero()?;
    let tile = SceneTile::new(60, 70);
    let plan = plan_pending_replacement(plane, tile, 0, None)?;
    let mut mutation = TraceMutation {
        actions: Vec::new(),
        remove_result: true,
        insert_result: true,
    };

    let report = apply_pending_replacement(&mut mutation, plan)?;

    assert_eq!(mutation.actions, vec!["remove"]);
    assert!(report.removed);
    assert!(!report.inserted);
    Ok(())
}

#[test]
fn removal_category_is_independent_from_replacement_loc_layer() -> Result<(), Box<dyn Error>> {
    let plane = plane_zero()?;
    let tile = SceneTile::new(10, 11);
    let floor_placement = placement(tile, 22)?;
    let plan =
        plan_pending_replacement(plane, tile, 2, Some((ObjectId::new(300), floor_placement)))?;

    assert_eq!(plan.removal.category, PendingSceneCategory::GameObject);
    assert_eq!(plan.removal.tile, tile);
    let replacement = plan
        .replacement
        .ok_or_else(|| "replacement should be present".to_string())?;
    assert_eq!(replacement.tile, tile);
    assert_eq!(
        replacement.placement.kind.layer(),
        SceneLayer::FloorDecoration
    );
    Ok(())
}

#[test]
fn initial_game_object_is_replaced_through_live_scene_path() -> Result<(), Box<dyn Error>> {
    let plane = plane_zero()?;
    let tile = SceneTile::new(1, 1);
    let overlap = SceneTile::new(2, 1);
    let mut scene = SceneGrid::new(4, 4, 1)?;

    assert!(scene.insert_placement(plane, ObjectId::new(400), placement(tile, 10)?)?);
    assert_eq!(
        scene
            .game_object(plane, tile)
            .map(|object| object.object_id()),
        Some(ObjectId::new(400))
    );
    assert_eq!(
        scene
            .tile(plane, overlap)
            .map(|semantic_tile| semantic_tile.game_objects().len()),
        Some(1)
    );

    let plan = plan_pending_replacement(
        plane,
        tile,
        2,
        Some((ObjectId::new(401), placement(tile, 22)?)),
    )?;
    let report = apply_pending_replacement(&mut scene, plan)?;

    assert!(report.removed);
    assert!(report.inserted);
    assert!(scene.game_object(plane, tile).is_none());
    assert!(
        scene
            .tile(plane, overlap)
            .is_some_and(|semantic_tile| semantic_tile.game_objects().is_empty())
    );
    assert_eq!(
        scene
            .floor_decoration(plane, tile)
            .map(|placed| placed.object_id()),
        Some(ObjectId::new(401))
    );
    Ok(())
}

#[test]
fn pending_game_object_removal_matches_anchor_not_overlap() -> Result<(), Box<dyn Error>> {
    let plane = plane_zero()?;
    let anchor = SceneTile::new(1, 1);
    let overlap = SceneTile::new(2, 1);
    let mut scene = SceneGrid::new(4, 4, 1)?;

    assert!(scene.insert_placement(plane, ObjectId::new(500), placement(anchor, 10)?)?);
    let plan = plan_pending_replacement(plane, overlap, 2, None)?;
    let report = apply_pending_replacement(&mut scene, plan)?;

    assert!(!report.removed);
    assert!(!report.inserted);
    assert_eq!(
        scene
            .game_object(plane, anchor)
            .map(|object| object.object_id()),
        Some(ObjectId::new(500))
    );
    assert_eq!(
        scene
            .tile(plane, overlap)
            .map(|semantic_tile| semantic_tile.game_objects().len()),
        Some(1)
    );
    Ok(())
}
