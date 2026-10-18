//! Persist an explicitly accepted reconstruction through the normal prefab library.
use super::*;
pub(super) fn prefab(viewer: &EditorApp, original: &Prefab, env: &Env) -> Prefab {
    let score = env.score();
    let current = env.observation(ObservationMode::ScoreOnly).current;
    let legal = env.legal_mask();
    let building = (!current.tiles.is_empty()).then(|| {
        let floors: Vec<_> = current
            .tiles
            .iter()
            .map(|t| osrs_edit::PrefabCell {
                x: t.x,
                y: t.y,
                plane: t.plane,
            })
            .collect();
        let mut support = std::collections::BTreeSet::new();
        for f in &floors {
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                support.insert(osrs_edit::PrefabCell {
                    x: f.x + dx,
                    y: f.y + dy,
                    plane: f.plane,
                });
            }
        }
        let mut object_sizes = original
            .building
            .as_ref()
            .map(|b| b.object_sizes.clone())
            .unwrap_or_default();
        for o in &current.objects {
            let (w, h) = *object_sizes.entry(o.id).or_insert_with(|| {
                let (sw, sh) = viewer.placer.object_size(o.id);
                (u16::from(sw), u16::from(sh))
            });
            let (w, h) = if o.orientation & 1 == 1 {
                (h, w)
            } else {
                (w, h)
            };
            for dx in 0..=w {
                for dy in 0..=h {
                    if o.x + dx <= legal.width && o.y + dy <= legal.height {
                        support.insert(osrs_edit::PrefabCell {
                            x: o.x + dx,
                            y: o.y + dy,
                            plane: o.plane,
                        });
                    }
                }
            }
        }
        let height_set: std::collections::BTreeSet<_> = current
            .heights
            .iter()
            .map(|h| (h.x, h.y, h.plane))
            .collect();
        support.retain(|c| height_set.contains(&(c.x, c.y, c.plane)));
        let source_datum = current
            .heights
            .iter()
            .find(|h| h.plane == 0)
            .map_or(0, |h| h.value);
        osrs_edit::PrefabBuilding {
            floors,
            support: support.into_iter().collect(),
            source_datum,
            object_sizes,
            provenance: original
                .building
                .as_ref()
                .map_or_else(|| "lab-accepted".into(), |b| b.provenance.clone()),
            relations: vec![],
            reviewed: true,
            labels: Default::default(),
        }
    });
    let mut candidate = Prefab {
        format: osrs_edit::PREFAB_FORMAT.into(),
        version: osrs_edit::PREFAB_VERSION,
        name: format!("{} reconstruction", original.name),
        tags: vec!["lab-reviewed".into()],
        notes: format!(
            "Accepted local reconstruction; score {} seed {}",
            score.total,
            env.seed()
        ),
        source: osrs_edit::PrefabSource::Authored,
        width: legal.width,
        height: legal.height,
        planes: legal.planes,
        tiles: current.tiles,
        heights: current.heights,
        objects: current.objects,
        building,
    };
    if candidate.validate().is_err() {
        candidate.building = None;
    }
    candidate
}
