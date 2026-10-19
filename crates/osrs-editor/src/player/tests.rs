use super::*;

#[test]
fn a_new_player_is_the_level_three_start() {
    let player = PlayerSandbox::default();
    assert_eq!(player.combat_level(), 3);
    assert_eq!(player.total_level(), 22 + 10);
    assert_eq!(player.hitpoints, 10);
    assert_eq!(SKILLS.len(), player.levels.len());
}

#[test]
fn combat_level_matches_known_values() {
    let mut player = PlayerSandbox::default();
    player.max_levels();
    assert_eq!(player.combat_level(), 126);
    // 99 attack/strength/defence/hitpoints, everything else 1.
    let mut melee = PlayerSandbox::default();
    for skill in [ATTACK, STRENGTH, DEFENCE, HITPOINTS] {
        melee.set_level(skill, 99);
    }
    assert_eq!(melee.combat_level(), 113);
    // A pure ranger: ranged dominates.
    let mut ranger = PlayerSandbox::default();
    ranger.set_level(RANGED, 99);
    ranger.set_level(HITPOINTS, 99);
    assert_eq!(ranger.combat_level(), 73);
}

#[test]
fn levels_clamp_and_pull_current_points_down() {
    let mut player = PlayerSandbox::default();
    player.max_levels();
    player.set_level(HITPOINTS, 0);
    assert_eq!(player.levels[HITPOINTS], 10, "hitpoints never below 10");
    assert_eq!(player.hitpoints, 10, "current hitpoints follow the level");
    player.set_level(PRAYER, 200);
    assert_eq!(player.levels[PRAYER], MAX_LEVEL);
    player.set_level(PRAYER, 5);
    assert_eq!(player.prayer, 5);
    player.set_level(99, 50); // out of range: ignored
    assert_eq!(player.levels.len(), 23);
}

#[test]
fn wearing_and_removing_items_by_slot() {
    let mut player = PlayerSandbox::default();
    player.wear(3, Some(4151));
    player.wear(0, Some(1155));
    assert_eq!(player.equipment.get(&3), Some(&4151));
    player.wear(3, None);
    assert_eq!(player.equipment.keys().copied().collect::<Vec<_>>(), [0]);
}

#[test]
fn the_player_round_trips_through_settings_json() {
    let mut player = PlayerSandbox {
        name: "Test".to_owned(),
        stand: Some(Stand {
            x: 3222,
            y: 3218,
            plane: 0,
        }),
        ..PlayerSandbox::default()
    };
    player.varps.insert(281, 1000);
    player.varbits.insert(4895, 3);
    player.varcs.insert(5, -1);
    player.wear(3, Some(4151));
    player.kits[2] = Some(18);
    let json = serde_json::to_string(&player).unwrap_or_default();
    let back: Result<PlayerSandbox, _> = serde_json::from_str(&json);
    assert_eq!(back.ok(), Some(player));
    // Old or partial settings fill in defaults.
    let partial: Result<PlayerSandbox, _> = serde_json::from_str(r#"{"name":"A"}"#);
    assert_eq!(partial.as_ref().map(|p| p.run_energy).ok(), Some(100));
    assert_eq!(partial.map(|p| p.show_overlay).ok(), Some(true));
}
