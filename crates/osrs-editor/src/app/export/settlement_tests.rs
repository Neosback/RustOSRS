//! Creation acceptance test using production tool planners, history, project codecs, and readers.
use crate::app::paint_commit::{TerrainPaintIntent, build_terrain_paint_patch};
use crate::app::path_commit::{TerrainPathIntent, build_terrain_path_patch};
use crate::export_flow::test_support::*;
use crate::export_flow::{ExportSnapshot, ExportStamp, prepare_with, publish_fixture};
use crate::heights::commit::{HeightStrokeSpec, build_terrain_height_patch};
use crate::session::{EditorSession, ProjectFile, diff_region};
use osrs_cache::verified_export::VerifiedExport;
use osrs_core::{
    coords::MapTile,
    ids::{FloorUnderlayId, ObjectId},
};
use osrs_edit::{LocKind, MapPatch, ObjectOrientation, ObjectPlacement, build_object_patch};

fn commit(session: &mut EditorSession, patch: MapPatch) {
    assert!(
        session
            .commit(patch)
            .unwrap_or_else(|e| unreachable!("{e:?}"))
            .is_some()
    );
}

#[test]
fn settlement_creation_save_reopen_export_and_inspection_preserve_the_captured_map() {
    let f = fixture();
    let mut authoring = EditorSession::new(baseline(&f.regions));
    let painted: Vec<_> = [(3263, 3204), (3264, 3204)]
        .into_iter()
        .map(|tile| {
            let mut intent =
                TerrainPaintIntent::new(osrs_core::coords::MapTile::new(tile.0, tile.1));
            intent.underlay = Some(Some(FloorUnderlayId::new(5)));
            intent
        })
        .collect();
    let patch = build_terrain_paint_patch(authoring.document(), plane(0), &painted)
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    commit(&mut authoring, patch);
    let height = HeightStrokeSpec::new(
        plane(0),
        crate::heights::HeightTool::default(),
        1,
        crate::paint::BrushShape::Square,
        -80,
        42,
    );
    let patch = build_terrain_height_patch(
        authoring.document(),
        &height,
        &[MapTile::new(3263, 3204), MapTile::new(3264, 3204)],
    )
    .unwrap_or_else(|e| unreachable!("{e:?}"));
    commit(&mut authoring, patch);
    let fitted = crate::terrain_path::fit_path_overlay(
        &crate::terrain_path::smooth_path(&[
            crate::terrain_path::ControlPoint::new(3261.5, 3208.5, 1.5),
            crate::terrain_path::ControlPoint::new(3266.5, 3208.5, 1.5),
        ]),
        &crate::terrain_path::FitOptions {
            edge_weight: 0.8,
            prior_weight: 0.0,
            blocked: &|_| false,
        },
    )
    .tiles;
    assert!(fitted.iter().any(|t| t.tile_x == 3263) && fitted.iter().any(|t| t.tile_x == 3264));
    let intents: Vec<_> = fitted
        .iter()
        .map(|t| TerrainPathIntent {
            tile: osrs_core::coords::MapTile::new(t.tile_x, t.tile_y),
            overlay: Some(2),
            shape: t.shape,
            rotation: t.orientation,
            height: None,
            flags: None,
        })
        .collect();
    let patch = build_terrain_path_patch(authoring.document(), plane(0), &intents)
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    commit(&mut authoring, patch);
    let first = ObjectPlacement {
        object_id: ObjectId::new(1000),
        position: position(3263, 3206, 1),
        kind: LocKind::new(10).unwrap_or_else(|| unreachable!()),
        orientation: ObjectOrientation::new(0).unwrap_or_else(|| unreachable!()),
    };
    let patch = build_object_patch(authoring.document(), &[], &[first])
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    commit(&mut authoring, patch);
    let moved = ObjectPlacement {
        position: position(3264, 3206, 1),
        orientation: ObjectOrientation::new(2).unwrap_or_else(|| unreachable!()),
        ..first
    };
    let patch = build_object_patch(authoring.document(), &[first], &[moved])
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    commit(&mut authoring, patch);
    let edited = authoring.document().clone();
    authoring.undo().unwrap_or_else(|e| unreachable!("{e:?}"));
    assert!(authoring.document().contains_object(first));
    authoring.redo().unwrap_or_else(|e| unreachable!("{e:?}"));
    assert_eq!(authoring.document(), &edited);

    let diffs: Vec<_> = f
        .regions
        .iter()
        .map(|r| {
            let square = square(*r);
            diff_region(&square.terrain, &square.locations, authoring.document())
                .unwrap_or_else(|e| unreachable!("{e:?}"))
        })
        .filter(|d| !d.is_empty())
        .collect();
    let root = temp("settlement");
    let project = root.join("settlement.json");
    std::fs::write(
        &project,
        ProjectFile::from_diffs(&f.profile.id, &diffs).to_json(),
    )
    .unwrap_or_else(|e| unreachable!("{e}"));
    let loaded = ProjectFile::from_json(
        &std::fs::read_to_string(project).unwrap_or_else(|e| unreachable!("{e}")),
    )
    .unwrap_or_else(|e| unreachable!("{e:?}"));
    let pending = loaded
        .into_diffs()
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let mut reopened = EditorSession::new(baseline(&f.regions));
    for diff in &pending {
        reopened
            .apply_untracked(&diff.to_patch(reopened.document()))
            .unwrap_or_else(|e| unreachable!("{e:?}"));
    }
    assert_eq!(reopened.document(), authoring.document());
    let parked = authoring.clone();
    // Export immediately after project open, before any saved regions have streamed in.
    let snapshot = ExportSnapshot {
        stamp: ExportStamp {
            epoch: 1,
            job: 9,
            edit_revision: 7,
        },
        source: osrs_cache::source::CacheSource::Dir(f.dir.clone()),
        profile: f.profile.id.clone(),
        document: osrs_edit::MapDocument::new(),
        authored: Default::default(),
        pending,
    };
    let prepared = prepare_with(
        snapshot,
        f.profile
            .cache_fingerprint
            .value
            .clone()
            .unwrap_or_default(),
        |r| Ok(Some(square(r))),
    );
    assert!(prepared.can_publish(), "{:?}", prepared.issues);
    let out = root.join("export");
    let report = publish_fixture(&prepared, &out, std::slice::from_ref(&f.profile))
        .unwrap_or_else(|e| unreachable!("{e}"));
    let inspection = VerifiedExport::validate(
        &f.dir,
        &out,
        &report.manifest,
        std::slice::from_ref(&f.profile),
    )
    .unwrap_or_else(|e| unreachable!("{e}"));
    let reader = osrs_world::SourceSquares::open(inspection.source())
        .unwrap_or_else(|e| unreachable!("{e}"));
    for region in &prepared.regions {
        let actual = reader
            .read(region.region)
            .unwrap_or_else(|e| unreachable!("{e}"))
            .unwrap_or_else(|| unreachable!());
        assert!(
            diff_region(&actual.terrain, &actual.locations, reopened.document())
                .unwrap_or_else(|e| unreachable!("{e:?}"))
                .is_empty()
        );
        assert_eq!(actual.files[2..], square(region.region).files[2..]);
    }
    drop(reader);
    assert_eq!(
        authoring, parked,
        "inspection and return must not touch authoring document, history, revisions, or dirty ownership"
    );
    let _ = std::fs::remove_dir_all(root);
}
