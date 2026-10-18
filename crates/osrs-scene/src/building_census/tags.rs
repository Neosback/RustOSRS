//! What a room is for, read from what stands in it. Names are the evidence: a room with a bank
//! booth is a bank, one with a range and a sink is a kitchen. Hints only; nothing keys on them.

use super::{Furniture, ObjectTable};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RoomTag {
    Bank,
    Kitchen,
    Bedroom,
    Church,
    Shop,
    Tavern,
    Study,
    Smithy,
    Storage,
    Armory,
    Cell,
    Living,
    General,
}

impl RoomTag {
    pub const fn word(self) -> &'static str {
        match self {
            Self::Bank => "bank",
            Self::Kitchen => "kitchen",
            Self::Bedroom => "bedroom",
            Self::Church => "church",
            Self::Shop => "shop",
            Self::Tavern => "tavern",
            Self::Study => "study",
            Self::Smithy => "smithy",
            Self::Storage => "storage",
            Self::Armory => "armory",
            Self::Cell => "cell",
            Self::Living => "living",
            Self::General => "general",
        }
    }
}

/// Words that point to a tag, with a weight: strong evidence outweighs a stray chair.
const EVIDENCE: &[(RoomTag, &str, u32)] = &[
    (RoomTag::Bank, "bankbooth", 8),
    (RoomTag::Bank, "bank_booth", 8),
    (RoomTag::Bank, "bank booth", 8),
    (RoomTag::Bank, "bank chest", 8),
    (RoomTag::Bank, "bank_chest", 8),
    (RoomTag::Bank, "deposit", 6),
    (RoomTag::Bank, "bank", 4),
    (RoomTag::Kitchen, "range", 4),
    (RoomTag::Kitchen, "stove", 4),
    (RoomTag::Kitchen, "oven", 4),
    (RoomTag::Kitchen, "sink", 3),
    (RoomTag::Kitchen, "cooking", 3),
    (RoomTag::Bedroom, "bed", 4),
    (RoomTag::Bedroom, "wardrobe", 3),
    (RoomTag::Bedroom, "dresser", 2),
    (RoomTag::Church, "altar", 6),
    (RoomTag::Church, "pew", 4),
    (RoomTag::Church, "lectern", 2),
    (RoomTag::Shop, "stall", 3),
    (RoomTag::Shop, "shopcounter", 5),
    (RoomTag::Shop, "shop_counter", 5),
    (RoomTag::Shop, "counter", 2),
    (RoomTag::Shop, "till", 2),
    (RoomTag::Tavern, "beer", 4),
    (RoomTag::Tavern, "keg", 3),
    (RoomTag::Tavern, "bar", 1),
    (RoomTag::Tavern, "stool", 1),
    (RoomTag::Study, "bookcase", 3),
    (RoomTag::Study, "bookshelf", 3),
    (RoomTag::Study, "desk", 2),
    (RoomTag::Smithy, "anvil", 5),
    (RoomTag::Smithy, "furnace", 4),
    (RoomTag::Smithy, "forge", 4),
    (RoomTag::Storage, "crate", 2),
    (RoomTag::Storage, "sack", 2),
    (RoomTag::Storage, "barrel", 1),
    (RoomTag::Storage, "chest", 1),
    (RoomTag::Armory, "weaponrack", 5),
    (RoomTag::Armory, "weapon rack", 5),
    (RoomTag::Armory, "armour", 3),
    (RoomTag::Cell, "cell", 4),
    (RoomTag::Cell, "prison", 4),
    (RoomTag::Cell, "shackle", 3),
    (RoomTag::Living, "table", 2),
    (RoomTag::Living, "chair", 2),
    (RoomTag::Living, "fireplace", 3),
    (RoomTag::Living, "rug", 2),
    (RoomTag::Living, "carpet", 2),
];

/// The room's tag: the one with the most evidence, `General` when nothing says anything.
pub fn tag_room(furniture: &[Furniture], objects: &ObjectTable) -> RoomTag {
    let mut scores = std::collections::BTreeMap::<RoomTag, u32>::new();
    for piece in furniture {
        let Some(info) = objects.get(&piece.object) else {
            continue;
        };
        for (tag, word, weight) in EVIDENCE {
            if info.says(word) {
                *scores.entry(*tag).or_default() += weight;
            }
        }
    }
    scores
        .into_iter()
        .max_by_key(|(tag, score)| (*score, std::cmp::Reverse(*tag)))
        .filter(|(_, score)| *score >= 2)
        .map_or(RoomTag::General, |(tag, _)| tag)
}
