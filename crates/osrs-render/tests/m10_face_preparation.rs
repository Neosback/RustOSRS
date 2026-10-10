use osrs_core::model::FacePriority;
use osrs_render::{
    ReferenceFaceAlpha, ReferencePriorityFace, ReferencePriorityThresholds,
    prepare_reference_priority_order,
};
use std::error::Error;

const INPUT_FIXTURE: &str = include_str!(
    "../../../reference-fixtures/priority/all_0_11_threshold_crossing.input.json"
);
const EXPECTED_FIXTURE: &str = include_str!(
    "../../../reference-fixtures/priority/all_0_11_threshold_crossing.expected.json"
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FixtureFace {
    id: usize,
    priority: u8,
    depth_bucket: i32,
    alpha: Option<i8>,
}

#[derive(Debug, PartialEq, Eq)]
struct FixtureExpected {
    ordered_face_ids: Vec<usize>,
    thresholds: ReferencePriorityThresholds,
}

#[test]
fn production_priority_order_executes_the_pinned_m5_fixture() -> Result<(), Box<dyn Error>> {
    let input = parse_input_fixture(INPUT_FIXTURE)?;
    let expected = parse_expected_fixture(EXPECTED_FIXTURE)?;

    let mut faces = Vec::with_capacity(input.len());
    for face in &input {
        let priority = FacePriority::new(face.priority).ok_or_else(|| {
            std::io::Error::other(format!("fixture priority {} is invalid", face.priority))
        })?;
        faces.push(ReferencePriorityFace::new(
            face.id,
            priority,
            face.depth_bucket,
        ));
    }

    let order = prepare_reference_priority_order(&faces);

    assert_eq!(
        order.ordered_face_indices(),
        expected.ordered_face_ids.as_slice()
    );
    assert_eq!(order.thresholds(), expected.thresholds);
    assert_eq!(
        order.priority_counts(),
        &[1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 4, 4]
    );
    Ok(())
}

#[test]
fn reference_alpha_interpretation_preserves_raw_sentinel_semantics()
-> Result<(), Box<dyn Error>> {
    let input = parse_input_fixture(INPUT_FIXTURE)?;

    let absent = input
        .iter()
        .find(|face| face.id == 0)
        .ok_or_else(|| std::io::Error::other("fixture face 0 missing"))?;
    let ordinary = input
        .iter()
        .find(|face| face.id == 10)
        .ok_or_else(|| std::io::Error::other("fixture face 10 missing"))?;
    let sentinel = input
        .iter()
        .find(|face| face.id == 14)
        .ok_or_else(|| std::io::Error::other("fixture face 14 missing"))?;

    let absent_alpha = ReferenceFaceAlpha::from_raw(absent.alpha);
    assert_eq!(absent_alpha.raw(), None);
    assert_eq!(absent_alpha.rasterizer_alpha(), 0);
    assert!(absent_alpha.is_reference_opaque());

    let ordinary_alpha = ReferenceFaceAlpha::from_raw(ordinary.alpha);
    assert_eq!(ordinary_alpha.raw(), Some(64));
    assert_eq!(ordinary_alpha.rasterizer_alpha(), 64);
    assert!(!ordinary_alpha.is_minus_one_sentinel());

    let sentinel_alpha = ReferenceFaceAlpha::from_raw(sentinel.alpha);
    assert_eq!(sentinel_alpha.raw(), Some(-1));
    assert_eq!(sentinel_alpha.rasterizer_alpha(), 253);
    assert!(sentinel_alpha.is_minus_one_sentinel());

    assert_eq!(ReferenceFaceAlpha::from_raw(Some(0)).rasterizer_alpha(), 0);
    assert_eq!(ReferenceFaceAlpha::from_raw(Some(-2)).rasterizer_alpha(), 254);
    Ok(())
}

fn parse_input_fixture(input: &str) -> Result<Vec<FixtureFace>, Box<dyn Error>> {
    let mut faces = Vec::new();
    for line in input.lines().map(str::trim) {
        if !line.starts_with("{\"id\":") {
            continue;
        }

        let cleaned = line
            .trim_end_matches(',')
            .trim_start_matches('{')
            .trim_end_matches('}');
        let mut id = None;
        let mut priority = None;
        let mut depth_bucket = None;
        let mut alpha = None;
        let mut alpha_seen = false;

        for field in cleaned.split(", ") {
            let (name, value) = field
                .split_once(": ")
                .ok_or_else(|| std::io::Error::other("invalid fixture face field"))?;
            match name.trim_matches('"') {
                "id" => id = Some(value.parse()?),
                "priority" => priority = Some(value.parse()?),
                "depth_bucket" => depth_bucket = Some(value.parse()?),
                "alpha" => {
                    alpha_seen = true;
                    alpha = if value == "null" {
                        None
                    } else {
                        Some(value.parse()?)
                    };
                }
                other => {
                    return Err(std::io::Error::other(format!(
                        "unexpected fixture face field {other}"
                    ))
                    .into());
                }
            }
        }

        if !alpha_seen {
            return Err(std::io::Error::other("fixture face alpha field missing").into());
        }
        faces.push(FixtureFace {
            id: id.ok_or_else(|| std::io::Error::other("fixture face id missing"))?,
            priority: priority
                .ok_or_else(|| std::io::Error::other("fixture face priority missing"))?,
            depth_bucket: depth_bucket
                .ok_or_else(|| std::io::Error::other("fixture face depth missing"))?,
            alpha,
        });
    }

    if faces.is_empty() {
        return Err(std::io::Error::other("priority fixture contains no faces").into());
    }
    Ok(faces)
}

fn parse_expected_fixture(input: &str) -> Result<FixtureExpected, Box<dyn Error>> {
    let ordered_line = input
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("\"ordered_face_ids\""))
        .ok_or_else(|| std::io::Error::other("expected ordered_face_ids missing"))?;
    let (_, ordered_tail) = ordered_line
        .split_once('[')
        .ok_or_else(|| std::io::Error::other("expected ordered_face_ids is invalid"))?;
    let (ordered_values, _) = ordered_tail
        .split_once(']')
        .ok_or_else(|| std::io::Error::other("expected ordered_face_ids is unterminated"))?;
    let ordered_face_ids = ordered_values
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::parse)
        .collect::<Result<Vec<usize>, _>>()?;

    Ok(FixtureExpected {
        ordered_face_ids,
        thresholds: ReferencePriorityThresholds {
            avg12: parse_expected_integer(input, "avg12")?,
            avg34: parse_expected_integer(input, "avg34")?,
            avg68: parse_expected_integer(input, "avg68")?,
        },
    })
}

fn parse_expected_integer(input: &str, key: &str) -> Result<i64, Box<dyn Error>> {
    let prefix = format!("\"{key}\":");
    let line = input
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with(&prefix))
        .ok_or_else(|| std::io::Error::other(format!("expected {key} missing")))?;
    let (_, value) = line
        .split_once(':')
        .ok_or_else(|| std::io::Error::other(format!("expected {key} is invalid")))?;
    Ok(value.trim().trim_end_matches(',').parse()?)
}
