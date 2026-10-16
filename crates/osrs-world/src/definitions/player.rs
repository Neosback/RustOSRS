//! Kit and item definitions and assembled player bases, read lazily from the cache.
use super::WorldDefinitions;
use osrs_core::{
    ids::{ItemId, KitId},
    lighting::ReferenceLitModel,
    player::{
        ItemDefinition, KitDefinition, PlayerAppearance, PlayerLook, SlotContent, WornItem,
        default_kits,
    },
};
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

/// Config archive groups holding identity kits and items.
const KIT_GROUP: u32 = 3;
const ITEM_GROUP: u32 = 10;
/// Assembled bases kept at once (a look changes rarely; a few cover quick edits).
const MAX_BASES: usize = 8;

#[derive(Default)]
pub(super) struct PlayerCache {
    kit_files: Option<HashMap<u32, osrs_cache::transport::CacheFile>>,
    item_files: Option<HashMap<u32, osrs_cache::transport::CacheFile>>,
    kits: HashMap<KitId, Arc<KitDefinition>>,
    items: HashMap<ItemId, Arc<ItemDefinition>>,
    bases: Vec<(PlayerAppearance, Arc<ReferenceLitModel>)>,
}

impl WorldDefinitions {
    fn config_files(
        &self,
        group: u32,
    ) -> Result<HashMap<u32, osrs_cache::transport::CacheFile>, String> {
        Ok(self
            .cache()
            .read_group_files(2, group)
            .map_err(|e| e.to_string())?
            .into_iter()
            .collect())
    }

    pub fn kit(&mut self, id: KitId) -> Result<Option<Arc<KitDefinition>>, String> {
        if let Some(kit) = self.players.kits.get(&id) {
            return Ok(Some(kit.clone()));
        }
        if self.players.kit_files.is_none() {
            self.players.kit_files = Some(self.config_files(KIT_GROUP)?);
        }
        let Some(file) = self
            .players
            .kit_files
            .as_ref()
            .and_then(|files| files.get(&id.get()))
        else {
            return Ok(None);
        };
        let kit = Arc::new(
            osrs_cache::decode::decode_kit_definition(
                id,
                &file.bytes,
                &self.context,
                &file.provenance,
            )
            .map_err(|e| e.to_string())?,
        );
        self.players.kits.insert(id, kit.clone());
        Ok(Some(kit))
    }

    pub fn item(&mut self, id: ItemId) -> Result<Option<Arc<ItemDefinition>>, String> {
        if let Some(item) = self.players.items.get(&id) {
            return Ok(Some(item.clone()));
        }
        if self.players.item_files.is_none() {
            self.players.item_files = Some(self.config_files(ITEM_GROUP)?);
        }
        let Some(file) = self
            .players
            .item_files
            .as_ref()
            .and_then(|files| files.get(&id.get()))
        else {
            return Ok(None);
        };
        let item = Arc::new(
            osrs_cache::decode::decode_item_definition(
                id,
                &file.bytes,
                &self.context,
                &file.provenance,
            )
            .map_err(|e| e.to_string())?,
        );
        self.players.items.insert(id, item.clone());
        Ok(Some(item))
    }

    /// Every identity kit of the cache, by ID.
    pub fn all_kits(&mut self) -> Result<Vec<Arc<KitDefinition>>, String> {
        if self.players.kit_files.is_none() {
            self.players.kit_files = Some(self.config_files(KIT_GROUP)?);
        }
        let mut ids: Vec<u32> = self
            .players
            .kit_files
            .as_ref()
            .map(|files| files.keys().copied().collect())
            .unwrap_or_default();
        ids.sort_unstable();
        let mut kits = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(kit) = self.kit(KitId::new(id))? {
                kits.push(kit);
            }
        }
        Ok(kits)
    }

    /// The twelve body slots a look produces. An unset look wears the default kits; worn items
    /// replace and hide slots by their definitions.
    pub fn compose_player(&mut self, look: &PlayerLook) -> Result<PlayerAppearance, String> {
        let kits = if look.kits.iter().all(Option::is_none) {
            let all = self.all_kits()?;
            default_kits(look.gender, all.iter().map(Arc::as_ref))
        } else {
            look.kits
        };
        let mut worn: Vec<WornItem> = Vec::new();
        for (slot, id) in &look.worn {
            let item = self
                .item(*id)?
                .ok_or_else(|| format!("missing item {id}"))?;
            worn.push(item.worn(*slot));
        }
        Ok(PlayerAppearance::compose(
            look.gender,
            kits,
            look.colours,
            &worn,
        ))
    }

    /// The unposed lit model of a player (shared; posing copies it).
    pub fn player_base(
        &mut self,
        appearance: &PlayerAppearance,
    ) -> Result<Option<Arc<ReferenceLitModel>>, String> {
        if let Some((_, base)) = self.players.bases.iter().find(|(a, _)| a == appearance) {
            return Ok(Some(base.clone()));
        }
        let mut kits = BTreeMap::new();
        let mut items = BTreeMap::new();
        for slot in appearance.slots {
            match slot {
                SlotContent::Empty => {}
                SlotContent::Kit(id) => {
                    let kit = self.kit(id)?.ok_or_else(|| format!("missing kit {id}"))?;
                    kits.insert(id, kit);
                }
                SlotContent::Item(id) => {
                    let item = self.item(id)?.ok_or_else(|| format!("missing item {id}"))?;
                    items.insert(id, item);
                }
            }
        }
        let Some(base) = osrs_cache::player_model::build_player_base(
            &mut self.models,
            appearance,
            &kits,
            &items,
        )?
        else {
            return Ok(None);
        };
        let base = Arc::new(base);
        if self.players.bases.len() >= MAX_BASES {
            self.players.bases.remove(0);
        }
        self.players.bases.push((*appearance, base.clone()));
        Ok(Some(base))
    }
}
