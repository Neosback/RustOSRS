//! A mock player to play with: stats, status, look, worn items and the three kinds of game
//! variables (varps, varbits, varcs). It is plain data with no authority over the map, the
//! session or rendering; it exists so tools and, later, real player rendering and playtesting
//! have a player whose state can be set by hand. Persisted with the editor's settings.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The skills in the order the game lists them.
pub const SKILLS: [&str; 23] = [
    "Attack",
    "Defence",
    "Strength",
    "Hitpoints",
    "Ranged",
    "Prayer",
    "Magic",
    "Cooking",
    "Woodcutting",
    "Fletching",
    "Fishing",
    "Firemaking",
    "Crafting",
    "Smithing",
    "Mining",
    "Herblore",
    "Agility",
    "Thieving",
    "Slayer",
    "Farming",
    "Runecraft",
    "Hunter",
    "Construction",
];
const ATTACK: usize = 0;
const DEFENCE: usize = 1;
const STRENGTH: usize = 2;
const HITPOINTS: usize = 3;
const RANGED: usize = 4;
const PRAYER: usize = 5;
const MAGIC: usize = 6;

/// Equipment slots by the game's slot numbers.
pub const EQUIPMENT_SLOTS: [(u8, &str); 11] = [
    (0, "Head"),
    (1, "Cape"),
    (2, "Neck"),
    (3, "Weapon"),
    (4, "Body"),
    (5, "Shield"),
    (7, "Legs"),
    (9, "Hands"),
    (10, "Feet"),
    (12, "Ring"),
    (13, "Ammo"),
];

/// Appearance kit slots (head, jaw, torso, arms, hands, legs, feet).
pub const KIT_SLOTS: [&str; 7] = ["Head", "Jaw", "Torso", "Arms", "Hands", "Legs", "Feet"];
/// Colour slots (hair, torso, legs, feet, skin).
pub const COLOUR_SLOTS: [&str; 5] = ["Hair", "Torso", "Legs", "Feet", "Skin"];

pub const MIN_LEVEL: u8 = 1;
pub const MAX_LEVEL: u8 = 99;

/// Where the mock player stands, in world tiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stand {
    pub x: i32,
    pub y: i32,
    pub plane: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlayerSandbox {
    pub name: String,
    pub female: bool,
    /// Base level of each skill, in [`SKILLS`] order.
    pub levels: [u8; 23],
    /// Current hitpoints and prayer points (the boosted or drained value, not the level).
    pub hitpoints: u16,
    pub prayer: u16,
    /// Run energy in percent.
    pub run_energy: u8,
    pub running: bool,
    pub weight_kg: i16,
    /// Standing tile; `None` leaves the player out of the world.
    pub stand: Option<Stand>,
    /// Draw the player's model on its tile (off: only the marker).
    pub show_model: bool,
    /// Identity-kit id per [`KIT_SLOTS`] entry (`None` for none).
    pub kits: [Option<u16>; 7],
    pub colours: [u8; 5],
    /// Worn item id by game slot number.
    pub equipment: BTreeMap<u8, u32>,
    pub varps: BTreeMap<u32, i32>,
    pub varbits: BTreeMap<u32, i32>,
    pub varcs: BTreeMap<u32, i32>,
}

impl Default for PlayerSandbox {
    fn default() -> Self {
        let mut levels = [MIN_LEVEL; 23];
        levels[HITPOINTS] = 10;
        Self {
            name: "Player".to_owned(),
            female: false,
            levels,
            hitpoints: 10,
            prayer: 1,
            run_energy: 100,
            running: false,
            weight_kg: 0,
            stand: None,
            show_model: true,
            kits: [None; 7],
            colours: [0; 5],
            equipment: BTreeMap::new(),
            varps: BTreeMap::new(),
            varbits: BTreeMap::new(),
            varcs: BTreeMap::new(),
        }
    }
}

impl PlayerSandbox {
    /// What the player looks like, for model assembly: kits and colours as set, worn items by
    /// body slot (item 0 means nothing worn).
    pub fn look(&self) -> osrs_core::player::PlayerLook {
        use osrs_core::{
            ids::{ItemId, KitId},
            player::{Gender, PlayerLook},
        };
        PlayerLook {
            gender: if self.female {
                Gender::Female
            } else {
                Gender::Male
            },
            kits: self.kits.map(|kit| kit.map(|id| KitId::new(u32::from(id)))),
            colours: self.colours,
            worn: self
                .equipment
                .iter()
                .filter(|(_, item)| **item != 0)
                .map(|(slot, item)| (*slot, ItemId::new(*item)))
                .collect(),
        }
    }

    /// Set a skill's level (clamped; hitpoints never below 10) and keep hitpoints and prayer
    /// from exceeding their levels.
    pub fn set_level(&mut self, skill: usize, level: u8) {
        let Some(slot) = self.levels.get_mut(skill) else {
            return;
        };
        let floor = if skill == HITPOINTS { 10 } else { MIN_LEVEL };
        *slot = level.clamp(floor, MAX_LEVEL);
        self.hitpoints = self.hitpoints.min(u16::from(self.levels[HITPOINTS]));
        self.prayer = self.prayer.min(u16::from(self.levels[PRAYER]));
    }

    pub fn max_levels(&mut self) {
        self.levels = [MAX_LEVEL; 23];
        self.hitpoints = u16::from(MAX_LEVEL);
        self.prayer = u16::from(MAX_LEVEL);
    }

    /// The combat level the game would show (floor of the sum of the base and the best style).
    pub fn combat_level(&self) -> u8 {
        let level = |skill: usize| f64::from(self.levels[skill]);
        let base = 0.25 * (level(DEFENCE) + level(HITPOINTS) + (level(PRAYER) / 2.0).floor());
        let melee = 0.325 * (level(ATTACK) + level(STRENGTH));
        let ranged = 0.325 * (1.5 * level(RANGED)).floor();
        let magic = 0.325 * (1.5 * level(MAGIC)).floor();
        (base + melee.max(ranged).max(magic)).floor() as u8
    }

    /// Total level.
    pub fn total_level(&self) -> u32 {
        self.levels.iter().map(|&l| u32::from(l)).sum()
    }

    pub fn wear(&mut self, slot: u8, item: Option<u32>) {
        match item {
            Some(item) => {
                self.equipment.insert(slot, item);
            }
            None => {
                self.equipment.remove(&slot);
            }
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests;
