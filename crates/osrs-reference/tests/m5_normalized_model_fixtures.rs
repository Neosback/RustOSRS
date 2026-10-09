use osrs_reference::loader::FixtureRepository;
use osrs_reference::schema::{
    NormalizedExpectedCase, NormalizedFixtureExpected, NormalizedFixtureInput,
    NormalizedFixtureKind, NormalizedInputCase, NormalizedObjectModels,
};
use std::path::Path;

fn repository() -> Result<FixtureRepository, Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reference-fixtures");
    Ok(FixtureRepository::new(root)?)
}

#[test]
fn first_m5_model_fixtures_are_hash_verified_and_schema_typed()
-> Result<(), Box<dyn std::error::Error>> {
    let repository = repository()?;
    for (manifest, kind) in [
        (
            "manifest/model-selection-typed-orientation-4.yaml",
            NormalizedFixtureKind::ModelSelection,
        ),
        (
            "manifest/model-mirror-geometry-winding.yaml",
            NormalizedFixtureKind::ModelMirror,
        ),
        (
            "manifest/model-transform-type4-order.yaml",
            NormalizedFixtureKind::ModelTransform,
        ),
    ] {
        let fixture = repository.load(manifest)?;
        let input = NormalizedFixtureInput::parse(&fixture.input)?;
        let expected = NormalizedFixtureExpected::parse(&fixture.expected)?;
        assert_eq!(input.kind(), kind);
        assert_eq!(expected.kind(), kind);
    }
    Ok(())
}

#[test]
fn typed_selection_fixture_preserves_exact_selection_and_mirror_result()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = repository()?.load("manifest/model-selection-typed-orientation-4.yaml")?;
    let input = NormalizedFixtureInput::parse(&fixture.input)?;
    let expected = NormalizedFixtureExpected::parse(&fixture.expected)?;

    match input.case {
        NormalizedInputCase::ModelSelection {
            models: NormalizedObjectModels::Typed { entries },
            is_rotated,
            requested_type,
            orientation,
        } => {
            assert_eq!(entries.len(), 2);
            assert_eq!(entries[0].loc_type, 2);
            assert_eq!(entries[0].model_id, 100);
            assert_eq!(entries[1].loc_type, 4);
            assert_eq!(entries[1].model_id, 200);
            assert!(!is_rotated);
            assert_eq!(requested_type, 4);
            assert_eq!(orientation, 4);
        }
        _ => return Err("selection input decoded as the wrong normalized case".into()),
    }

    match expected.case {
        NormalizedExpectedCase::ModelSelection {
            selection: Some(selection),
        } => {
            assert_eq!(selection.model_ids, [200]);
            assert!(selection.mirror);
        }
        _ => return Err("selection expected output decoded as the wrong normalized case".into()),
    }
    Ok(())
}

#[test]
fn mirror_and_type4_fixtures_preserve_order_sensitive_integer_outputs()
-> Result<(), Box<dyn std::error::Error>> {
    let repository = repository()?;

    let mirror = repository.load("manifest/model-mirror-geometry-winding.yaml")?;
    let mirror_expected = NormalizedFixtureExpected::parse(&mirror.expected)?;
    match mirror_expected.case {
        NormalizedExpectedCase::ModelMirror { vertices, faces } => {
            assert_eq!(vertices.len(), 3);
            assert_eq!((vertices[0].x, vertices[0].y, vertices[0].z), (1, 2, -3));
            assert_eq!((vertices[1].x, vertices[1].y, vertices[1].z), (4, 5, 6));
            assert_eq!((vertices[2].x, vertices[2].y, vertices[2].z), (7, 8, 9));
            assert_eq!(faces.len(), 1);
            assert_eq!((faces[0].a, faces[0].b, faces[0].c), (2, 1, 0));
        }
        _ => return Err("mirror expected output decoded as the wrong normalized case".into()),
    }

    let transform = repository.load("manifest/model-transform-type4-order.yaml")?;
    let transform_input = NormalizedFixtureInput::parse(&transform.input)?;
    match transform_input.case {
        NormalizedInputCase::ModelTransform {
            vertices,
            face_colors,
            face_textures,
            requested_type,
            orientation,
            ..
        } => {
            assert_eq!(requested_type, 4);
            assert_eq!(orientation, 5);
            assert_eq!(vertices.len(), 3);
            assert_eq!((vertices[0].x, vertices[0].y, vertices[0].z), (128, 64, 0));
            assert_eq!((vertices[1].x, vertices[1].y, vertices[1].z), (0, 0, 0));
            assert_eq!((vertices[2].x, vertices[2].y, vertices[2].z), (0, 128, 0));
            assert_eq!(face_colors, [100]);
            assert_eq!(face_textures, [Some(7)]);
        }
        _ => return Err("transform input decoded as the wrong normalized case".into()),
    }

    let transform_expected = NormalizedFixtureExpected::parse(&transform.expected)?;
    match transform_expected.case {
        NormalizedExpectedCase::ModelTransform {
            vertices,
            face_colors,
            face_textures,
        } => {
            assert_eq!(vertices.len(), 3);
            assert_eq!(
                (vertices[0].x, vertices[0].y, vertices[0].z),
                (-262, 27, -115)
            );
            assert_eq!((vertices[1].x, vertices[1].y, vertices[1].z), (-80, -5, -25));
            assert_eq!((vertices[2].x, vertices[2].y, vertices[2].z), (-80, 59, -25));
            assert_eq!(face_colors, [300]);
            assert_eq!(face_textures, [Some(9)]);
        }
        _ => return Err("transform expected output decoded as the wrong normalized case".into()),
    }
    Ok(())
}
