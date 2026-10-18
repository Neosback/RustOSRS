//! Host-side task construction from a saved building. Agent actions still expose only tools.
use crate::{LabTask, PaletteEntry, Snapshot};
use osrs_core::ids::ObjectId;
use osrs_edit::{LocKind, Prefab, PrefabHeight};
pub fn reconstruction_task(
    p: &Prefab,
    size_of: &dyn Fn(ObjectId, LocKind) -> Option<(u16, u16)>,
) -> Result<LabTask, String> {
    p.validate().map_err(|e| e.to_string())?;
    let mut palette = std::collections::BTreeMap::new();
    for o in &p.objects {
        let size = p.building.as_ref().and_then(|b| b.object_sizes.get(&o.id)).copied()
            .or_else(|| size_of(ObjectId::new(o.id), LocKind::new(o.kind)?))
            .ok_or_else(|| format!("Missing dimensions for object {}. Load its definitions or review the building before reconstruction.",o.id))?;
        let w = u8::try_from(size.0)
            .ok()
            .filter(|w| *w > 0)
            .ok_or("Object width exceeds Lab limits")?;
        let h = u8::try_from(size.1)
            .ok()
            .filter(|h| *h > 0)
            .ok_or("Object height exceeds Lab limits")?;
        palette.insert(
            (o.id, o.kind),
            PaletteEntry {
                id: o.id,
                kind: o.kind,
                size: [w, h],
            },
        );
    }
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
    Ok(LabTask {
        format: "rustosrs-lab-task".into(),
        version: 1,
        name: p.name.clone(),
        source: p
            .building
            .as_ref()
            .map_or_else(|| "prefab".into(), |b| b.provenance.clone()),
        group: match p.source {
            osrs_edit::PrefabSource::Captured { tile_x, tile_y, .. } => format!(
                "geography:{}:{}",
                tile_x.div_euclid(512),
                tile_y.div_euclid(512)
            ),
            _ => format!("family:{}", p.name.split(" copy").next().unwrap_or(&p.name)),
        },
        width: p.width,
        height: p.height,
        planes: p.planes,
        budget: 4096,
        initial,
        target,
        palette: palette.into_values().collect(),
        overlays: p
            .tiles
            .iter()
            .filter_map(|t| t.overlay.as_ref().map(|o| o.id))
            .collect(),
        underlays: p.tiles.iter().filter_map(|t| t.underlay).collect(),
        dont_care_tiles: vec![],
        dont_care_heights: vec![],
    })
}
