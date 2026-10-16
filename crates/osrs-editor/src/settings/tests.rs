use super::*;

#[test]
fn missing_fields_fall_back_to_defaults() -> Result<(), serde_json::Error> {
    let settings: Settings = serde_json::from_str(r#"{ "brightness": 0.6 }"#)?;
    assert_eq!(settings.brightness, 0.6);
    assert_eq!(settings.draw_distance_tiles, 50);
    assert!(settings.properties_panel);
    assert!(settings.hd_parallax_mapping);
    Ok(())
}

#[test]
fn the_reference_profile_is_the_default_and_persists() -> Result<(), serde_json::Error> {
    assert_eq!(Settings::default().render_profile, ProfileChoice::Reference);
    let hd: Settings = serde_json::from_str(
        r#"{ "render_profile": "Hd", "hd_exposure": 9.0, "hd_lighting_revision": 1 }"#,
    )?;
    assert_eq!(hd.render_profile.render_profile(), RenderProfile::Hd);
    assert_eq!(hd.sanitized().hd_exposure, 4.0);
    // A file written before the switch existed keeps the Reference profile.
    let old: Settings = serde_json::from_str(r#"{ "brightness": 0.7 }"#)?;
    assert_eq!(old.render_profile, ProfileChoice::Reference);
    Ok(())
}

#[test]
fn hd_environment_settings_persist_and_default_like_the_plugin() -> Result<(), serde_json::Error> {
    use osrs_hd::environment::{DefaultSky, FogDepthMode};
    let defaults = Settings::default().hd_environment;
    assert!(defaults.atmospheric_lighting && defaults.ground_fog);
    assert_eq!(defaults.default_sky, DefaultSky::Default);
    // Partial environment tables keep the other defaults.
    let settings: Settings = serde_json::from_str(
        r#"{ "hd_environment": { "fog_depth_mode": "Static", "static_fog_depth": 250.0 } }"#,
    )?;
    assert_eq!(settings.hd_environment.fog_depth_mode, FogDepthMode::Static);
    assert!(settings.hd_environment.atmospheric_lighting);
    // Out-of-range values are clamped.
    assert_eq!(settings.sanitized().hd_environment.static_fog_depth, 100.0);
    // Shadow options persist with their enum names and fall back to the plugin's defaults.
    let shadows: Settings = serde_json::from_str(
        r#"{ "hd_shadows": { "mode": "Fast", "filtering": "Pcss", "distance_tiles": 900 } }"#,
    )?;
    assert_eq!(shadows.hd_shadows.mode, osrs_hd::shadows::ShadowMode::Fast);
    assert!(shadows.hd_shadows.terrain_shadows && shadows.hd_shadows.transparency);
    assert_eq!(shadows.sanitized().hd_shadows.distance_tiles, 184);
    // The host sky colour is runtime state and is not persisted.
    let text = serde_json::to_string(&Settings::default())?;
    assert!(!text.contains("host_sky"));
    Ok(())
}

#[test]
fn an_exposure_saved_with_the_placeholder_lighting_is_reset_once() -> Result<(), serde_json::Error>
{
    let old: Settings = serde_json::from_str(r#"{ "hd_exposure": 1.67 }"#)?;
    assert_eq!(old.hd_lighting_revision, 0);
    let migrated = old.sanitized();
    assert_eq!(migrated.hd_exposure, 1.0);
    // A deliberate choice made afterwards survives later loads.
    let kept = Settings {
        hd_exposure: 1.5,
        ..migrated
    };
    assert_eq!(kept.sanitized().hd_exposure, 1.5);
    Ok(())
}

#[test]
fn the_context_menu_settings_page_and_workspace_persist() -> Result<(), serde_json::Error> {
    assert_eq!(Settings::default().context_menu, ContextMenuStyle::Editor);
    // Files from before the rail still load: the retired `side_tab` key is ignored.
    let settings: Settings = serde_json::from_str(
        r#"{ "context_menu": "Game", "settings_page": "Controls", "side_tab": "Shading",
             "rail_tab": "Graphics", "tool": "Paint" }"#,
    )?;
    assert_eq!(settings.context_menu, ContextMenuStyle::Game);
    assert_eq!(settings.settings_page, SettingsPage::Controls);
    // `sanitized` folds the retired Graphics button into View's Graphics tab.
    let settings = settings.sanitized();
    assert_eq!(settings.rail_tab, RailTab::View);
    assert_eq!(settings.view_tab, ViewTab::Graphics);
    assert_eq!(settings.tool, crate::paint::Tool::Paint);
    // Plugins left the rail for a menu-bar window; a file that names it opens on the Inspector.
    let old: Settings = serde_json::from_str(r#"{ "rail_tab": "Plugins" }"#)?;
    assert_eq!(old.rail_tab, RailTab::Inspect);
    Ok(())
}

#[test]
fn save_and_load_round_trip() -> io::Result<()> {
    let directory = std::env::temp_dir().join(format!("rustosrs-settings-{}", std::process::id()));
    let path = directory.join("editor.json");
    let mut settings = Settings {
        smooth_terrain: true,
        fly_speed: 1234.0,
        ..Settings::default()
    };
    settings
        .plugins
        .insert("demo".to_owned(), serde_json::json!({ "n": 1 }));
    settings.save(&path)?;
    assert_eq!(Settings::load(&path), settings);
    let _ = fs::remove_dir_all(directory);
    Ok(())
}

#[test]
fn unreadable_file_gives_defaults() {
    assert_eq!(
        Settings::load(std::path::Path::new("/nonexistent/rustosrs.json")),
        Settings::default()
    );
}

#[test]
fn the_sound_and_player_rail_tabs_and_the_mock_player_persist() -> Result<(), serde_json::Error> {
    assert!(RailTab::ALL.contains(&RailTab::Sound) && RailTab::ALL.contains(&RailTab::Player));
    let labels: std::collections::BTreeSet<_> = RailTab::ALL.map(RailTab::label).into();
    assert_eq!(
        labels.len(),
        RailTab::ALL.len(),
        "every rail button has its own name"
    );
    // A file from before these panels has the default player and still loads.
    let old: Settings = serde_json::from_str(r#"{ "rail_tab": "View" }"#)?;
    assert_eq!(old.player, crate::player::PlayerSandbox::default());
    let mut settings = Settings {
        rail_tab: RailTab::Sound,
        ..Settings::default()
    };
    settings.player.name = "Ada".to_owned();
    settings.player.varps.insert(281, 7);
    let back: Settings = serde_json::from_str(&serde_json::to_string(&settings)?)?;
    assert_eq!(back.rail_tab, RailTab::Sound);
    assert_eq!(back.player.name, "Ada");
    assert_eq!(back.player.varps.get(&281), Some(&7));
    // `sanitized` leaves the new tabs alone.
    assert_eq!(back.sanitized().rail_tab, RailTab::Sound);
    Ok(())
}
