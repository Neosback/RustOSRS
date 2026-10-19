//! Canonical names and descriptions for OSRS Player Variables (VarPlayers / varps)
//! and Varbits, derived from RuneLite deobfuscated constants and client cache schemas.

/// A known VarPlayer definition with id, Jagex identifier, and human-readable label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VarpInfo {
    pub id: u32,
    pub name: &'static str,
    pub description: &'static str,
}

/// A known Varbit definition with id, Jagex identifier, and human-readable label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VarbitInfo {
    pub id: u32,
    pub name: &'static str,
    pub description: &'static str,
}

/// Curated table of prominent OSRS VarPlayer IDs in ascending order for binary search.
pub const VARPS: &[VarpInfo] = &[
    VarpInfo {
        id: 0,
        name: "MCANNON",
        description: "Dwarf multicannon state",
    },
    VarpInfo {
        id: 2,
        name: "DROPCANNON",
        description: "Cannon placement status",
    },
    VarpInfo {
        id: 3,
        name: "CANNON_AMMO",
        description: "Cannonballs loaded in cannon",
    },
    VarpInfo {
        id: 4,
        name: "CANNON_COORD",
        description: "Cannon world coordinates",
    },
    VarpInfo {
        id: 5,
        name: "GRAIL",
        description: "Holy Grail quest progress",
    },
    VarpInfo {
        id: 10,
        name: "COGQUEST",
        description: "Clock Tower quest progress",
    },
    VarpInfo {
        id: 18,
        name: "MUSICPLAY",
        description: "Currently playing music track",
    },
    VarpInfo {
        id: 19,
        name: "MUSICLOOP",
        description: "Music loop mode",
    },
    VarpInfo {
        id: 29,
        name: "COOKQUEST",
        description: "Cook's Assistant quest progress",
    },
    VarpInfo {
        id: 30,
        name: "DRUNKMONKQUEST",
        description: "Monk's Friend quest progress",
    },
    VarpInfo {
        id: 31,
        name: "DORICQUEST",
        description: "Doric's Quest progress",
    },
    VarpInfo {
        id: 32,
        name: "HAUNTED",
        description: "Restless Ghost quest progress",
    },
    VarpInfo {
        id: 33,
        name: "ERNESTLEVER",
        description: "Ernest the Chicken levers",
    },
    VarpInfo {
        id: 43,
        name: "COM_MODE",
        description: "Combat attack style (0..3)",
    },
    VarpInfo {
        id: 44,
        name: "COM_AMMO",
        description: "Equipped ammo count",
    },
    VarpInfo {
        id: 46,
        name: "COM_STANCE",
        description: "Combat stance / defensive mode",
    },
    VarpInfo {
        id: 60,
        name: "SHEEPHERDERQUEST",
        description: "Sheep Herder quest progress",
    },
    VarpInfo {
        id: 62,
        name: "GOBLINQUEST",
        description: "Goblin Diplomacy quest progress",
    },
    VarpInfo {
        id: 63,
        name: "RUNEMYSTERIES",
        description: "Rune Mysteries quest progress",
    },
    VarpInfo {
        id: 65,
        name: "WATERFALL_QUEST",
        description: "Waterfall Quest progress",
    },
    VarpInfo {
        id: 67,
        name: "HETTY",
        description: "Witch's Potion quest progress",
    },
    VarpInfo {
        id: 68,
        name: "BIOHAZARD",
        description: "Biohazard quest progress",
    },
    VarpInfo {
        id: 71,
        name: "HUNT",
        description: "Jungle Potion quest progress",
    },
    VarpInfo {
        id: 76,
        name: "SCORPCATCHER",
        description: "Scorpion Catcher quest progress",
    },
    VarpInfo {
        id: 77,
        name: "BARCRAWL",
        description: "Alfred Grimhand's Barcrawl",
    },
    VarpInfo {
        id: 80,
        name: "DRUIDQUEST",
        description: "Druidic Ritual quest progress",
    },
    VarpInfo {
        id: 83,
        name: "PRAYER0",
        description: "Prayer: Thick Skin",
    },
    VarpInfo {
        id: 84,
        name: "PRAYER1",
        description: "Prayer: Burst of Strength",
    },
    VarpInfo {
        id: 85,
        name: "PRAYER2",
        description: "Prayer: Clarity of Thought",
    },
    VarpInfo {
        id: 86,
        name: "PRAYER3",
        description: "Prayer: Sharp Eye",
    },
    VarpInfo {
        id: 87,
        name: "PRAYER4",
        description: "Prayer: Mystic Will",
    },
    VarpInfo {
        id: 88,
        name: "PRAYER5",
        description: "Prayer: Rock Skin",
    },
    VarpInfo {
        id: 89,
        name: "PRAYER6",
        description: "Prayer: Superhuman Strength",
    },
    VarpInfo {
        id: 90,
        name: "PRAYER7",
        description: "Prayer: Improved Reflexes",
    },
    VarpInfo {
        id: 91,
        name: "PRAYER8",
        description: "Prayer: Rapid Restore",
    },
    VarpInfo {
        id: 92,
        name: "PRAYER9",
        description: "Prayer: Rapid Heal",
    },
    VarpInfo {
        id: 93,
        name: "PRAYER10",
        description: "Prayer: Protect Item",
    },
    VarpInfo {
        id: 94,
        name: "PRAYER11",
        description: "Prayer: Hawk Eye",
    },
    VarpInfo {
        id: 95,
        name: "PRAYER12",
        description: "Prayer: Mystic Lore",
    },
    VarpInfo {
        id: 96,
        name: "PRAYER13",
        description: "Prayer: Steel Skin",
    },
    VarpInfo {
        id: 97,
        name: "PRAYER14",
        description: "Prayer: Ultimate Strength",
    },
    VarpInfo {
        id: 98,
        name: "PRAYER15",
        description: "Prayer: Incredible Reflexes",
    },
    VarpInfo {
        id: 99,
        name: "PRAYER16",
        description: "Prayer: Protect from Magic",
    },
    VarpInfo {
        id: 100,
        name: "PRAYER17",
        description: "Prayer: Protect from Missiles",
    },
    VarpInfo {
        id: 101,
        name: "QP",
        description: "Quest Points total",
    },
    VarpInfo {
        id: 102,
        name: "POISON",
        description: "Poison severity (>0: poisoned)",
    },
    VarpInfo {
        id: 107,
        name: "PRIESTSTART",
        description: "Priest in Peril quest progress",
    },
    VarpInfo {
        id: 111,
        name: "TREEQUEST",
        description: "Tree Gnome Village quest progress",
    },
    VarpInfo {
        id: 115,
        name: "BANK_TAB",
        description: "Active bank tab index",
    },
    VarpInfo {
        id: 142,
        name: "TELEBLOCK",
        description: "Teleblock timer duration",
    },
    VarpInfo {
        id: 166,
        name: "BRIGHTNESS",
        description: "Screen brightness setting (1..4)",
    },
    VarpInfo {
        id: 168,
        name: "MUSIC_VOLUME",
        description: "Music volume level (0..255)",
    },
    VarpInfo {
        id: 169,
        name: "SOUND_EFFECT_VOLUME",
        description: "Sound effect volume (0..255)",
    },
    VarpInfo {
        id: 170,
        name: "MOUSE_BUTTONS",
        description: "Mouse button mode (0 = two, 1 = one)",
    },
    VarpInfo {
        id: 171,
        name: "CHAT_EFFECTS",
        description: "Chat color and wave effects",
    },
    VarpInfo {
        id: 172,
        name: "AUTO_RETALIATE",
        description: "Auto retaliate (0 = on, 1 = off)",
    },
    VarpInfo {
        id: 173,
        name: "RUN_SLIDER",
        description: "Run toggle (0 = walk, 1 = run)",
    },
    VarpInfo {
        id: 261,
        name: "POUCH_STATUS",
        description: "Essence pouch degradation state",
    },
    VarpInfo {
        id: 286,
        name: "DUEL_PENDING",
        description: "Active duel request status",
    },
    VarpInfo {
        id: 300,
        name: "SPECIAL_ATTACK_PERCENT",
        description: "Special attack energy (0..1000)",
    },
    VarpInfo {
        id: 301,
        name: "SPECIAL_ATTACK_ENABLED",
        description: "Special attack toggled on",
    },
    VarpInfo {
        id: 302,
        name: "SPLIT_PRIVATE_CHAT",
        description: "Split private chat enabled",
    },
    VarpInfo {
        id: 304,
        name: "ACCEPT_AID",
        description: "Accept Aid toggle (0 = off, 1 = on)",
    },
    VarpInfo {
        id: 456,
        name: "DISEASE_VALUE",
        description: "Disease damage counter",
    },
    VarpInfo {
        id: 872,
        name: "AREA_SOUND_EFFECT_VOLUME",
        description: "Area ambient sound volume (0..255)",
    },
    VarpInfo {
        id: 1060,
        name: "NMZ_REWARD_POINTS",
        description: "Nightmare Zone reward points",
    },
    VarpInfo {
        id: 1427,
        name: "IN_RAID_PARTY",
        description: "Raids / CoX party recruitment status",
    },
    VarpInfo {
        id: 1780,
        name: "MEMBERSHIP_DAYS",
        description: "Remaining membership days",
    },
];

/// Curated table of prominent OSRS Varbit IDs in ascending order for binary search.
pub const VARBITS: &[VarbitInfo] = &[
    VarbitInfo {
        id: 0,
        name: "TROLL_FREED_EADGAR",
        description: "Eadgar freed during Troll Stronghold",
    },
    VarbitInfo {
        id: 10,
        name: "PYRE_LOC",
        description: "Pyre ship location status",
    },
    VarbitInfo {
        id: 24,
        name: "STAMINA_DURATION",
        description: "Stamina potion duration (ticks / 10)",
    },
    VarbitInfo {
        id: 25,
        name: "STAMINA_ACTIVE",
        description: "Stamina run depletion reduction active",
    },
    VarbitInfo {
        id: 26,
        name: "GAME_FILTER",
        description: "Game message filter (0 = all, 1 = filtered)",
    },
    VarbitInfo {
        id: 28,
        name: "CLAN_LEADER",
        description: "Clan leader rank flag",
    },
    VarbitInfo {
        id: 29,
        name: "RUNE_POUCH_TYPE_1",
        description: "Rune pouch slot 1 rune ID",
    },
    VarbitInfo {
        id: 30,
        name: "RUNE_POUCH_TYPE_2",
        description: "Rune pouch slot 2 rune ID",
    },
    VarbitInfo {
        id: 31,
        name: "RUNE_POUCH_TYPE_3",
        description: "Rune pouch slot 3 rune ID",
    },
    VarbitInfo {
        id: 32,
        name: "RUNE_POUCH_TYPE_4",
        description: "Rune pouch slot 4 rune ID",
    },
    VarbitInfo {
        id: 71,
        name: "SETTINGS_MINIMENU_SCROLLBAR",
        description: "Right-click context menu scrollbar",
    },
    VarbitInfo {
        id: 72,
        name: "MISC_APPROVAL",
        description: "Miscellania approval rating percentage",
    },
    VarbitInfo {
        id: 74,
        name: "MISC_COFFERS",
        description: "Miscellania kingdom coffers gold",
    },
    VarbitInfo {
        id: 118,
        name: "QUICK_PRAYER",
        description: "Quick prayers currently active",
    },
    VarbitInfo {
        id: 119,
        name: "PRAYER_PROTECT_MELEE",
        description: "Protect from Melee active",
    },
    VarbitInfo {
        id: 120,
        name: "PRAYER_PROTECT_MISSILES",
        description: "Protect from Missiles active",
    },
    VarbitInfo {
        id: 121,
        name: "PRAYER_PROTECT_MAGIC",
        description: "Protect from Magic active",
    },
    VarbitInfo {
        id: 122,
        name: "PRAYER_RETRIBUTION",
        description: "Retribution active",
    },
    VarbitInfo {
        id: 123,
        name: "PRAYER_REDEMPTION",
        description: "Redemption active",
    },
    VarbitInfo {
        id: 124,
        name: "PRAYER_SMITE",
        description: "Smite active",
    },
    VarbitInfo {
        id: 125,
        name: "PRAYER_PRESERVE",
        description: "Preserve active",
    },
    VarbitInfo {
        id: 126,
        name: "PRAYER_CHIVALRY",
        description: "Chivalry active",
    },
    VarbitInfo {
        id: 127,
        name: "PRAYER_PIETY",
        description: "Piety active",
    },
    VarbitInfo {
        id: 128,
        name: "PRAYER_RIGOUR",
        description: "Rigour active",
    },
    VarbitInfo {
        id: 129,
        name: "PRAYER_AUGURY",
        description: "Augury active",
    },
    VarbitInfo {
        id: 4608,
        name: "TRANSPARENT_CHATBOX",
        description: "Transparent chatbox background enabled",
    },
    VarbitInfo {
        id: 4609,
        name: "SIDE_PANEL_OPAQUE",
        description: "Opaque side panel background enabled",
    },
    VarbitInfo {
        id: 4611,
        name: "SCROLLBAR_ARROWS",
        description: "Show scrollbar arrow buttons",
    },
    VarbitInfo {
        id: 6542,
        name: "FAVOUR_ARCEUUS",
        description: "Arceuus House favour percentage",
    },
    VarbitInfo {
        id: 6543,
        name: "FAVOUR_HOSIDIUS",
        description: "Hosidius House favour percentage",
    },
    VarbitInfo {
        id: 6544,
        name: "FAVOUR_LOVAKENGJ",
        description: "Lovakengj House favour percentage",
    },
    VarbitInfo {
        id: 6545,
        name: "FAVOUR_PISCARILIUS",
        description: "Piscarilius House favour percentage",
    },
    VarbitInfo {
        id: 6546,
        name: "FAVOUR_SHAYZIEN",
        description: "Shayzien House favour percentage",
    },
    VarbitInfo {
        id: 8119,
        name: "DEATH_COFFER_VALUE",
        description: "Death's Coffer accumulated value",
    },
    VarbitInfo {
        id: 10556,
        name: "LEAGUES_TASK_NOTIFICATION",
        description: "Leagues task completion popups",
    },
];

/// Look up a VarPlayer by numerical ID.
pub fn varp_info(id: u32) -> Option<&'static VarpInfo> {
    VARPS
        .binary_search_by_key(&id, |v| v.id)
        .ok()
        .map(|idx| &VARPS[idx])
}

/// Look up a Varbit by numerical ID.
pub fn varbit_info(id: u32) -> Option<&'static VarbitInfo> {
    VARBITS
        .binary_search_by_key(&id, |v| v.id)
        .ok()
        .map(|idx| &VARBITS[idx])
}

/// Search VarPlayers by substring in name or description (case-insensitive).
pub fn search_varps(query: &str) -> Vec<&'static VarpInfo> {
    if query.is_empty() {
        return VARPS.iter().collect();
    }
    let q = query.to_lowercase();
    VARPS
        .iter()
        .filter(|v| v.name.to_lowercase().contains(&q) || v.description.to_lowercase().contains(&q))
        .collect()
}

/// Search Varbits by substring in name or description (case-insensitive).
pub fn search_varbits(query: &str) -> Vec<&'static VarbitInfo> {
    if query.is_empty() {
        return VARBITS.iter().collect();
    }
    let q = query.to_lowercase();
    VARBITS
        .iter()
        .filter(|v| v.name.to_lowercase().contains(&q) || v.description.to_lowercase().contains(&q))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varps_and_varbits_are_sorted_and_searchable() {
        for window in VARPS.windows(2) {
            assert!(
                window[0].id < window[1].id,
                "VARPS must be strictly sorted by ID"
            );
        }
        for window in VARBITS.windows(2) {
            assert!(
                window[0].id < window[1].id,
                "VARBITS must be strictly sorted by ID"
            );
        }
        assert_eq!(varp_info(43).map(|v| v.name), Some("COM_MODE"));
        assert_eq!(varbit_info(25).map(|v| v.name), Some("STAMINA_ACTIVE"));
        assert!(!search_varps("combat").is_empty());
        assert!(!search_varbits("stamina").is_empty());
    }
}
