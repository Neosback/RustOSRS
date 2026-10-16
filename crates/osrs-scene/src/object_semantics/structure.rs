//! What a definition says about its own role, without any name.
use osrs_core::definitions::{ObjectDefinition, ObjectModels};
use std::collections::BTreeSet;

/// The scene slot a loc type fills (the client's `LocModelType` families).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Slot {
    /// Straight boundary wall along one tile edge (type 0).
    Wall,
    /// L-shaped boundary covering two edges (type 2).
    WallCorner,
    /// Corner post at a tile vertex (types 1 and 3).
    Pillar,
    /// Attached to a wall: signs, torches, banners (types 4 to 8).
    WallDecoration,
    /// Diagonal boundary (type 9).
    WallDiagonal,
    /// Free-standing object (types 10 and 11).
    Prop,
    /// Roof piece (types 12 to 21).
    Roof,
    /// Lies on the ground (type 22).
    FloorDecoration,
}

pub const fn slot_of(loc_type: u8) -> Option<Slot> {
    Some(match loc_type {
        0 => Slot::Wall,
        1 | 3 => Slot::Pillar,
        2 => Slot::WallCorner,
        4..=8 => Slot::WallDecoration,
        9 => Slot::WallDiagonal,
        10 | 11 => Slot::Prop,
        12..=21 => Slot::Roof,
        22 => Slot::FloorDecoration,
        _ => return None,
    })
}

/// Facts read directly from a definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Structure {
    pub slots: BTreeSet<Slot>,
    pub size: (u16, u16),
    /// Menu verbs, lowercased.
    pub verbs: Vec<String>,
    pub animated: bool,
    /// The definition changes into other objects (open and closed doors, growth stages).
    pub morphs: bool,
    pub interact_type: u8,
}

pub fn structure_of(definition: &ObjectDefinition) -> Structure {
    let slots = match &definition.models {
        None => BTreeSet::new(),
        Some(ObjectModels::Untyped(_)) => BTreeSet::from([Slot::Prop]),
        Some(ObjectModels::Typed(models)) => models
            .iter()
            .filter_map(|model| slot_of(model.loc_type.get()))
            .collect(),
    };
    Structure {
        slots,
        size: (definition.size_x, definition.size_y),
        verbs: definition
            .actions
            .iter()
            .flatten()
            .map(|verb| verb.to_ascii_lowercase())
            .collect(),
        animated: definition.animation.is_some(),
        morphs: definition.morphs.is_some(),
        interact_type: definition.placement.interact_type,
    }
}

impl Structure {
    /// A verb that only a passage has.
    pub fn door_like(&self) -> bool {
        self.verbs.iter().any(|verb| {
            matches!(
                verb.as_str(),
                "open" | "close" | "unlock" | "pick-lock" | "lock"
            )
        })
    }

    pub fn is_boundary(&self) -> bool {
        self.slots.iter().any(|s| {
            matches!(
                s,
                Slot::Wall | Slot::WallCorner | Slot::Pillar | Slot::WallDiagonal
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_loc_type_maps_to_one_slot_and_unknown_types_to_none() {
        let expected = [
            (0, Slot::Wall),
            (1, Slot::Pillar),
            (2, Slot::WallCorner),
            (3, Slot::Pillar),
            (4, Slot::WallDecoration),
            (8, Slot::WallDecoration),
            (9, Slot::WallDiagonal),
            (10, Slot::Prop),
            (11, Slot::Prop),
            (12, Slot::Roof),
            (21, Slot::Roof),
            (22, Slot::FloorDecoration),
        ];
        for (kind, slot) in expected {
            assert_eq!(slot_of(kind), Some(slot), "type {kind}");
        }
        assert_eq!(slot_of(23), None);
    }

    #[test]
    fn door_like_needs_a_passage_verb() {
        let mut s = Structure {
            slots: BTreeSet::from([Slot::Wall]),
            size: (1, 1),
            verbs: vec!["examine".into()],
            animated: false,
            morphs: false,
            interact_type: 1,
        };
        assert!(!s.door_like());
        s.verbs.push("open".into());
        assert!(s.door_like() && s.is_boundary());
    }
}
