const FIXTURES: &str = include_str!("../../../reference-fixtures/model/m4-p0-p1.txt");

#[derive(Debug, PartialEq, Eq)]
struct Fixture<'a> {
    name: &'a str,
    spec: &'a str,
    artifact: &'a str,
}

fn fixtures() -> Result<Vec<Fixture<'static>>, Box<dyn std::error::Error>> {
    let mut fixtures = Vec::new();
    for (line_number, raw) in FIXTURES.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts = line.split('|').collect::<Vec<_>>();
        if parts.len() != 3 {
            return Err(format!(
                "fixture line {} has {} fields; expected 3",
                line_number + 1,
                parts.len()
            )
            .into());
        }
        fixtures.push(Fixture {
            name: parts[0],
            spec: parts[1],
            artifact: parts[2],
        });
    }
    Ok(fixtures)
}

#[test]
fn checked_in_m4_fixture_inventory_is_exact() -> Result<(), Box<dyn std::error::Error>> {
    let fixtures = fixtures()?;
    let expected = [
        ("typed_selection", "MODEL-BUILD-001"),
        ("untyped_type10_selection", "MODEL-BUILD-001"),
        ("mirror_geometry_winding", "MODEL-BUILD-002"),
        ("multi_model_combine", "MODEL-BUILD-001"),
        ("complex_texture_preservation", "MODEL-BUILD-001"),
        ("type4_transform_order", "MODEL-BUILD-003+COORD-002"),
        ("ordinary_orientation", "MODEL-BUILD-003+COORD-002"),
        ("instance_source_isolation", "MODEL-BUILD-005-PARTIAL"),
    ];

    assert_eq!(fixtures.len(), expected.len());
    for (fixture, (name, spec)) in fixtures.iter().zip(expected) {
        assert_eq!(fixture.name, name);
        assert_eq!(fixture.spec, spec);
        assert!(
            !fixture.artifact.is_empty(),
            "fixture {name} must name its executable verification artifact"
        );
    }

    Ok(())
}
