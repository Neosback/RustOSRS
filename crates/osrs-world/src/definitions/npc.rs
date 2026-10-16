use super::WorldDefinitions;
use osrs_core::{entity::NpcDefinition, ids::NpcId, lighting::ReferenceLitModel};
use std::{collections::HashMap, sync::Arc};
#[derive(Default)]
pub(super) struct NpcCache {
    files: Option<HashMap<u32, osrs_cache::transport::CacheFile>>,
    varbit_files: Option<HashMap<u32, osrs_cache::transport::CacheFile>>,
    definitions: HashMap<NpcId, Arc<NpcDefinition>>,
    bases: HashMap<NpcId, Arc<ReferenceLitModel>>,
}
/// One searchable NPC definition summary; the full definition is read on demand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NpcCatalogEntry {
    pub id: NpcId,
    pub name: Option<String>,
    pub size: u16,
    pub renderable: bool,
}
impl WorldDefinitions {
    /// Summaries of every decodable NPC definition, plus per-definition decode failures.
    pub fn npc_catalog(&mut self) -> (Vec<NpcCatalogEntry>, Vec<String>) {
        if self.npcs.files.is_none() {
            match self.cache().read_group_files(2, 9) {
                Ok(files) => self.npcs.files = Some(files.into_iter().collect()),
                Err(e) => return (Vec::new(), vec![e.to_string()]),
            }
        }
        let mut ids: Vec<u32> = self
            .npcs
            .files
            .as_ref()
            .map(|files| files.keys().copied().collect())
            .unwrap_or_default();
        ids.sort_unstable();
        let (mut entries, mut errors) = (Vec::with_capacity(ids.len()), Vec::new());
        for raw in ids {
            let id = NpcId::new(raw);
            match self.npc(id) {
                Ok(Some(d)) => entries.push(NpcCatalogEntry {
                    id,
                    name: d.name.clone(),
                    size: d.size,
                    renderable: !d.body_models.is_empty(),
                }),
                Ok(None) => {}
                Err(e) => errors.push(format!("NPC {raw}: {e}")),
            }
        }
        (entries, errors)
    }
    pub fn npc(&mut self, id: NpcId) -> Result<Option<Arc<NpcDefinition>>, String> {
        if let Some(definition) = self.npcs.definitions.get(&id) {
            return Ok(Some(definition.clone()));
        }
        if self.npcs.files.is_none() {
            self.npcs.files = Some(
                self.cache()
                    .read_group_files(2, 9)
                    .map_err(|e| e.to_string())?
                    .into_iter()
                    .collect(),
            );
        }
        let Some(file) = self
            .npcs
            .files
            .as_ref()
            .and_then(|files| files.get(&id.get()))
        else {
            return Ok(None);
        };
        let definition = Arc::new(
            osrs_cache::decode::decode_npc_definition(
                id,
                &file.bytes,
                &self.context,
                &file.provenance,
            )
            .map_err(|e| e.to_string())?,
        );
        self.npcs.definitions.insert(id, definition.clone());
        Ok(Some(definition))
    }
    pub fn npc_base(&mut self, id: NpcId) -> Result<Option<Arc<ReferenceLitModel>>, String> {
        if let Some(base) = self.npcs.bases.get(&id) {
            return Ok(Some(base.clone()));
        }
        let Some(definition) = self.npc(id)? else {
            return Ok(None);
        };
        let Some(base) = osrs_cache::npc_model::build_npc_base(&mut self.models, &definition)?
        else {
            return Ok(None);
        };
        let base = Arc::new(base);
        self.npcs.bases.insert(id, base.clone());
        Ok(Some(base))
    }
    /// NPC lit bases are projections; retain only active requested definitions.
    pub fn retain_npc_bases(&mut self, ids: &std::collections::BTreeSet<NpcId>) {
        self.npcs.bases.retain(|id, _| ids.contains(id));
    }
}

impl WorldDefinitions {
    /// Resolve nested morphs using explicit runtime variables (unset varps read 0). A varbit
    /// definition missing from the cache is an error; a null transform renders no NPC.
    pub fn resolve_npc(
        &mut self,
        id: NpcId,
        varps: &std::collections::BTreeMap<osrs_core::ids::VarpId, i32>,
    ) -> Result<Option<NpcId>, String> {
        use osrs_core::{
            definitions::VarbitDefinition,
            ids::VarbitId,
            morph::{MorphVariableState, resolve_npc_morph},
        };
        struct Vars<'a> {
            values: &'a std::collections::BTreeMap<osrs_core::ids::VarpId, i32>,
            bit: Option<VarbitDefinition>,
        }
        impl MorphVariableState for Vars<'_> {
            /// A varp nobody has set holds its default of 0, as in a new account.
            fn varp_value(&self, id: osrs_core::ids::VarpId) -> Option<i32> {
                Some(self.values.get(&id).copied().unwrap_or(0))
            }
            fn varbit_definition(&self, id: VarbitId) -> Option<&VarbitDefinition> {
                self.bit.as_ref().filter(|v| v.identity.id == id)
            }
        }
        let mut seen = std::collections::BTreeSet::new();
        let mut current = id;
        loop {
            if !seen.insert(current) {
                return Err(format!("cyclic NPC morph at {current}"));
            }
            let definition = self
                .npc(current)?
                .ok_or_else(|| format!("missing NPC morph definition {current}"))?;
            let Some(morphs) = &definition.morphs else {
                return Ok(Some(current));
            };
            let bit = if let Some(bit) = morphs.transform_varbit {
                if self.npcs.varbit_files.is_none() {
                    self.npcs.varbit_files = Some(
                        self.cache()
                            .read_group_files(2, 14)
                            .map_err(|e| e.to_string())?
                            .into_iter()
                            .collect(),
                    );
                }
                let file = self
                    .npcs
                    .varbit_files
                    .as_ref()
                    .and_then(|files| files.get(&bit.get()))
                    .ok_or_else(|| format!("missing NPC morph varbit {bit}"))?;
                Some(
                    osrs_cache::decode::decode_varbit(
                        bit,
                        &file.bytes,
                        &self.context,
                        &file.provenance,
                    )
                    .map_err(|e| e.to_string())?,
                )
            } else {
                None
            };
            let Some(next) = resolve_npc_morph(morphs, &Vars { values: varps, bit })
                .map_err(|e| e.to_string())?
            else {
                return Ok(None);
            };
            current = next;
        }
    }
}
