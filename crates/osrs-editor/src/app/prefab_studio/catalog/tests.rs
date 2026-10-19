//! Apply WFC to authoring authority, with real transaction and sparse save round trips.
use super::*;
use crate::{app::edit_runtime::EditRuntime, session::EditorSession};
use osrs_edit::{CaptureSpec, LearnedPattern, MapDocument, PrefabSource, capture_prefab_over};
fn data() -> LearnedCatalog {
    LearnedCatalog {
        format: "rustosrs-learned-catalog".into(),
        version: 1,
        style: "fixture".into(),
        role: "floor".into(),
        source: "reviewed fixture".into(),
        patterns: vec![LearnedPattern {
            shape: 0,
            rotation: 0,
            floor_class: (3 << 16) | 8,
            sockets: [0; 4],
        }],
        weights: vec![1],
        adjacency: (0..4).map(|d| (0, d, 0)).collect(),
    }
}
fn solution(data: &LearnedCatalog) -> WfcSolution {
    osrs_edit::solve_wfc_with_repair(
        &data.compile().unwrap_or_else(|e| panic!("{e:?}")),
        8,
        8,
        42,
        &[],
        64,
        4,
    )
    .unwrap_or_else(|e| panic!("{e:?}"))
}
fn origin() -> MapTile {
    MapTile::new(super::super::CENTRE.0 - 4, super::super::CENTRE.1 - 4)
}
fn p() -> SourcePlane {
    SourcePlane::new(0).unwrap_or_else(|| unreachable!())
}
#[test]
#[allow(clippy::unwrap_used)] // Test fixture: id 1 is a valid pattern id.
fn changed_pins_and_pending_solves_cannot_apply_an_old_result() {
    let catalog = data();
    let mut view = CatalogView {
        width: 8,
        height: 8,
        mask: vec![true; 64],
        solution: Some(solution(&catalog)),
        ..Default::default()
    };
    assert!(view.can_apply());
    view.pins.push(WfcConstraint {
        coord: WfcCoord::new(1, 1),
        allowed: PatternDomain::singleton(WfcPatternId::new(1).unwrap()),
    });
    assert!(!view.can_apply());
    view.pins.clear();
    #[cfg(not(target_arch = "wasm32"))]
    {
        let (_send, receive) = std::sync::mpsc::channel();
        view.pending = Some(receive);
        assert!(!view.can_apply());
        view.use_cells(&[(4, 4), (5, 4)]);
        assert!(view.pending.is_none());
        assert!(!view.can_apply());
    }
}
#[test]
fn wfc_preview_cancel_leaves_author_document_and_history_unchanged() {
    let pad = MapDocument::default();
    let runtime = EditRuntime::new(EditorSession::new(pad.clone()));
    let catalog = data();
    let patch = osrs_edit::plan_wfc_floors(
        runtime.document(),
        &catalog,
        &solution(&catalog),
        origin(),
        p(),
    )
    .unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(patch.len(), 64);
    drop(patch); // Cancel before entering the edit gateway.
    assert_eq!(runtime.document(), &pad);
    assert!(!runtime.can_undo());
}
#[test]
fn wfc_author_commit_survives_save_and_exact_undo_redo() {
    let pad = MapDocument::default();
    let mut runtime = EditRuntime::new(EditorSession::new(pad.clone()));
    let catalog = data();
    let patch = osrs_edit::plan_wfc_floors(
        runtime.document(),
        &catalog,
        &solution(&catalog),
        origin(),
        p(),
    )
    .unwrap_or_else(|e| panic!("{e:?}"));
    runtime
        .execute(patch, |_| Ok::<_, ()>(()))
        .unwrap_or_else(|e| panic!("{e:?}"));
    let authored = runtime.document().clone();
    assert!(runtime.can_undo());
    let origin = MapTile::new(super::super::CENTRE.0 - 4, super::super::CENTRE.1 - 4);
    let spec = CaptureSpec {
        origin,
        width: 8,
        height: 8,
        base_plane: p(),
        planes: 1,
        terrain: true,
        heights: false,
        objects: true,
    };
    let original = osrs_edit::Prefab {
        format: osrs_edit::PREFAB_FORMAT.into(),
        version: osrs_edit::PREFAB_VERSION,
        name: "wfc".into(),
        tags: vec![],
        notes: String::new(),
        source: PrefabSource::Authored,
        width: 8,
        height: 8,
        planes: 1,
        tiles: vec![],
        heights: vec![],
        objects: vec![],
        building: None,
    };
    let saved = capture_prefab_over(runtime.document(), &pad, spec, &original)
        .unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(saved.source, PrefabSource::Authored);
    assert_eq!(saved.tiles.len(), 64);
    assert!(
        saved
            .tiles
            .iter()
            .all(|t| t.underlay == Some(3) && t.overlay.as_ref().is_some_and(|o| o.id == 8))
    );
    runtime.undo().unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(runtime.document(), &pad);
    runtime
        .replay(
            crate::app::edit_runtime::EditReplayDirection::Redo,
            crate::app::edit_runtime::EditFootprint::default(),
            |_| Ok::<_, ()>(()),
        )
        .unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(runtime.document(), &authored);
}
