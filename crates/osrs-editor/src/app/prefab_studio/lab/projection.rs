//! Boundary conversion between task-local semantics and the isolated stage.
use super::*;
use osrs_edit::{PrefabHeight, PrefabOverlay, PrefabSource, PrefabTile};
use osrs_lab::{PaletteEntry, Snapshot};
pub(super) fn task_from_prefab(p: &Prefab) -> LabTask {
    let datum = p.building.as_ref().map_or(0, |b| b.source_datum);
    let initial = Snapshot {
        tiles: vec![],
        objects: vec![],
        heights: (0..p.planes)
            .flat_map(|plane| {
                (0..=p.width).flat_map(move |x| {
                    (0..=p.height).map(move |y| PrefabHeight {
                        x,
                        y,
                        plane,
                        value: datum - i32::from(plane) * 240,
                    })
                })
            })
            .collect(),
    };
    let mut target = initial.clone();
    target.tiles = p.tiles.clone();
    target.objects = p.objects.clone();
    for h in &p.heights {
        if let Some(existing) = target
            .heights
            .iter_mut()
            .find(|v| (v.x, v.y, v.plane) == (h.x, h.y, h.plane))
        {
            *existing = h.clone();
        }
    }
    LabTask {
        format: "rustosrs-lab-task".into(),
        version: 1,
        name: p.name.clone(),
        source: p
            .building
            .as_ref()
            .map_or_else(|| "prefab".into(), |b| b.provenance.clone()),
        group: format!("prefab:{}", p.name),
        width: p.width,
        height: p.height,
        planes: p.planes,
        budget: 4096,
        initial,
        target,
        palette: p
            .objects
            .iter()
            .map(|o| PaletteEntry {
                id: o.id,
                kind: o.kind,
                size: p
                    .building
                    .as_ref()
                    .and_then(|b| b.object_sizes.get(&o.id))
                    .map_or([1, 1], |&(x, y)| [x as u8, y as u8]),
            })
            .collect(),
        overlays: p
            .tiles
            .iter()
            .filter_map(|t| t.overlay.as_ref().map(|o| o.id))
            .collect(),
        underlays: p.tiles.iter().filter_map(|t| t.underlay).collect(),
        dont_care_tiles: vec![],
        dont_care_heights: vec![],
    }
}
pub(super) fn project(viewer: &mut EditorApp, pad: &MapDocument, env: &Env) -> Result<(), String> {
    let legal = env.legal_mask();
    let snapshot = env.observation(ObservationMode::ScoreOnly).current;
    let mut tiles = Vec::new();
    for p in 0..legal.planes {
        let plane = SourcePlane::new(p).ok_or("plane")?;
        for x in 0..legal.width {
            for y in 0..legal.height {
                let t = env.document().terrain_at(osrs_edit::MapPosition::new(
                    MapTile::new(i32::from(x), i32::from(y)),
                    plane,
                ));
                tiles.push(PrefabTile {
                    x,
                    y,
                    plane: p,
                    underlay: t.underlay.map(|i| i.get()),
                    overlay: t.overlay.map(|o| PrefabOverlay {
                        id: o.id.get(),
                        shape: o.shape.get(),
                        rotation: o.rotation.get(),
                    }),
                    flags: t.flags,
                });
            }
        }
    }
    let prefab = Prefab {
        format: osrs_edit::PREFAB_FORMAT.into(),
        version: osrs_edit::PREFAB_VERSION,
        name: "Lab replay".into(),
        tags: vec![],
        notes: String::new(),
        source: PrefabSource::Authored,
        width: legal.width,
        height: legal.height,
        planes: legal.planes,
        tiles,
        heights: snapshot.heights,
        objects: snapshot.objects,
        building: None,
    };
    let mut desired = pad.clone();
    let origin = MapTile::new(
        CENTRE.0 - i32::from(legal.width) / 2,
        CENTRE.1 - i32::from(legal.height) / 2,
    );
    let patch = plan_place_exact(
        &desired,
        &prefab,
        PlaceSpec {
            origin,
            base_plane: SourcePlane::new(0).ok_or("plane")?,
            quarter_turns: 0,
            terrain: true,
            heights: true,
            objects: true,
            replace_objects: false,
        },
        &|id, kind| {
            legal
                .palette
                .iter()
                .find(|p| p.id == id.get() && p.kind == kind.get())
                .map_or((1, 1), |p| (p.size[0], p.size[1]))
        },
    )
    .map_err(|e| e.to_string())?
    .patch;
    desired.apply_patch(&patch).map_err(|e| format!("{e:?}"))?;
    let patch = plan_document_sync(viewer.session.document(), &desired)
        .map_err(|_| "Lab scene height coverage is incomplete")?;
    if !patch.is_empty()
        && !crate::app::editing::apply_baseline_patch(viewer, patch, "Lab replay projection")
    {
        return Err("Lab replay projection failed".into());
    }
    viewer.interaction.clear_selection();
    Ok(())
}
