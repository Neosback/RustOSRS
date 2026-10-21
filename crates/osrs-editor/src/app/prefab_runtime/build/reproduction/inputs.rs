//! Both outcome formats validate the same frozen intent through canonical owners.
use super::*;

pub(super) fn prepare(i: &Inputs) -> Result<(MapDocument, Arc<PreviewInput>, Kit), Error> {
    i.source.validate()?;
    i.editor_build.validate()?;
    let origin = MapTile::new(i.origin[0], i.origin[1]);
    let plane =
        SourcePlane::new(i.plane).ok_or_else(|| Error::Input("Invalid source plane".into()))?;
    form::validate(&i.request, origin).map_err(Error::Input)?;
    form::validate(&i.candidate, origin).map_err(Error::Input)?;
    let mut requested = i.request.clone();
    requested.seed = i.candidate.seed;
    if requested != i.candidate || i.candidate.seed.wrapping_sub(i.request.seed) >= 8 {
        return Err(Error::Input(
            "Candidate does not belong to its captured request".into(),
        ));
    }
    let document = MapDocument::from_snapshot_json_v1(&i.initial_map_json)
        .map_err(|e| Error::Input(e.to_string()))?;
    let file: KitFile = serde_json::from_str(&i.kit_json)?;
    let kit = file.compile().map_err(|e| Error::Input(e.to_string()))?;
    let footprints =
        FootprintInputs::restore(i.footprints.clone(), &document, &file).map_err(Error::Input)?;
    let input = Arc::new(PreviewInput {
        form: i.request.clone(),
        origin,
        plane,
        source: i.source_generation,
        revision: i.semantic_revision,
        footprints,
    });
    Ok((
        document,
        input,
        Kit {
            name: kit.name().into(),
            kit,
            file,
        },
    ))
}
