use osrs_hd::{
    HdData, IdRef,
    materials::MaterialSet,
    tiles::{Areas, GroundMaterials, TileOverrides, WaterTypes},
};
use std::path::PathBuf;

fn assets_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/117hd")
}

fn build_overrides(data: &HdData) -> (Areas, TileOverrides) {
    let materials = MaterialSet::default();
    let grounds = GroundMaterials::new(data, &materials);
    let water = WaterTypes::new(data);
    let areas = Areas::new(data);
    let overrides = TileOverrides::new(data, &grounds, &water, &|id| match id {
        IdRef::Number(number) => Some(*number),
        IdRef::Name(_) => None,
    });
    (areas, overrides)
}

#[test]
fn toa_loot_chest_platform_keeps_the_first_nested_area_override() {
    let data = HdData::load(assets_dir()).expect("load checked-in 117HD data");
    let (areas, overrides) = build_overrides(&data);

    // The vendored RLHD data defines TOA_LOOT_ROOM_CHEST_PLATFORM as
    // [3678, 5138]..=[3682, 5143], nested inside the broader
    // TOA_LOOT_ROOM [3648, 5120]..=[3711, 5183]. The chest-platform override
    // appears first in tile_overrides.json and must win, matching upstream's
    // first-match behavior for area-only overrides.
    let chest_tile = (3680, 5140, 0);
    assert!(areas.contains("TOA_LOOT_ROOM_CHEST_PLATFORM", chest_tile.0, chest_tile.1, chest_tile.2));
    assert!(areas.contains("TOA_LOOT_ROOM", chest_tile.0, chest_tile.1, chest_tile.2));

    let chest = overrides.lookup(&areas, chest_tile, &[]);
    assert_eq!(
        overrides.get(chest.index).name,
        "TOA_DISABLE_BLENDING_LOOT_ROOM_CHEST_PLATFORM"
    );
    assert!(!chest.queried_as_overlay);

    // A tile elsewhere in the same room still resolves to the broader rule.
    let room_tile = (3650, 5160, 0);
    assert!(!areas.contains(
        "TOA_LOOT_ROOM_CHEST_PLATFORM",
        room_tile.0,
        room_tile.1,
        room_tile.2
    ));
    assert!(areas.contains("TOA_LOOT_ROOM", room_tile.0, room_tile.1, room_tile.2));

    let room = overrides.lookup(&areas, room_tile, &[]);
    assert_eq!(overrides.get(room.index).name, "TOA_DISABLE_BLENDING_LOOT_ROOM");
    assert!(!room.queried_as_overlay);
}
