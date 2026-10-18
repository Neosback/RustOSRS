//! Relocation curriculum uses the production foundation planner; policies see tools only.
use crate::{LabTask, PaletteEntry, Snapshot};
use osrs_core::coords::{MapTile, SourcePlane};
use osrs_edit::{PlaceSpec, Prefab, PrefabHeight, PrefabSource, plan_place_prefab};
pub fn relocation_task(prefab: &Prefab, seed: u64) -> Result<LabTask, String> {
    prefab.validate().map_err(|e| e.to_string())?;
    let b = prefab
        .building
        .as_ref()
        .filter(|b| b.reviewed)
        .ok_or("Relocation needs a reviewed building prefab")?;
    let (w, h) = osrs_edit::turned_size(prefab.width, prefab.height, (seed % 4) as u8);
    let width = w
        .checked_add(6)
        .filter(|w| *w <= 128)
        .ok_or("Building is too wide for a Lab relocation task")?;
    let height = h
        .checked_add(6)
        .filter(|h| *h <= 128)
        .ok_or("Building is too tall for a Lab relocation task")?;
    let planes = prefab.planes;
    let initial = Snapshot {
        tiles: vec![],
        objects: vec![],
        heights: (0..planes)
            .flat_map(|p| {
                (0..=width).flat_map(move |x| {
                    (0..=height).map(move |y| PrefabHeight {
                        x,
                        y,
                        plane: p,
                        value: -800 - i32::from(p) * 240
                            + (i32::from(x) + i32::from(y) + (seed % 8) as i32) * 8,
                    })
                })
            })
            .collect(),
    };
    let mut document = initial.document(width, height, planes)?;
    let patch = plan_place_prefab(
        &document,
        prefab,
        PlaceSpec {
            origin: MapTile::new(3, 3),
            base_plane: SourcePlane::new(0).ok_or("plane")?,
            quarter_turns: (seed % 4) as u8,
            terrain: true,
            heights: true,
            objects: true,
            replace_objects: false,
        },
        &|id, _| {
            b.object_sizes
                .get(&id.get())
                .map_or((1, 1), |&(w, h)| (w as u8, h as u8))
        },
    )
    .map_err(|e| e.to_string())?;
    document
        .apply_patch(&patch.patch)
        .map_err(|e| format!("{e:?}"))?;
    let target = Snapshot::capture(&document);
    let palette = prefab
        .objects
        .iter()
        .map(|o| {
            (
                (o.id, o.kind),
                PaletteEntry {
                    id: o.id,
                    kind: o.kind,
                    size: b
                        .object_sizes
                        .get(&o.id)
                        .map_or([1, 1], |&(w, h)| [w as u8, h as u8]),
                },
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>()
        .into_values()
        .collect();
    let group = match prefab.source {
        PrefabSource::Captured { tile_x, tile_y, .. } => format!(
            "geography:{}:{}",
            tile_x.div_euclid(512),
            tile_y.div_euclid(512)
        ),
        _ => format!(
            "family:{}",
            prefab.name.split(" copy").next().unwrap_or(&prefab.name)
        ),
    };
    Ok(LabTask {
        format: "rustosrs-lab-task".into(),
        version: 1,
        name: format!("Relocate {}", prefab.name),
        source: b.provenance.clone(),
        group,
        width,
        height,
        planes,
        budget: 8192,
        initial,
        target,
        palette,
        overlays: prefab
            .tiles
            .iter()
            .filter_map(|t| t.overlay.as_ref().map(|o| o.id))
            .collect(),
        underlays: prefab.tiles.iter().filter_map(|t| t.underlay).collect(),
    })
}
