use osrs_reference::fixture::FixtureExecution;
use osrs_reference::inventory::FixtureInventory;
use osrs_reference::loader::FixtureRepository;
use osrs_reference::runner::{run_fixture, run_inventory};
use std::path::Path;

fn repository() -> Result<FixtureRepository, Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reference-fixtures");
    Ok(FixtureRepository::new(root)?)
}

#[test]
fn discovers_and_validates_every_checked_in_yaml_fixture() -> Result<(), Box<dyn std::error::Error>>
{
    let inventory = FixtureInventory::discover(&repository()?)?;
    let ids = inventory
        .fixtures()
        .iter()
        .map(|fixture| fixture.manifest.fixture_id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        ids,
        [
            "model.mirror.geometry_winding",
            "model.selection.typed_exact.orientation_4",
            "model.transform.type4_order",
            "normals.base.flat_triangle",
            "normals.base.mixed_signed_faces",
            "normals.base.smooth_triangle",
            "normals.merge.coincident_triangle.hide_false",
            "normals.merge.coincident_triangle.hide_true",
            "normals.merge.duplicate_vertices",
            "normals.merge.flat_neighbor",
            "normals.merge.translated_negative",
            "normals.merge.translated_shared_face",
            "normals.merge.two_shared_vertices",
            "planes.link_below.four_plane_column",
            "priority.all_0_11.threshold_crossing",
        ]
    );

    let report = run_inventory(&inventory)?;
    assert_eq!(report.len(), 15);
    assert_eq!(report.fixture_ids(), ids);
    Ok(())
}

#[test]
fn evidence_only_cannot_disable_an_existing_semantic_executor()
-> Result<(), Box<dyn std::error::Error>> {
    let inventory = FixtureInventory::discover(&repository()?)?;
    for existing in inventory
        .fixtures()
        .iter()
        .filter(|fixture| !fixture.manifest.fixture_id.starts_with("priority."))
    {
        let mut fixture = existing.clone();
        fixture.manifest.execution = FixtureExecution::EvidenceOnly;
        let Err(error) = run_fixture(&fixture) else {
            panic!("evidence-only bypassed {}", fixture.manifest.fixture_id);
        };
        assert!(
            error
                .detail()
                .contains("evidence_only is not permitted for implemented fixture kind"),
            "{}: {error}",
            fixture.manifest.fixture_id
        );
    }
    Ok(())
}

#[test]
fn canonical_runner_executes_every_normal_fixture_and_detects_wrong_expected_values()
-> Result<(), Box<dyn std::error::Error>> {
    let inventory = FixtureInventory::discover(&repository()?)?;
    let mut count = 0;
    for existing in inventory
        .fixtures()
        .iter()
        .filter(|fixture| fixture.manifest.fixture_id.starts_with("normals."))
    {
        let mut fixture = existing.clone();
        fixture.manifest.execution = FixtureExecution::Semantic;
        run_fixture(&fixture)?;
        let mut expected =
            osrs_reference::schema::NormalizedFixtureExpected::parse(&fixture.expected)?;
        match &mut expected.case {
            osrs_reference::schema::NormalizedExpectedCase::BaseNormals {
                vertex_normals, ..
            } => vertex_normals[0].x += 1,
            osrs_reference::schema::NormalizedExpectedCase::NormalMerge {
                left_face_render_types,
                ..
            } => *left_face_render_types = Some(vec![99]),
            _ => panic!("unexpected normal fixture kind"),
        }
        fixture.expected = serde_yaml_ng::to_string(&expected)?.into_bytes();
        let Err(error) = run_fixture(&fixture) else {
            panic!(
                "wrong expected values passed {}",
                fixture.manifest.fixture_id
            );
        };
        assert!(
            error
                .detail()
                .contains("exact normalized semantic mismatch"),
            "{error}"
        );
        count += 1;
    }
    assert_eq!(count, 10, "normal inventory unexpectedly changed");
    Ok(())
}

#[test]
fn normal_runner_rejects_invalid_model_inputs_with_fixture_context()
-> Result<(), Box<dyn std::error::Error>> {
    use osrs_reference::schema::{NormalizedFixtureInput, NormalizedInputCase};

    let inventory = FixtureInventory::discover(&repository()?)?;
    let Some(original) = inventory
        .fixtures()
        .iter()
        .find(|fixture| fixture.manifest.fixture_id == "normals.base.smooth_triangle")
    else {
        panic!("missing smooth-normal fixture");
    };
    for invalid in 0..3 {
        let mut fixture = original.clone();
        let mut input = NormalizedFixtureInput::parse(&fixture.input)?;
        let NormalizedInputCase::BaseNormals { model } = &mut input.case else {
            panic!("expected base normals");
        };
        match invalid {
            0 => model.face_render_types = Some(vec![255]),
            1 => model.face_render_types = Some(vec![]),
            _ => model.faces[0].a = 99,
        }
        fixture.input = serde_yaml_ng::to_string(&input)?.into_bytes();
        let Err(error) = run_fixture(&fixture) else {
            panic!("invalid model must not execute");
        };
        assert_eq!(error.fixture_id(), fixture.manifest.fixture_id);
        assert!(!error.detail().contains("semantic mismatch"), "{error}");
    }
    Ok(())
}
