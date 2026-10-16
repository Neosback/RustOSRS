use super::*;
use crate::{
    definitions::DefinitionIdentity,
    provenance::{CacheFingerprint, ProfileDigest, TargetProvenance},
};

fn provenance() -> TargetProvenance {
    TargetProvenance::new(
        "test",
        ProfileDigest::from_lower_hex(&"b".repeat(64)).unwrap_or_else(|_| unreachable!()),
        CacheFingerprint::from_lower_hex(&"a".repeat(64)).unwrap_or_else(|_| unreachable!()),
        1,
    )
    .unwrap_or_else(|_| unreachable!())
}

fn kit(id: u32, part: u8, selectable: bool) -> KitDefinition {
    KitDefinition {
        identity: DefinitionIdentity::new(KitId::new(id), provenance()),
        body_part: Some(part),
        models: vec![ModelId::new(id + 1000)],
        non_selectable: !selectable,
        recolors: Vec::new(),
        retextures: Vec::new(),
    }
}

#[test]
fn kits_fill_the_slots_the_client_assigns_them() {
    let kits = [Some(KitId::new(1)); KIT_PARTS];
    let appearance = PlayerAppearance::compose(Gender::Male, kits, [0; 5], &[]);
    // hair 8, jaw 11, torso 4, arms 6, hands 9, legs 7, feet 10
    for slot in [8, 11, 4, 6, 9, 7, 10] {
        assert_eq!(
            appearance.slots[slot],
            SlotContent::Kit(KitId::new(1)),
            "slot {slot}"
        );
    }
    for slot in [0, 1, 2, 3, 5] {
        assert_eq!(appearance.slots[slot], SlotContent::Empty, "slot {slot}");
    }
}

#[test]
fn a_worn_item_replaces_its_slot_and_hides_the_slots_it_covers() {
    let kits = [Some(KitId::new(1)); KIT_PARTS];
    let platebody = WornItem {
        slot: 4,
        item: ItemId::new(1119),
        hides: [Some(6), None],
    };
    let full_helm = WornItem {
        slot: 0,
        item: ItemId::new(1163),
        hides: [Some(8), Some(11)],
    };
    let appearance = PlayerAppearance::compose(Gender::Male, kits, [0; 5], &[platebody, full_helm]);
    assert_eq!(appearance.slots[4], SlotContent::Item(ItemId::new(1119)));
    assert_eq!(appearance.slots[0], SlotContent::Item(ItemId::new(1163)));
    assert_eq!(
        appearance.slots[6],
        SlotContent::Empty,
        "arms hidden under the body"
    );
    assert_eq!(
        appearance.slots[8],
        SlotContent::Empty,
        "hair hidden under the helm"
    );
    assert_eq!(
        appearance.slots[11],
        SlotContent::Empty,
        "jaw hidden under the helm"
    );
    assert_eq!(
        appearance.slots[7],
        SlotContent::Kit(KitId::new(1)),
        "legs untouched"
    );
}

#[test]
fn an_item_never_hides_another_worn_item() {
    let a = WornItem {
        slot: 4,
        item: ItemId::new(1),
        hides: [Some(7), None],
    };
    let b = WornItem {
        slot: 7,
        item: ItemId::new(2),
        hides: [None, None],
    };
    let appearance = PlayerAppearance::compose(Gender::Male, [None; KIT_PARTS], [0; 5], &[a, b]);
    assert_eq!(appearance.slots[7], SlotContent::Item(ItemId::new(2)));
}

#[test]
fn default_kits_pick_the_first_selectable_kit_per_part_and_gender() {
    let defs = [
        kit(0, 0, false), // not selectable
        kit(1, 0, true),
        kit(2, 0, true),
        kit(3, 2, true),
        kit(4, 7, true), // female hair
        kit(5, 9, true), // female torso
    ];
    let male = default_kits(Gender::Male, &defs);
    assert_eq!(male[0], Some(KitId::new(1)));
    assert_eq!(male[2], Some(KitId::new(3)));
    assert_eq!(male[1], None, "no male jaw defined");
    let female = default_kits(Gender::Female, &defs);
    assert_eq!(female[0], Some(KitId::new(4)));
    assert_eq!(female[2], Some(KitId::new(5)));
}

#[test]
fn body_colours_follow_the_clients_palettes() {
    // Choice 0 maps each slot's base colour to itself or its first palette entry.
    let zero = body_recolors([0; 5]);
    assert_eq!(
        zero[0],
        RecolorPair {
            from: 6798,
            to: 6798
        }
    );
    // Hair choice 1 asks for 107 and, through the second table, 10304 -> 115 (bits of -10304).
    let hair = body_recolors([1, 0, 0, 0, 0]);
    assert_eq!(
        hair[0],
        RecolorPair {
            from: 6798,
            to: 107
        }
    );
    assert_eq!(
        hair[1],
        RecolorPair {
            from: (-10304i16) as u16,
            to: 115
        }
    );
    // An out-of-range choice adds nothing for that slot.
    let none = body_recolors([200; 5]);
    assert!(none.is_empty());
    // Feet and skin have no second table.
    // Slots 0 and 1 give two pairs each, slots 2, 3 and 4 one each.
    assert_eq!(body_recolors([0, 0, 0, 1, 1]).len(), 2 + 2 + 1 + 1 + 1);
}
