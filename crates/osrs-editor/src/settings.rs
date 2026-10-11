//! Persistent editor settings (`editor.json`), loaded at start and saved when they change.
//!
//! Every field has a default and unknown fields are ignored, so adding settings never invalidates
//! an existing file. Tools keep their own state under [`Settings::plugins`], keyed by plugin name.

use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, io, path::PathBuf};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: usize,
    pub properties_panel: bool,
    pub stats_overlay: bool,
    pub brightness: f32,
    pub remove_color_banding: bool,
    pub bright_textures: bool,
    pub fog_depth_tiles: u32,
    pub draw_distance_tiles: u32,
    /// 0 off, 1 protanope, 2 deuteranope, 3 tritanope.
    pub colorblind_mode: u8,
    pub colorblind_intensity: f32,
    pub sky_color: [f32; 3],
    pub smooth_terrain: bool,
    pub grid_tiles: bool,
    pub grid_zones: bool,
    pub grid_regions: bool,
    pub load_radius: i32,
    pub fov_degrees: f32,
    pub fly_speed: f32,
    /// Per-plugin state (see `EditorPlugin::save_settings`).
    pub plugins: BTreeMap<String, serde_json::Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: 3,
            properties_panel: true,
            stats_overlay: true,
            brightness: 0.8,
            remove_color_banding: true,
            bright_textures: false,
            fog_depth_tiles: 0,
            draw_distance_tiles: 50,
            colorblind_mode: 0,
            colorblind_intensity: 100.0,
            sky_color: [0.0, 0.0, 0.0],
            smooth_terrain: false,
            grid_tiles: false,
            grid_zones: false,
            grid_regions: false,
            load_radius: 1,
            fov_degrees: 60.0,
            fly_speed: 900.0,
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
    fn missing_fields_fall_back_to_defaults() {
        let settings: Settings = serde_json::from_str(r#"{ "brightness": 0.6 }"#).unwrap();
        assert_eq!(settings.brightness, 0.6);
        assert_eq!(settings.draw_distance_tiles, 50);
        assert!(settings.properties_panel);
    }

    #[test]
    fn save_and_load_round_trip() {
        let directory =
            std::env::temp_dir().join(format!("rustosrs-settings-{}", std::process::id()));
        let path = directory.join("editor.json");
        let mut settings = Settings::default();
        settings.grid_zones = true;
        settings.fly_speed = 1234.0;
        settings
            .plugins
            .insert("demo".to_owned(), serde_json::json!({ "n": 1 }));
        settings.save(&path).unwrap();
        assert_eq!(Settings::load(&path), settings);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn unreadable_file_gives_defaults() {
        assert_eq!(
            Settings::load(std::path::Path::new("/nonexistent/rustosrs.json")),
            Settings::default()
        );
    }
}
