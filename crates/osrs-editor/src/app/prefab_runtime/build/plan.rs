//! One candidate plans against a frozen document and passes shared semantic validation.
use super::*;
mod error;
pub(super) use error::{Failure, Stage};

pub(super) fn plan(
    input: Arc<PreviewInput>,
    form: &BuildForm,
    kit: &Kit,
    document: &MapDocument,
) -> Result<Preview, Failure> {
    let size_of = |id, kind| input.footprints.resolve(id, kind);
    let origin = input.origin;
    let base_plane = input.plane;
    if form.mode != GenerationMode::Generate && form.sketch.is_none() {
        return Err(Failure::request(
            "Use a drawn room layout before completing, furnishing, restyling or repairing",
        ));
    }
    if form.mode == GenerationMode::Repair && form.region.is_none() {
        return Err(Failure::request("Select a repair area first"));
    }
    if form.stage.is_some() && form.sketch.is_none() {
        return Err(Failure::request(
            "Keep a drawn room layout before rerolling a single stage",
        ));
    }
    let site = RectSite::new(origin, form.width, form.height)
        .ok_or_else(|| Failure::request("the site is empty"))?;
    let request = BuildingRequest {
        seed: form.seed,
        site,
        base_plane,
        floors: form.floors.clamp(1, 3),
        archetype: form.archetype,
        style: form.style,
        constraints: BuildingConstraints::default(),
    };
    let snapshot = capture_generation_snapshot(document, request, []);
    let generated = match &form.sketch {
        Some(sketch) => kit.kit.generate_in_rooms(&snapshot, form.variation, sketch),
        None => kit.kit.generate(&snapshot, form.variation),
    }
    .map_err(|error| Failure::new(Stage::Generation, describe(&error), format!("{error:?}")))?;
    let building = design::filter_generated(&generated, form, &kit.kit, document);
    let mut ops = Vec::new();
    let mut removed = 0;
    if form.clear_site && form.mode != GenerationMode::Complete {
        let base = base_plane.index().get();
        for offset in 0..=form.floors.clamp(1, 3) {
            let Some(plane) = SourcePlane::new(base.saturating_add(offset)) else {
                continue;
            };
            for placement in osrs_edit::try_objects_overlapping_rect(
                document,
                (origin.x, origin.y),
                (
                    origin.x + i32::from(form.width),
                    origin.y + i32::from(form.height),
                ),
                plane,
                &size_of,
            )
            .map_err(|message| Failure::new(Stage::Clearing, message.clone(), message))?
            {
                if form.locks.contains(&placement)
                    || !design::includes(&MapOp::RemoveObject { placement }, form, &kit.kit)
                {
                    continue;
                }
                ops.push(MapOp::RemoveObject { placement });
                removed += 1;
            }
        }
    }
    // A piece already standing exactly as the building wants it stays; adding it again would
    // make the whole patch invalid (and removing then re-adding it would be pointless).
    let removing: std::collections::HashSet<_> = ops
        .iter()
        .filter_map(|op| match op {
            MapOp::RemoveObject { placement } => Some(*placement),
            _ => None,
        })
        .collect();
    let mut standing = 0;
    for op in building.ops() {
        match op {
            MapOp::AddObject { placement }
                if document.contains_object(*placement) && !removing.contains(placement) =>
            {
                standing += 1;
            }
            other => ops.push(*other),
        }
    }
    let objects = building
        .ops()
        .iter()
        .filter(|op| matches!(op, MapOp::AddObject { .. }))
        .count();
    let apply = MapPatch::new(ops);
    document.validate_patch(&apply).map_err(|error| {
        let message = format!(
            "the building does not fit the map: {}",
            describe_apply(&error)
        );
        Failure::new(Stage::MapValidation, message, format!("{error:?}"))
    })?;
    let verdict = design::validate(document, &apply, form, kit, origin, base_plane, &size_of)
        .map_err(|message| Failure::new(Stage::CandidateValidation, message.clone(), message))?;
    let thumbnail = prefab_from_patch(
        &building,
        origin,
        base_plane,
        form.width,
        form.height,
        "candidate",
        PrefabSource::Generated {
            archetype: format!("{:?}", form.archetype),
            style: kit.name.clone(),
            seed: form.seed,
            floors: form.floors,
        },
    )
    .map_err(|e| Failure::new(Stage::Thumbnail, e.to_string(), format!("{e:?}")))?;
    Ok(Preview {
        input,
        origin,
        base_plane,
        form: form.clone(),
        building,
        apply,
        objects,
        removed,
        standing,
        verdict,
        thumbnail,
    })
}
