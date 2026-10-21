//! Definition dependencies of every asset a kit can emit, independent of a particular seed.
use super::*;
use std::collections::BTreeMap;

/// A scenery definition needed by the kit. Furniture sizes are portable kit declarations;
/// a caller with source definitions must prefer those definitions over these hints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KitFootprint {
    pub object: ObjectId,
    pub kind: LocKind,
    pub declared_size: Option<(u8, u8)>,
}

impl KitFile {
    /// All definition-sized assets, deduplicated by object and loc kind. Structural kinds need
    /// no definition size. Invalid kinds and conflicting furniture size declarations are errors.
    pub fn footprint_requirements(&self) -> Result<Vec<KitFootprint>, KitError> {
        let mut facts = BTreeMap::<(ObjectId, LocKind), Option<(u8, u8)>>::new();
        let mut add = |id, raw_kind, size: Option<(u8, u8)>| -> Result<(), KitError> {
            let kind = kind(raw_kind)?;
            if !kind.uses_definition_footprint() {
                return Ok(());
            }
            let declared = facts.entry((ObjectId::new(id), kind)).or_default();
            if let Some(size) = size {
                let (w, h) =
                    crate::prefab::footprint(ObjectId::new(id), kind.get(), 0, &|_, _| size);
                let size = (w as u8, h as u8);
                if declared.is_some_and(|previous| previous != size) {
                    return Err(KitError::Value {
                        what: "conflicting furniture footprint for object",
                        value: id,
                    });
                }
                *declared = Some(size);
            }
            Ok(())
        };
        for asset in [
            Some(&self.exterior_wall),
            Some(&self.interior_wall),
            Some(&self.door),
            self.window.as_ref(),
            self.corner.as_ref(),
        ]
        .into_iter()
        .flatten()
        .chain(self.door_variants.iter().map(|v| &v.asset))
        {
            add(asset.id, asset.kind, None)?;
        }
        if let Some(roof) = &self.roof {
            for pattern in &roof.patterns {
                add(pattern.id, pattern.kind, None)?;
            }
        }
        // Hip roofs emit only structural one-tile kinds (12..20).
        if let Some(stairs) = &self.stairs {
            for asset in [&stairs.up, &stairs.down] {
                add(asset.id, asset.kind, None)?;
            }
        }
        if let Some(finish) = &self.finish {
            for furniture in finish.furniture.values().flatten() {
                add(
                    furniture.id,
                    furniture.kind,
                    Some((furniture.size[0], furniture.size[1])),
                )?;
            }
            for decor in &finish.wall_decor {
                add(decor.id, decor.kind, None)?;
            }
            if let Some(window) = &finish.window_decor {
                for decor in [window.outside.as_ref(), window.inside.as_ref()]
                    .into_iter()
                    .flatten()
                {
                    add(decor.id, decor.kind, None)?;
                }
            }
        }
        Ok(facts
            .into_iter()
            .map(|((object, kind), declared_size)| KitFootprint {
                object,
                kind,
                declared_size,
            })
            .collect())
    }
}
