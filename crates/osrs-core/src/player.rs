//! Player appearance: which identity kits and worn items fill the twelve body slots, and the
//! colours that tint them. Values follow the client's `PlayerComposition`; assembling the model
//! belongs to `osrs-cache`, which reads the kit and item definitions.
use crate::{
    definitions::{DefinitionIdentity, RecolorPair, RetexturePair},
    ids::{ItemId, KitId, ModelId},
};
use std::collections::BTreeMap;

/// Twelve body slots in appearance-packet order: head 0, cape 1, amulet 2, weapon 3, torso 4,
/// shield 5, arms 6, legs 7, hair 8, hands 9, feet 10, jaw 11.
pub const SLOTS: usize = 12;
/// Identity-kit body parts: hair, jaw, torso, arms, hands, legs, feet (female parts are +7).
pub const KIT_PARTS: usize = 7;
/// The body slot each kit part fills.
pub const KIT_SLOT: [usize; KIT_PARTS] = [8, 11, 4, 6, 9, 7, 10];
/// Tinted colour slots: hair, torso, legs, feet, skin.
pub const COLOUR_SLOTS: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Gender {
    #[default]
    Male,
    Female,
}

/// What fills one body slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SlotContent {
    #[default]
    Empty,
    Kit(KitId),
    Item(ItemId),
}

/// The twelve slots plus colours: everything the model of a player depends on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PlayerAppearance {
    pub gender: Gender,
    pub slots: [SlotContent; SLOTS],
    pub colours: [u8; COLOUR_SLOTS],
}

/// A worn item and the slots it covers (its own, plus up to two it hides).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WornItem {
    pub slot: u8,
    pub item: ItemId,
    pub hides: [Option<u8>; 2],
}

impl PlayerAppearance {
    /// Put kits in their slots, then worn items over them; an item hides the kit (or empties
    /// the slot) of each slot it covers. A slot holding a worn item is never hidden.
    pub fn compose(
        gender: Gender,
        kits: [Option<KitId>; KIT_PARTS],
        colours: [u8; COLOUR_SLOTS],
        worn: &[WornItem],
    ) -> Self {
        let mut slots = [SlotContent::Empty; SLOTS];
        for (part, kit) in kits.iter().enumerate() {
            if let Some(kit) = kit {
                slots[KIT_SLOT[part]] = SlotContent::Kit(*kit);
            }
        }
        for item in worn {
            if let Some(slot) = slots.get_mut(usize::from(item.slot)) {
                *slot = SlotContent::Item(item.item);
            }
        }
        for item in worn {
            for hidden in item.hides.into_iter().flatten() {
                if let Some(slot) = slots.get_mut(usize::from(hidden))
                    && !matches!(slot, SlotContent::Item(_))
                {
                    *slot = SlotContent::Empty;
                }
            }
        }
        Self {
            gender,
            slots,
            colours,
        }
    }
}

/// What a player looks like, as chosen: explicit kits (all `None` means the default look), body
/// colours, and worn items by body slot. [`PlayerAppearance::compose`] turns it into slots.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct PlayerLook {
    pub gender: Gender,
    pub kits: [Option<KitId>; KIT_PARTS],
    pub colours: [u8; COLOUR_SLOTS],
    pub worn: Vec<(u8, ItemId)>,
}

/// An identity kit: a body part with its own models and recolours.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KitDefinition {
    pub identity: DefinitionIdentity<KitId>,
    /// 0 to 6 for male parts, 7 to 13 for female; `None` when the definition names none.
    pub body_part: Option<u8>,
    pub models: Vec<ModelId>,
    pub non_selectable: bool,
    pub recolors: Vec<RecolorPair>,
    pub retextures: Vec<RetexturePair>,
}

impl KitDefinition {
    /// Whether this kit can fill `part` (0 to 6) for `gender`.
    pub fn fits(&self, part: usize, gender: Gender) -> bool {
        let wanted = match gender {
            Gender::Male => part,
            Gender::Female => part + 7,
        };
        self.body_part.is_some_and(|p| usize::from(p) == wanted)
    }
}

/// The first selectable kit for each part: what a new character wears before any choice.
pub fn default_kits<'a>(
    gender: Gender,
    kits: impl IntoIterator<Item = &'a KitDefinition>,
) -> [Option<KitId>; KIT_PARTS] {
    let mut by_id: BTreeMap<KitId, &KitDefinition> = BTreeMap::new();
    for kit in kits {
        by_id.insert(kit.identity.id, kit);
    }
    let mut chosen = [None; KIT_PARTS];
    for (part, slot) in chosen.iter_mut().enumerate() {
        *slot = by_id
            .values()
            .find(|kit| !kit.non_selectable && kit.fits(part, gender))
            .map(|kit| kit.identity.id);
    }
    chosen
}

/// Models a worn item shows for one gender.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WearModels {
    pub models: [Option<ModelId>; 3],
    /// Vertical shift applied to the first model (in model units).
    pub offset: u8,
}

/// The parts of an item definition that decide how it looks when worn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemDefinition {
    pub identity: DefinitionIdentity<ItemId>,
    pub name: Option<String>,
    /// Body slot the item is worn in.
    pub equip_slot: Option<u8>,
    /// Further slots the item hides.
    pub hides: [Option<u8>; 2],
    pub male: WearModels,
    pub female: WearModels,
    pub recolors: Vec<RecolorPair>,
    pub retextures: Vec<RetexturePair>,
}

impl ItemDefinition {
    pub fn worn_models(&self, gender: Gender) -> WearModels {
        match gender {
            Gender::Male => self.male,
            Gender::Female => self.female,
        }
    }

    pub fn worn(&self, slot: u8) -> WornItem {
        WornItem {
            slot: self.equip_slot.unwrap_or(slot),
            item: self.identity.id,
            hides: self.hides,
        }
    }
}

/// Face colours the body tints replace (hair, torso, legs, feet, skin), from the client.
const RECOLOR_FROM: [i16; COLOUR_SLOTS] = [6798, 8741, 25238, 4626, 4550];
/// The palette of each colour slot.
const RECOLOR_TO: [&[i16]; COLOUR_SLOTS] = [
    &[
        6798, 107, 10283, 16, 4797, 7744, 5799, 4634, -31839, 22433, 2983, -11343, 8, 5281, 10438,
        3650, -27322, -21845, 200, 571, 908, 21830, 28946, -15701, -14010, -22122, 937, 8130,
        -13422, 30385,
    ],
    &[
        8741, 12, -1506, -22374, 7735, 8404, 1701, -27106, 24094, 10153, -8915, 4783, 1341, 16578,
        -30533, 25239, 8, 5281, 10438, 3650, -27322, -21845, 200, 571, 908, 21830, 28946, -15701,
        -14010,
    ],
    &[
        25238, 8742, 12, -1506, -22374, 7735, 8404, 1701, -27106, 24094, 10153, -8915, 4783, 1341,
        16578, -30533, 8, 5281, 10438, 3650, -27322, -21845, 200, 571, 908, 21830, 28946, -15701,
        -14010,
    ],
    &[4626, 11146, 6439, 12, 4758, 10270],
    &[
        4550, 4537, 5681, 5673, 5790, 6806, 8076, 4574, 17050, 0, 127, -31821, -17991,
    ],
];
/// A second colour the first two slots also replace, and its palettes.
const TINT_FROM: [i16; COLOUR_SLOTS] = [-10304, 9104, -1, -1, -1];
const TINT_TO: [&[i16]; COLOUR_SLOTS] = [
    &[
        6554, 115, 10304, 28, 5702, 7756, 5681, 4510, -31835, 22437, 2859, -11339, 16, 5157, 10446,
        3658, -27314, -21965, 472, 580, 784, 21966, 28950, -15697, -14002, -22116, 945, 8144,
        -13414, 30389,
    ],
    &[
        9104, 10275, 7595, 3610, 7975, 8526, 918, -26734, 24466, 10145, -6882, 5027, 1457, 16565,
        -30545, 25486, 24, 5392, 10429, 3673, -27335, -21957, 192, 687, 412, 21821, 28835, -15460,
        -14019,
    ],
    &[],
    &[],
    &[],
];

/// The face-colour replacements the body colours ask for, in the order the client applies them.
pub fn body_recolors(colours: [u8; COLOUR_SLOTS]) -> Vec<RecolorPair> {
    let pair = |from: i16, to: i16| RecolorPair {
        from: from as u16,
        to: to as u16,
    };
    let mut out = Vec::new();
    for slot in 0..COLOUR_SLOTS {
        let choice = usize::from(colours[slot]);
        if let Some(to) = RECOLOR_TO[slot].get(choice) {
            out.push(pair(RECOLOR_FROM[slot], *to));
        }
        if let Some(to) = TINT_TO[slot].get(choice) {
            out.push(pair(TINT_FROM[slot], *to));
        }
    }
    out
}

/// Retextures are not used for body colours; kept as a type so callers share one signature.
pub type BodyRetextures = Vec<RetexturePair>;

#[cfg(test)]
mod tests;
