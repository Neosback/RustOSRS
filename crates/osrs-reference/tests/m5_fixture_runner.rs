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
            "normals.base.smooth_triangle",
            "normals.merge.coincident_triangle.hide_false",
            "normals.merge.coincident_triangle.hide_true",
            "normals.merge.translated_negative",
            "planes.link_below.four_plane_column",
            "priority.all_0_11.threshold_crossing",
        ]
    );

    let report = run_inventory(&inventory)?;
    assert_eq!(report.len(), 10);
    assert_eq!(report.fixture_ids(), ids);
    Ok(())
}

#[test]
fn evidence_only_cannot_disable_an_existing_semantic_executor()
-> Result<(), Box<dyn std::error::Error>> {
    let inventory = FixtureInventory::discover(&repository()?)?;
    let Some(existing) = inventory
        .fixtures()
        .iter()
        .find(|fixture| fixture.manifest.fixture_id == "model.mirror.geometry_winding")
    else {
        panic!("missing model mirror fixture");
    };

    let mut fixture = existing.clone();
    fixture.manifest.execution = FixtureExecution::EvidenceOnly;

    let Err(error) = run_fixture(&fixture) else {
        panic!("evidence_only unexpectedly bypassed an implemented semantic executor");
    };
    assert!(
        error
            .detail()
            .contains("evidence_only is not permitted for implemented fixture kind")
    );
    Ok(())
}
