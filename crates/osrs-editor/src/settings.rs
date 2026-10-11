//! Persistent editor settings (`editor.json`), loaded at start and saved when they change.
//!
//! Every field has a default and unknown fields are ignored, so adding settings never invalidates
//! an existing file. Tools keep their own state under [`Settings::plugins`], keyed by plugin name.

use osrs_hd::{environment::EnvironmentSettings, shadows::ShadowSettings};
use osrs_render::gpu::{Colorblind, ColorblindMode, Fog, HdParams, RenderProfile};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, io, path::PathBuf};

/// Categories on the dock's icon rail; each swaps the dock panel's contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RailTab {
    /// The selected object and the hovered tile.
    #[default]
    Inspect,
    /// Camera, world and interface options used while working.
    View,
    /// Render profile, lighting, fog and shadows.
    Graphics,
    /// Plugin sections.
    Plugins,
}

impl RailTab {
    pub const ALL: [Self; 4] = [Self::Inspect, Self::View, Self::Graphics, Self::Plugins];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Inspect => "Inspector",
            Self::View => "View",
            Self::Graphics => "Graphics",
            Self::Plugins => "Plugins",
        }
    }
}

/// Tabs inside the Inspector category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum InspectTab {
    #[default]
    Object,
    Tile,
}

/// Pages of the settings window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SettingsPage {
    #[default]
    Interface,
    CameraWorld,
    Graphics,
    HdLighting,
    HdShadowsWater,
    Controls,
    Advanced,
}

/// What Shift + right click opens in the viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ContextMenuStyle {
    /// The editor's own menu: select, focus, teleport, copy, and (later) the editing actions.
    #[default]
    Editor,
    /// The game's menu as the client draws it (`Choose Option`, the object's actions, `Examine`,
    /// `Walk here`, `Cancel`), with nothing of the editor in it.
    Game,
}

impl ContextMenuStyle {
    pub const ALL: [Self; 2] = [Self::Editor, Self::Game];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Editor => "Editor menu",
            Self::Game => "Game menu (OSRS / RuneLite)",
        }
    }

    pub const fn short_label(self) -> &'static str {
        match self {
            Self::Editor => "Editor",
            Self::Game => "Game",
        }
    }
}

/// Size of the minimap assembly: the game's own sprites at 1x are `Medium`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MinimapSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl MinimapSize {
    pub const ALL: [Self; 3] = [Self::Small, Self::Medium, Self::Large];

    /// Interface scale applied to every minimap sprite.
    pub const fn scale(self) -> f32 {
        match self {
            Self::Small => 0.75,
            Self::Medium => 1.0,
            Self::Large => 1.5,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Small => "Small",
            Self::Medium => "Medium (normal)",
            Self::Large => "Large",
        }
    }
}

/// Colourblindness correction mode (RuneLite's GPU plugin option).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ColorblindChoice {
    #[default]
    None,
    Protanope,
    Deuteranope,
    Tritanope,
}

impl ColorblindChoice {
    pub const ALL: [Self; 4] = [
        Self::None,
        Self::Protanope,
        Self::Deuteranope,
        Self::Tritanope,
    ];

    pub fn render_mode(self) -> ColorblindMode {
        match self {
            Self::None => ColorblindMode::None,
            Self::Protanope => ColorblindMode::Protanope,
            Self::Deuteranope => ColorblindMode::Deuteranope,
            Self::Tritanope => ColorblindMode::Tritanope,
        }
    }
}

/// Which renderer draws the viewport (the persisted form of `osrs_render::gpu::RenderProfile`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ProfileChoice {
    /// RuneLite GPU plugin parity.
    #[default]
    Reference,
    /// The 117 HD port (work in progress).
    Hd,
}

impl ProfileChoice {
    pub const ALL: [Self; 2] = [Self::Reference, Self::Hd];

    pub const fn render_profile(self) -> RenderProfile {
        match self {
            Self::Reference => RenderProfile::Reference,
            Self::Hd => RenderProfile::Hd,
        }
    }

    pub const fn label(self) -> &'static str {
        self.render_profile().label()
    }
}

/// Revision 1: HD lighting comes from the area environments. Exposures saved while the HD profile
/// had a placeholder sun (often raised to brighten it) would now blow the image out.
pub const HD_LIGHTING_REVISION: u32 = 1;

/// The client's default sky colour (`0xC8B8A8`): fog fades into it and the viewport clears to it.
pub const DEFAULT_SKY: [f32; 3] = [200.0 / 255.0, 184.0 / 255.0, 168.0 / 255.0];

/// Every persisted editor value, in one place. The running editor reads and writes this struct
/// directly (the UI binds sliders to its fields), so a value has exactly one home; only state
/// that lives elsewhere at runtime (camera, keymap, plugins) is copied in and out on save/load.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Index into `theme::ALL`.
    pub appearance: usize,
    pub properties_panel: bool,
    pub stats_overlay: bool,
    /// The minimap assembly (map, compass, globe) in the viewport's corner.
    pub show_minimap: bool,
    pub minimap_size: MinimapSize,
    /// Play object animations (off shows every animated object in its rest pose).
    pub animate_objects: bool,
    /// The in-game style action text next to the cursor (RuneLite's mouse tooltips).
    pub mouse_tooltips: bool,
    /// Outline the tile under the cursor.
    pub hover_tile: bool,
    /// Reference (RuneLite parity, default) or HD (117 HD port).
    pub render_profile: ProfileChoice,
    /// Bumped when the meaning of the HD lighting settings changed; older files are migrated in
    /// [`Settings::sanitized`].
    #[serde(default)]
    pub hd_lighting_revision: u32,
    /// HD: the plugin's Brightness, in percent (`100` = unchanged), applied as a gamma.
    pub hd_brightness: f32,
    /// HD resolve pass: linear exposure.
    pub hd_exposure: f32,
    /// HD resolve pass: phase-0 placeholder vignette strength.
    pub hd_vignette: f32,
    /// HD: normal mapping on materials that have a normal map.
    pub hd_normal_mapping: bool,
    /// HD: caustics on the sea floor and under the surface.
    pub hd_underwater_caustics: bool,
    /// HD: parallax occlusion mapping on materials that have displacement maps.
    pub hd_parallax_mapping: bool,
    /// HD: per-area environments (lighting, fog, sky and water colour; Properties > Shading).
    pub hd_environment: EnvironmentSettings,
    /// HD: the shadow map (Properties > Shading > Shadows).
    pub hd_shadows: ShadowSettings,
    pub brightness: f32,
    pub remove_color_banding: bool,
    pub bright_textures: bool,
    pub fog_depth_tiles: u32,
    pub draw_distance_tiles: u32,
    pub colorblind_mode: ColorblindChoice,
    pub colorblind_intensity: f32,
    pub sky_color: [f32; 3],
    pub smooth_terrain: bool,
    pub load_radius: i32,
    pub fov_degrees: f32,
    pub fly_speed: f32,
    pub orbit_camera: bool,
    /// Side panel on the right (otherwise left).
    pub dock_right: bool,
    /// Dock category and, inside the Inspector, its tab.
    pub rail_tab: RailTab,
    pub inspect_tab: InspectTab,
    /// The active editing tool and its drawer.
    pub tool: crate::paint::Tool,
    pub drawer_open: bool,
    pub drawer_tab: crate::paint::DrawerTab,
    pub brush: crate::paint::Brush,
    /// The settings window's current page.
    pub settings_page: SettingsPage,
    /// Shift + right click: the editor's menu or the game's.
    pub context_menu: ContextMenuStyle,
    /// Keyboard shortcut overrides: action id -> `"Shift+G"`.
    pub keymap: BTreeMap<String, String>,
    /// Per-plugin state (see `EditorPlugin::save_settings`).
    pub plugins: BTreeMap<String, serde_json::Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            appearance: 0,
            properties_panel: true,
            stats_overlay: true,
            show_minimap: true,
            minimap_size: MinimapSize::Medium,
            animate_objects: true,
            mouse_tooltips: true,
            hover_tile: true,
            render_profile: ProfileChoice::Reference,
            hd_lighting_revision: HD_LIGHTING_REVISION,
            hd_brightness: 100.0,
            hd_exposure: HdParams::default().exposure,
            // The placeholder vignette of the resolve pass is off unless asked for.
            hd_vignette: 0.0,
            hd_normal_mapping: true,
            hd_underwater_caustics: true,
            hd_parallax_mapping: true,
            hd_environment: EnvironmentSettings::default(),
            hd_shadows: ShadowSettings::default(),
            brightness: 0.8,
            remove_color_banding: true,
            bright_textures: false,
            fog_depth_tiles: 0,
            draw_distance_tiles: 50,
            colorblind_mode: ColorblindChoice::None,
            colorblind_intensity: 100.0,
            sky_color: DEFAULT_SKY,
            smooth_terrain: false,
            load_radius: 1,
            fov_degrees: 60.0,
            fly_speed: 900.0,
            orbit_camera: false,
            dock_right: true,
            rail_tab: RailTab::Inspect,
            inspect_tab: InspectTab::Object,
            tool: crate::paint::Tool::Select,
            drawer_open: true,
            drawer_tab: crate::paint::DrawerTab::Overlay,
            brush: crate::paint::Brush::default(),
            settings_page: SettingsPage::Interface,
            context_menu: ContextMenuStyle::Editor,
            keymap: BTreeMap::new(),
            plugins: BTreeMap::new(),
        }
    }
}

/// Where the settings file lives: `$RUSTOSRS_SETTINGS`, else the platform's config directory.
pub fn path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("RUSTOSRS_SETTINGS") {
        return Some(PathBuf::from(path));
    }
    let home = PathBuf::from(std::env::var_os("HOME")?);
    let directory = if cfg!(target_os = "macos") {
        home.join("Library/Application Support/RustOSRS")
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"))
            .join("rustosrs")
    };
    Some(directory.join("editor.json"))
}

impl Settings {
    /// RuneLite-style fog settings for the renderer.
    pub fn fog(&self) -> Fog {
        Fog {
            depth_tiles: self.fog_depth_tiles,
            draw_distance_tiles: self.draw_distance_tiles,
        }
    }

    pub fn hd_params(&self) -> HdParams {
        HdParams {
            exposure: self.hd_exposure,
            placeholder_vignette: self.hd_vignette,
        }
    }

    pub fn colorblind(&self) -> Colorblind {
        Colorblind {
            mode: self.colorblind_mode.render_mode(),
            intensity: self.colorblind_intensity,
        }
    }

    /// Clamp every value to what the UI allows (a hand-edited file can hold anything).
    pub fn sanitized(mut self) -> Self {
        self.appearance = self.appearance.min(crate::theme::ALL.len() - 1);
        // Pure black was the old default and made fog black; it reads as "unset".
        if self.sky_color == [0.0, 0.0, 0.0] {
            self.sky_color = DEFAULT_SKY;
        }
        if self.hd_lighting_revision < HD_LIGHTING_REVISION {
            self.hd_exposure = HdParams::default().exposure;
            self.hd_lighting_revision = HD_LIGHTING_REVISION;
        }
        self.brush.radius = self.brush.radius.min(16);
        self.hd_exposure = self.hd_exposure.clamp(0.25, 4.0);
        self.hd_vignette = self.hd_vignette.clamp(0.0, 1.0);
        self.hd_brightness = self.hd_brightness.clamp(25.0, 400.0);
        self.hd_environment.static_fog_depth =
            self.hd_environment.static_fog_depth.clamp(0.0, 100.0);
        self.hd_shadows.distance_tiles = self.hd_shadows.distance_tiles.clamp(4, 184);
        self.brightness = self.brightness.clamp(0.5, 1.0);
        self.fog_depth_tiles = self.fog_depth_tiles.min(100);
        self.draw_distance_tiles = self.draw_distance_tiles.clamp(8, 184);
        self.colorblind_intensity = self.colorblind_intensity.clamp(0.0, 100.0);
        self.load_radius = self.load_radius.clamp(0, 4);
        self.fov_degrees = self.fov_degrees.clamp(30.0, 100.0);
        self.fly_speed = self.fly_speed.clamp(50.0, 20_000.0);
        self
    }

    /// Settings from `path`; defaults when the file is missing or unreadable.
    pub fn load(path: &std::path::Path) -> Self {
        fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Write the settings atomically (temporary file, then rename).
    pub fn save(&self, path: &std::path::Path) -> io::Result<()> {
        if let Some(directory) = path.parent() {
            fs::create_dir_all(directory)?;
        }
        let temporary = path.with_extension("json.tmp");
        let text = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        fs::write(&temporary, text)?;
        fs::rename(&temporary, path)
    }
}

#[cfg(test)]
mod tests {
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
    fn hd_environment_settings_persist_and_default_like_the_plugin() -> Result<(), serde_json::Error>
    {
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
    fn an_exposure_saved_with_the_placeholder_lighting_is_reset_once()
    -> Result<(), serde_json::Error> {
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
        assert_eq!(settings.rail_tab, RailTab::Graphics);
        assert_eq!(settings.tool, crate::paint::Tool::Paint);
        Ok(())
    }

    #[test]
    fn save_and_load_round_trip() -> io::Result<()> {
        let directory =
            std::env::temp_dir().join(format!("rustosrs-settings-{}", std::process::id()));
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
}
