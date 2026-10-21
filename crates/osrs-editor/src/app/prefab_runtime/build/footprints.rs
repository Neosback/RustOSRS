//! Frozen definition facts for a building request. No document, catalog or model is retained.
use super::*;
use std::collections::BTreeMap;

pub(super) type SizeSource<'a> = dyn Fn(ObjectId, LocKind) -> Option<(u8, u8)> + 'a;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Fact {
    source: Option<(u8, u8)>,
    declared: Option<(u8, u8)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FootprintInputs(BTreeMap<(ObjectId, LocKind), Fact>);

impl FootprintInputs {
    pub(super) fn capture(
        document: &MapDocument,
        kit: &KitFile,
        source: &SizeSource<'_>,
    ) -> Result<Self, String> {
        let mut facts = BTreeMap::new();
        for p in document.object_entries() {
            if p.kind.uses_definition_footprint() {
                facts.entry((p.object_id, p.kind)).or_insert_with(|| Fact {
                    source: source(p.object_id, p.kind),
                    declared: None,
                });
            }
        }
        for dependency in kit
            .footprint_requirements()
            .map_err(|error| error.to_string())?
        {
            let fact = facts
                .entry((dependency.object, dependency.kind))
                .or_insert_with(|| Fact {
                    source: source(dependency.object, dependency.kind),
                    declared: None,
                });
            fact.declared = dependency.declared_size;
        }
        Ok(Self(facts))
    }

    pub(super) fn matches(&self, source: &SizeSource<'_>) -> bool {
        self.0
            .iter()
            .all(|(&(id, kind), fact)| fact.source == source(id, kind))
    }

    pub(super) fn resolve(&self, id: ObjectId, kind: LocKind) -> Result<(u8, u8), String> {
        self.0.get(&(id, kind)).and_then(|fact| fact.source.or(fact.declared))
            .ok_or_else(|| format!("Footprint unavailable for object {} (kind {}); load its source definitions or declare its furniture size in the kit", id.get(), kind.get()))
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Private, complete wire facts; source dimensions and portable declarations stay distinct.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Record {
    id: ObjectId,
    kind: u8,
    #[serde(deserialize_with = "super::reproduction::form::required_option")]
    source: Option<(u8, u8)>,
    #[serde(deserialize_with = "super::reproduction::form::required_option")]
    declared: Option<(u8, u8)>,
}
#[cfg(not(target_arch = "wasm32"))]
impl FootprintInputs {
    pub(super) fn records(&self) -> Vec<Record> {
        self.0
            .iter()
            .map(|(&(id, kind), f)| Record {
                id,
                kind: kind.get(),
                source: f.source,
                declared: f.declared,
            })
            .collect()
    }
    pub(super) fn restore(
        records: Vec<Record>,
        document: &MapDocument,
        kit: &KitFile,
    ) -> Result<Self, String> {
        let mut facts = BTreeMap::new();
        for r in records {
            let kind = LocKind::new(r.kind).ok_or("Invalid footprint loc kind")?;
            if facts
                .insert(
                    (r.id, kind),
                    Fact {
                        source: r.source,
                        declared: r.declared,
                    },
                )
                .is_some()
            {
                return Err("Duplicate footprint fact".into());
            }
        }
        let captured = Self::capture(document, kit, &|id, kind| {
            facts.get(&(id, kind)).and_then(|f| f.source)
        })?;
        if captured.0 != facts {
            return Err("Footprint facts do not match the document and kit requirements".into());
        }
        Ok(captured)
    }
}
