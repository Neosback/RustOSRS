//! Tile / zone / region grid overlay.
//!
//! Lines follow the terrain: every tile edge is one segment between its two corner heights, so the
//! grid drapes exactly over the ground. Tile lines fade out within a short radius, zone (8 tiles)
//! lines reach farther and region (64 tiles) borders farthest. The renderer draws the lines
//! depth-tested over the scene (see `SceneRenderer::set_overlay_lines`).

use crate::keymap::{ActionDef, Binding};
use crate::pick::Infos;
use crate::plugin::{EditorCommand, EditorPlugin, PluginContext};
use crate::style::{ColorKey, Colors};
use eframe::egui;
use osrs_render::gpu::LineVertex;

const TILE_RADIUS: i32 = 24;
const ZONE_RADIUS: i32 = 96;
const REGION_RADIUS: i32 = 384;
/// Lines sit this far above the terrain (render units; y is negative up).
const LIFT: f32 = 2.0;

/// Which grid levels are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct GridSettings {
    pub tiles: bool,
    pub zones: bool,
    pub regions: bool,
}

impl GridSettings {
    pub fn any(self) -> bool {
        self.tiles || self.zones || self.regions
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Level {
    Tile,
    Zone,
    Region,
}

impl Level {
    fn radius(self) -> i32 {
        match self {
            Self::Tile => TILE_RADIUS,
            Self::Zone => ZONE_RADIUS,
            Self::Region => REGION_RADIUS,
        }
    }

    /// The colour setting that styles this level (its alpha is the line opacity).
    fn key(self) -> ColorKey {
        match self {
            Self::Tile => ColorKey::GridTile,
            Self::Zone => ColorKey::GridZone,
            Self::Region => ColorKey::GridRegion,
        }
    }

    /// Level of a grid line at tile coordinate `coordinate` (a line along the other axis).
    fn of_line(coordinate: i32) -> Self {
        if coordinate.rem_euclid(64) == 0 {
            Self::Region
        } else if coordinate.rem_euclid(8) == 0 {
            Self::Zone
        } else {
            Self::Tile
        }
    }

    fn enabled(self, settings: GridSettings) -> bool {
        match self {
            Self::Tile => settings.tiles,
            Self::Zone => settings.zones,
            Self::Region => settings.regions,
        }
    }
}

/// Terrain height at tile corner `(x, y)` on `plane`.
fn corner_height(infos: &Infos, x: i32, y: i32, plane: usize) -> Option<f32> {
    let region = (x.div_euclid(64), y.div_euclid(64));
    infos.get(&region).map(|info| {
        info.corner_height(plane, x.rem_euclid(64) as usize, y.rem_euclid(64) as usize) as f32
    })
}

struct Builder<'a> {
    infos: &'a Infos,
    colors: &'a Colors,
    plane: usize,
    center: (i32, i32),
    origin: (i32, i32),
    lines: Vec<LineVertex>,
}

impl Builder<'_> {
    fn vertex(&self, x: i32, y: i32, height: f32, color: [f32; 4]) -> LineVertex {
        LineVertex {
            position: [
                (x * 128 - self.origin.0) as f32,
                height - LIFT,
                (y * 128 - self.origin.1) as f32,
            ],
            color,
        }
    }

    /// Segment between adjacent corners `a` and `b`.
    fn segment(&mut self, a: (i32, i32), b: (i32, i32), level: Level) {
        let mid = ((a.0 + b.0) as f32 * 0.5, (a.1 + b.1) as f32 * 0.5);
        let distance = (mid.0 - self.center.0 as f32)
            .abs()
            .max((mid.1 - self.center.1 as f32).abs());
        let radius = level.radius() as f32;
        if distance > radius {
            return;
        }
        let (Some(ha), Some(hb)) = (
            corner_height(self.infos, a.0, a.1, self.plane),
            corner_height(self.infos, b.0, b.1, self.plane),
        ) else {
            return;
        };
        let fade = (1.0 - distance / radius).clamp(0.0, 1.0).powf(0.7);
        let [r, g, bl, alpha] = self.colors.linear(level.key());
        let color = [r, g, bl, alpha * fade];
        let va = self.vertex(a.0, a.1, ha, color);
        let vb = self.vertex(b.0, b.1, hb, color);
        self.lines.push(va);
        self.lines.push(vb);
    }
}

/// Build the line list around `center` (world tile) on `plane`. Positions are relative to the
/// renderer's `origin` (world units).
pub fn build(
    settings: GridSettings,
    colors: &Colors,
    infos: &Infos,
    center: (i32, i32),
    plane: usize,
    origin: (i32, i32),
) -> Vec<LineVertex> {
    let mut builder = Builder {
        infos,
        colors,
        plane,
        center,
        origin,
        lines: Vec::new(),
    };
    if settings.tiles || settings.zones {
        let radius = if settings.zones {
            ZONE_RADIUS
        } else {
            TILE_RADIUS
        };
        for y in center.1 - radius..=center.1 + radius {
            for x in center.0 - radius..=center.0 + radius {
                // Segment along x on the line at row `y`, along y on the line at column `x`.
                let row = Level::of_line(y);
                if row != Level::Region && row.enabled(settings) {
                    builder.segment((x, y), (x + 1, y), row);
                }
                let column = Level::of_line(x);
                if column != Level::Region && column.enabled(settings) {
                    builder.segment((x, y), (x, y + 1), column);
                }
            }
        }
    }
    if settings.regions {
        let first = |c: i32| (c - REGION_RADIUS).div_euclid(64) * 64;
        for line_y in (first(center.1)..=center.1 + REGION_RADIUS).step_by(64) {
            for x in center.0 - REGION_RADIUS..=center.0 + REGION_RADIUS {
                builder.segment((x, line_y), (x + 1, line_y), Level::Region);
            }
        }
        for line_x in (first(center.0)..=center.0 + REGION_RADIUS).step_by(64) {
            for y in center.1 - REGION_RADIUS..=center.1 + REGION_RADIUS {
                builder.segment((line_x, y), (line_x, y + 1), Level::Region);
            }
        }
    }
    builder.lines
}

/// Inputs the grid lines were last built for.
#[derive(Clone, Copy, PartialEq, Eq)]
struct BuiltFor {
    settings: GridSettings,
    center: (i32, i32),
    plane: u8,
    origin: (i32, i32),
    infos_version: u64,
    colors: u64,
}

/// The grid overlay as an editor plugin.
pub struct GridPlugin {
    settings: GridSettings,
    built_for: Option<BuiltFor>,
}

impl GridPlugin {
    pub fn new() -> Self {
        let mut plugin = Self {
            settings: GridSettings::default(),
            built_for: None,
        };
        plugin.apply_env();
        plugin
    }

    /// Smoke-test hook: `RUSTOSRS_GRID="tiles,zones,regions"`.
    fn apply_env(&mut self) {
        if let Ok(value) = std::env::var("RUSTOSRS_GRID") {
            self.settings = GridSettings {
                tiles: value.contains("tiles"),
                zones: value.contains("zones"),
                regions: value.contains("regions"),
            };
        }
    }

    fn toggles(&mut self, ui: &mut egui::Ui) {
        crate::theme::toggle(ui, &mut self.settings.tiles, "Tiles");
        crate::theme::toggle(ui, &mut self.settings.zones, "Zones (8 tiles)");
        crate::theme::toggle(ui, &mut self.settings.regions, "Regions (64 tiles)");
    }
}

impl EditorPlugin for GridPlugin {
    fn name(&self) -> &'static str {
        "grid"
    }

    fn title(&self) -> &'static str {
        "Grid"
    }

    fn update(&mut self, cx: &mut PluginContext) {
        let key = BuiltFor {
            settings: self.settings,
            center: cx.view.camera_tile,
            plane: cx.view.view_plane,
            origin: cx.view.render_origin,
            infos_version: cx.view.infos_version,
            colors: cx.view.colors.stamp(),
        };
        if self.built_for == Some(key) {
            return;
        }
        self.built_for = Some(key);
        let lines = if self.settings.any() {
            build(
                self.settings,
                cx.view.colors,
                cx.view.infos,
                key.center,
                usize::from(key.plane),
                key.origin,
            )
        } else {
            Vec::new()
        };
        cx.send(EditorCommand::SetLines(lines));
    }

    fn has_menu(&self) -> bool {
        true
    }

    fn menu(&mut self, ui: &mut egui::Ui, _cx: &mut PluginContext) {
        self.toggles(ui);
    }

    fn toolbar(&mut self, ui: &mut egui::Ui, _cx: &mut PluginContext) {
        ui.toggle_value(
            &mut self.settings.tiles,
            format!("{} Grid", crate::icons::GRID),
        )
        .on_hover_text("Tile grid (G); the arrow chooses zone and region lines");
        ui.menu_button(egui_phosphor::regular::CARET_DOWN, |ui| self.toggles(ui));
    }

    fn has_properties(&self) -> bool {
        true
    }

    fn properties(&mut self, ui: &mut egui::Ui, _cx: &mut PluginContext) {
        self.toggles(ui);
        ui.weak(
            "Lines drape over the terrain; tile lines fade within 24 tiles, zones 96, regions 384.",
        );
    }

    fn actions(&self) -> &'static [ActionDef] {
        const ACTIONS: &[ActionDef] = &[ActionDef::new(
            "grid.toggle",
            "Toggle tile grid",
            "View",
            Binding::key(egui::Key::G),
        )];
        ACTIONS
    }

    fn on_action(&mut self, id: &'static str, _cx: &mut PluginContext) {
        if id == "grid.toggle" {
            self.settings.tiles = !self.settings.tiles;
        }
    }

    fn save_settings(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "tiles": self.settings.tiles,
            "zones": self.settings.zones,
            "regions": self.settings.regions,
        }))
    }

    fn load_settings(&mut self, value: &serde_json::Value) {
        let flag = |name: &str| value.get(name).and_then(serde_json::Value::as_bool);
        self.settings = GridSettings {
            tiles: flag("tiles").unwrap_or(false),
            zones: flag("zones").unwrap_or(false),
            regions: flag("regions").unwrap_or(false),
        };
        self.apply_env();
    }
}
