use super::*;

#[test]
fn old_projects_load_with_stock_appearance_and_new_projects_preserve_custom_looks() {
    let old = r#"{"format":"rustosrs-project-v1","profile":"p","regions":[]}"#;
    let mut file = ProjectFile::from_json(old).unwrap_or_else(|e| unreachable!("{e:?}"));
    assert_eq!(file.format, PROJECT_FORMAT);
    assert_eq!(file.appearance, osrs_hd::theme::ThemePack::default());
    file.appearance.definition = file
        .appearance
        .definition
        .duplicate("my-theme".into(), "Dusk".into());
    let saved = ProjectFile::from_json(&file.to_json()).unwrap_or_else(|e| unreachable!("{e:?}"));
    assert_eq!(saved.appearance, file.appearance);
}

fn plane0() -> SourcePlane {
    SourcePlane::new(0).unwrap_or_else(|| unreachable!("plane zero is valid"))
}

fn sample_diff() -> RegionDiff {
    let tile = MapTile::new(3205, 3207);
    RegionDiff {
        region: RegionCoord::new(50, 50),
        tiles: vec![(
            MapPosition::new(tile, plane0()),
            TerrainTile {
                underlay: Some(FloorUnderlayId::new(12)),
                overlay: Some(TerrainOverlay {
                    id: FloorOverlayId::new(7),
                    shape: OverlayShape::new(4).unwrap_or_else(|| unreachable!("valid")),
                    rotation: OverlayRotation::new(2).unwrap_or_else(|| unreachable!("valid")),
                }),
                flags: 5,
            },
        )],
        heights: vec![(HeightVertexCoord::new(tile, plane0()), -53)],
        objects_added: vec![ObjectPlacement {
            object_id: ObjectId::new(2600),
            position: MapPosition::new(MapTile::new(3220, 3221), plane0()),
            kind: LocKind::new(10).unwrap_or_else(|| unreachable!("valid")),
            orientation: ObjectOrientation::new(3).unwrap_or_else(|| unreachable!("valid")),
        }],
        objects_removed: Vec::new(),
    }
}

#[test]
fn a_project_round_trips_through_json_exactly() {
    let diff = sample_diff();
    let file = ProjectFile::from_diffs(
        "osrs-live-241-2026-09-30-openrs2-2727",
        std::slice::from_ref(&diff),
    );
    let parsed = ProjectFile::from_json(&file.to_json()).unwrap_or_else(|e| unreachable!("{e:?}"));
    assert_eq!(parsed, file);
    assert_eq!(
        parsed
            .into_diffs()
            .unwrap_or_else(|e| unreachable!("{e:?}")),
        vec![diff]
    );
}

#[test]
fn empty_diffs_are_not_recorded() {
    let mut empty = sample_diff();
    empty.tiles.clear();
    empty.heights.clear();
    empty.objects_added.clear();
    assert!(ProjectFile::from_diffs("p", &[empty]).regions.is_empty());
}

#[test]
fn unknown_formats_are_rejected_before_any_value_is_read() {
    let text = r#"{"format":"rustosrs-project-v99","profile":"p","regions":[]}"#;
    assert_eq!(
        ProjectFile::from_json(text),
        Err(ProjectError::UnsupportedFormat(
            "rustosrs-project-v99".to_owned()
        ))
    );
    assert!(matches!(
        ProjectFile::from_json("not json"),
        Err(ProjectError::Json(_))
    ));
}

#[test]
fn out_of_range_values_are_rejected_not_clamped() {
    let mut file = ProjectFile::from_diffs("p", &[sample_diff()]);
    file.regions[0].tiles[0][2] = 4; // plane
    assert_eq!(
        file.clone().into_diffs(),
        Err(ProjectError::Invalid("tile plane"))
    );
    file.regions[0].tiles[0][2] = 0;
    file.regions[0].tiles[0][5] = 12; // overlay shape
    assert_eq!(
        file.clone().into_diffs(),
        Err(ProjectError::Invalid("overlay shape"))
    );
    file.regions[0].tiles[0][5] = 4;
    file.regions[0].tiles[0][7] = 300; // flags
    assert_eq!(
        file.clone().into_diffs(),
        Err(ProjectError::Invalid("tile flags"))
    );
    file.regions[0].tiles[0][7] = 5;
    file.regions[0].objects_added[0][0] = -1; // object id
    assert_eq!(file.into_diffs(), Err(ProjectError::Invalid("object id")));
}
