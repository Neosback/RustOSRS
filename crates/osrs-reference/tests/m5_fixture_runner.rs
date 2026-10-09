use osrs_reference::inventory::FixtureInventory;
use osrs_reference::loader::FixtureRepository;
use osrs_reference::runner::run_inventory;
use std::path::Path;

fn repository() -> Result<FixtureRepository, Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reference-fixtures");
    Ok(FixtureRepository::new(root)?)
}

#[test]
fn discovers_and_executes_every_checked_in_yaml_fixture() -> Result<(), Box<dyn std::error::Error>>
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
        ]
    );

    let report = run_inventory(&inventory)?;
    assert_eq!(report.len(), 3);
    assert_eq!(report.fixture_ids(), ids);
    Ok(())
}
