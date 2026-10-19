//! Multi-prefab learning reuses the lens extractor, including sparse holes and human overrides.
use super::*;
use crate::{Prefab, WfcPieceClass};

pub fn learn_prefab_catalog(
    prefabs: &[Prefab],
    style: String,
    role: String,
    source: String,
    classify: &dyn Fn(u32, u8) -> WfcPieceClass,
) -> Result<LearnedCatalog, LearningError> {
    let mut grids = Vec::new();
    for prefab in prefabs {
        let building = prefab.building.as_ref().ok_or(LearningError::Unreviewed)?;
        if !building.reviewed {
            return Err(LearningError::Unreviewed);
        }
        for plane in 0..prefab.planes {
            if !building.floors.iter().any(|c| c.plane == plane) {
                continue;
            }
            let lens =
                crate::wfc_look(prefab, plane, classify).map_err(|_| LearningError::InvalidGrid)?;
            let cells = lens
                .cells
                .iter()
                .map(|id| {
                    id.map(|id| lens.catalog.patterns[usize::from(id)].clone())
                        .unwrap_or(LearnedPattern {
                            shape: 0,
                            rotation: 0,
                            floor_class: 0,
                            sockets: [0; 4],
                        })
                })
                .collect();
            grids.push(LearningGrid {
                width: lens.width,
                height: lens.height,
                cells,
                reviewed: true,
                active: lens.cells.iter().map(Option::is_some).collect(),
            });
        }
    }
    learn_catalog_with_rotations(&grids, style, role, source)
}
