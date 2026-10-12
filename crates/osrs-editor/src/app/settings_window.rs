//! The settings window: every persisted option in one place, in pages with a search box.
//!
//! One table ([`ROWS`]) declares each setting once: its page, section, label and search keywords.
//! The window, the search results and the profile gating all read that table; [`EditorApp::draw_row`]
//! is the only place a row's widget exists. Adding a setting means one `Row` variant, one table
//! entry and one match arm. The side panel (inspector) never shows settings.

use super::EditorApp;
use crate::camera::CameraMode;
use crate::keymap::Binding;
use crate::settings::{
    ColorblindChoice, ContextMenuStyle, MinimapSize, ProfileChoice, Settings, SettingsPage,
};
use eframe::egui;
use egui_phosphor::regular as ph;
use osrs_hd::{
    environment::{DefaultSky, FogDepthMode},
    shadows::{ShadowFiltering, ShadowMode, ShadowResolution},
};

/// Which render profile a setting acts on.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Applies {
    Any,
    Reference,
    Hd,
}

/// One setting.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Row {
    Theme,
    SidePanel,
    DockRight,
    StatsOverlay,
    ShowMinimap,
    MinimapSize,
    ContextMenu,
    MouseTooltips,
    HoverTile,
    TileInfoHud,
    MapIcons3d,
    ObjectGizmo,
    RenderScale,
    CameraMode,
    FieldOfView,
    FlySpeed,
    LoadRadius,
    DrawDistance,
    AnimateObjects,
    SmoothTerrain,
    SkyColor,
    Profile,
    ReferenceBrightness,
    ReferenceFog,
    ColorBanding,
    BrightTextures,
    ColorblindMode,
    ColorblindStrength,
    HdBrightness,
    AtmosphericLighting,
    NormalMapping,
    Parallax,
    Lightning,
    CurrentArea,
    DistanceFog,
    FogDepthMode,
    StaticFogDepth,
    GroundFog,
    DefaultSky,
    OverrideSky,
    DecoupleSky,
    PohThemes,
    LegacyTob,
    ShadowMode,
    ShadowQuality,
    ShadowFiltering,
    ShadowDistance,
    ShadowTransparency,
    TerrainShadows,
    RoofShadows,
    ShadowMemory,
    Caustics,
    Exposure,
    Vignette,
}

struct RowDef {
    row: Row,
    page: SettingsPage,
    section: &'static str,
    label: &'static str,
    /// Extra words the search matches.
    keywords: &'static str,
    applies: Applies,
}

const fn def(
    row: Row,
    page: SettingsPage,
    section: &'static str,
    label: &'static str,
    keywords: &'static str,
    applies: Applies,
) -> RowDef {
    RowDef {
        row,
        page,
        section,
        label,
        keywords,
        applies,
    }
}

use Applies::{Any, Hd, Reference};
use SettingsPage::{
    Advanced, CameraWorld, Colors, Controls, Graphics, HdLighting, HdShadowsWater, Interface,
};

/// Every setting, in the order a page shows them.
const ROWS: &[RowDef] = &[
    def(
        Row::Theme,
        Interface,
        "Appearance",
        "Theme",
        "colour color palette dark light midnight",
        Any,
    ),
    def(
        Row::SidePanel,
        Interface,
        "Panels",
        "Side panel",
        "inspector properties",
        Any,
    ),
    def(
        Row::DockRight,
        Interface,
        "Panels",
        "Dock side panel on the right",
        "left right inspector",
        Any,
    ),
    def(
        Row::StatsOverlay,
        Interface,
        "Panels",
        "Statistics overlay",
        "fps frame time gpu memory performance",
        Any,
    ),
    def(
        Row::ShowMinimap,
        Interface,
        "Panels",
        "Minimap",
        "hud compass",
        Any,
    ),
    def(
        Row::MinimapSize,
        Interface,
        "Panels",
        "Minimap size",
        "small large hud",
        Any,
    ),
    def(
        Row::ContextMenu,
        Interface,
        "Menus and hints",
        "Context menu",
        "shift right click editor game runelite osrs choose option",
        Any,
    ),
    def(
        Row::MouseTooltips,
        Interface,
        "Menus and hints",
        "Game hover text",
        "mouse tooltips runelite action text",
        Any,
    ),
    def(
        Row::HoverTile,
        Interface,
        "Menus and hints",
        "Highlight hovered tile",
        "outline cursor",
        Any,
    ),
    def(
        Row::TileInfoHud,
        Interface,
        "Menus and hints",
        "Tile info",
        "hud corner cursor overlay",
        Any,
    ),
    def(
        Row::MapIcons3d,
        Interface,
        "Menus and hints",
        "Map icons in the world",
        "billboards bank shop altar icons 3d",
        Any,
    ),
    def(
        Row::ObjectGizmo,
        Interface,
        "Menus and hints",
        "Transform gizmo",
        "arrows move turn objects tool handles",
        Any,
    ),
    def(
        Row::CameraMode,
        CameraWorld,
        "Camera",
        "Camera mode",
        "fly orbit",
        Any,
    ),
    def(
        Row::FieldOfView,
        CameraWorld,
        "Camera",
        "Field of view",
        "fov zoom",
        Any,
    ),
    def(
        Row::FlySpeed,
        CameraWorld,
        "Camera",
        "Fly speed",
        "movement wasd",
        Any,
    ),
    def(
        Row::LoadRadius,
        CameraWorld,
        "World",
        "Load radius",
        "regions streaming memory",
        Any,
    ),
    def(
        Row::DrawDistance,
        CameraWorld,
        "World",
        "Draw distance",
        "tiles fog view",
        Any,
    ),
    def(
        Row::AnimateObjects,
        CameraWorld,
        "World",
        "Animate objects",
        "animations rest pose",
        Any,
    ),
    def(
        Row::SmoothTerrain,
        CameraWorld,
        "World",
        "Smooth terrain",
        "not in the game blend",
        Any,
    ),
    def(
        Row::SkyColor,
        CameraWorld,
        "World",
        "Sky colour",
        "background clear fog color",
        Any,
    ),
    def(
        Row::Profile,
        Graphics,
        "Renderer",
        "Render profile",
        "reference runelite hd 117 hd",
        Any,
    ),
    def(
        Row::RenderScale,
        Graphics,
        "Renderer",
        "Resolution scale",
        "supersampling resolution pixels quality performance anti aliasing",
        Any,
    ),
    def(
        Row::ReferenceBrightness,
        Graphics,
        "Reference options",
        "Brightness",
        "gamma",
        Reference,
    ),
    def(
        Row::ReferenceFog,
        Graphics,
        "Reference options",
        "Fog depth",
        "tiles",
        Reference,
    ),
    def(
        Row::ColorBanding,
        Graphics,
        "Reference options",
        "Remove color banding",
        "hsl",
        Reference,
    ),
    def(
        Row::BrightTextures,
        Graphics,
        "Reference options",
        "Bright textures",
        "texture light",
        Reference,
    ),
    def(
        Row::ColorblindMode,
        Graphics,
        "Accessibility",
        "Colorblind mode",
        "protanope deuteranope tritanope",
        Any,
    ),
    def(
        Row::ColorblindStrength,
        Graphics,
        "Accessibility",
        "Colorblind strength",
        "intensity",
        Any,
    ),
    def(
        Row::HdBrightness,
        HdLighting,
        "Lighting",
        "Brightness",
        "gamma",
        Hd,
    ),
    def(
        Row::AtmosphericLighting,
        HdLighting,
        "Lighting",
        "Atmospheric lighting",
        "area environment light",
        Hd,
    ),
    def(
        Row::NormalMapping,
        HdLighting,
        "Lighting",
        "Normal mapping",
        "materials bump",
        Hd,
    ),
    def(
        Row::Parallax,
        HdLighting,
        "Lighting",
        "Parallax occlusion mapping",
        "displacement height",
        Hd,
    ),
    def(
        Row::Lightning,
        HdLighting,
        "Lighting",
        "Lightning flashes",
        "storm photosensitivity",
        Hd,
    ),
    def(
        Row::CurrentArea,
        HdLighting,
        "Fog and sky",
        "Current area",
        "environment",
        Hd,
    ),
    def(
        Row::DistanceFog,
        HdLighting,
        "Fog and sky",
        "Distance fog",
        "haze draw distance off",
        Hd,
    ),
    def(
        Row::FogDepthMode,
        HdLighting,
        "Fog and sky",
        "Fog depth mode",
        "dynamic static",
        Hd,
    ),
    def(
        Row::StaticFogDepth,
        HdLighting,
        "Fog and sky",
        "Static fog depth",
        "tiles",
        Hd,
    ),
    def(
        Row::GroundFog,
        HdLighting,
        "Fog and sky",
        "Ground fog",
        "mist",
        Hd,
    ),
    def(
        Row::DefaultSky,
        HdLighting,
        "Fog and sky",
        "Default sky",
        "colour blue black tan",
        Hd,
    ),
    def(
        Row::OverrideSky,
        HdLighting,
        "Fog and sky",
        "Override sky colour",
        "force",
        Hd,
    ),
    def(
        Row::DecoupleSky,
        HdLighting,
        "Fog and sky",
        "Decouple sky and water colour",
        "water",
        Hd,
    ),
    def(
        Row::PohThemes,
        HdLighting,
        "Area themes",
        "Player-owned house themes",
        "poh",
        Hd,
    ),
    def(
        Row::LegacyTob,
        HdLighting,
        "Area themes",
        "Legacy Theatre of Blood",
        "tob",
        Hd,
    ),
    def(
        Row::ShadowMode,
        HdShadowsWater,
        "Shadows",
        "Shadow mode",
        "off fast detailed",
        Hd,
    ),
    def(
        Row::ShadowQuality,
        HdShadowsWater,
        "Shadows",
        "Shadow quality",
        "resolution 1k 4k 8k",
        Hd,
    ),
    def(
        Row::ShadowFiltering,
        HdShadowsWater,
        "Shadows",
        "Shadow filtering",
        "pcf pcss soft smooth dither",
        Hd,
    ),
    def(
        Row::ShadowDistance,
        HdShadowsWater,
        "Shadows",
        "Shadow distance",
        "tiles",
        Hd,
    ),
    def(
        Row::ShadowTransparency,
        HdShadowsWater,
        "Shadows",
        "Shadow transparency",
        "alpha",
        Hd,
    ),
    def(
        Row::TerrainShadows,
        HdShadowsWater,
        "Shadows",
        "Terrain shadows",
        "hills",
        Hd,
    ),
    def(
        Row::RoofShadows,
        HdShadowsWater,
        "Shadows",
        "Roof shadows",
        "hidden roofs",
        Hd,
    ),
    def(
        Row::ShadowMemory,
        HdShadowsWater,
        "Shadows",
        "Shadow map memory",
        "gpu",
        Hd,
    ),
    def(
        Row::Caustics,
        HdShadowsWater,
        "Water",
        "Underwater caustics",
        "light patterns sea floor",
        Hd,
    ),
    def(
        Row::Exposure,
        HdShadowsWater,
        "Post processing",
        "Exposure (placeholder)",
        "multiplier",
        Hd,
    ),
    def(
        Row::Vignette,
        HdShadowsWater,
        "Post processing",
        "Vignette (placeholder)",
        "corners",
        Hd,
    ),
];

/// The settings the side panel's Settings tab shows (the ones used all the time), in groups. Rows
/// that act on the other render profile are left out there; the window shows everything.
const QUICK: &[(&str, &[Row])] = &[
    (
        "Camera",
        &[Row::CameraMode, Row::FieldOfView, Row::FlySpeed],
    ),
    (
        "World",
        &[Row::DrawDistance, Row::LoadRadius, Row::AnimateObjects],
    ),
    (
        "Graphics",
        &[
            Row::Profile,
            Row::RenderScale,
            Row::ReferenceBrightness,
            Row::HdBrightness,
            Row::ReferenceFog,
            Row::DistanceFog,
            Row::DefaultSky,
            Row::ShadowMode,
        ],
    ),
    (
        "Interface",
        &[Row::ContextMenu, Row::ShowMinimap, Row::StatsOverlay],
    ),
];

const PAGES: &[(SettingsPage, &str, &str)] = &[
    (Interface, ph::LAYOUT, "Interface"),
    (CameraWorld, ph::VIDEO_CAMERA, "Camera and world"),
    (Graphics, ph::MONITOR, "Graphics"),
    (HdLighting, ph::SUN, "Sky and lighting"),
    (HdShadowsWater, ph::DROP, "Shadows and water"),
    (Controls, ph::KEYBOARD, "Controls"),
    (Colors, ph::PALETTE, "Colours"),
    (Advanced, ph::WRENCH, "Advanced"),
];

/// One entry of the settings page list: icon and title, left aligned, filled when selected.
fn nav_item(ui: &mut egui::Ui, selected: bool, icon: &str, title: &str) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 30.0), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let visuals = ui.visuals();
        let fill = if selected {
            visuals.selection.bg_fill
        } else if response.hovered() {
            visuals.widgets.hovered.weak_bg_fill
        } else {
            egui::Color32::TRANSPARENT
        };
        ui.painter().rect_filled(rect, 6.0, fill);
        let color = if selected {
            visuals.selection.stroke.color
        } else {
            visuals.widgets.inactive.fg_stroke.color
        };
        let font = egui::TextStyle::Body.resolve(ui.style());
        ui.painter().text(
            rect.left_center() + egui::vec2(10.0, 0.0),
            egui::Align2::LEFT_CENTER,
            icon,
            font.clone(),
            color,
        );
        ui.painter().text(
            rect.left_center() + egui::vec2(36.0, 0.0),
            egui::Align2::LEFT_CENTER,
            title,
            font,
            color,
        );
    }
    response
}

/// Mouse controls (keys are listed from the keymap).
const MOUSE_CONTROLS: &[(&str, &str)] = &[
    ("Left click", "select the object under the cursor"),
    ("Right drag", "look around (fly) / orbit (orbit camera)"),
    ("Shift + right drag, middle drag", "pan"),
    (
        "Shift + right click",
        "context menu (editor or game style: Settings > Interface)",
    ),
    ("Scroll", "fly speed (fly) / zoom (orbit)"),
    ("Shift + movement keys", "move faster"),
];

fn matches(def: &RowDef, query: &str) -> bool {
    let haystack = format!("{} {} {}", def.label, def.section, def.keywords).to_lowercase();
    query.split_whitespace().all(|word| haystack.contains(word))
}

impl EditorApp {
    /// Open the settings window on `page`.
    pub(super) fn open_settings(&mut self, page: Option<SettingsPage>) {
        if let Some(page) = page {
            self.prefs.settings_page = page;
        }
        self.settings_open = true;
    }

    /// The side panel's Settings tab: the frequently used rows, stacked (label over control) to
    /// fit a narrow panel. They are drawn by [`Self::draw_row`], like the window's.
    pub(super) fn quick_settings_ui(&mut self, ui: &mut egui::Ui, groups: &[&str]) {
        let hd = self.prefs.render_profile == ProfileChoice::Hd;
        // Leave room for the value box ("3x3 regions"); the cap stops a wide value from growing the panel.
        ui.spacing_mut().slider_width = (ui.available_width() - 120.0).clamp(80.0, 300.0);
        for (group, rows) in QUICK {
            if !groups.contains(group) {
                continue;
            }
            let shown: Vec<&RowDef> = rows
                .iter()
                .filter_map(|row| ROWS.iter().find(|def| def.row == *row))
                .filter(|def| match def.applies {
                    Any => true,
                    Reference => !hd,
                    Hd => hd,
                })
                .filter(|def| self.row_visible(def.row))
                .collect();
            if shown.is_empty() {
                continue;
            }
            crate::theme::section_heading(ui, group);
            for def in shown {
                let toggle = match def.row {
                    Row::AnimateObjects => Some(&mut self.prefs.animate_objects),
                    Row::ShowMinimap => Some(&mut self.prefs.show_minimap),
                    Row::StatsOverlay => Some(&mut self.prefs.stats_overlay),
                    _ => None,
                };
                if let Some(value) = toggle {
                    crate::theme::toggle(ui, value, def.label);
                    continue;
                }
                ui.label(egui::RichText::new(def.label).weak());
                self.draw_row(ui, def.row);
            }
        }
        ui.add_space(10.0);
        ui.separator();
        if ui
            .button(format!("{}  All settings...", ph::GEAR))
            .on_hover_text("Every option, with search")
            .clicked()
        {
            self.open_settings(None);
            self.settings_focus_search = true;
        }
    }

    /// Whether a row is shown at all (some only make sense next to another setting).
    fn row_visible(&self, row: Row) -> bool {
        match row {
            Row::StaticFogDepth => self.prefs.hd_environment.fog_depth_mode == FogDepthMode::Static,
            Row::ShadowQuality
            | Row::ShadowFiltering
            | Row::ShadowDistance
            | Row::ShadowTransparency
            | Row::TerrainShadows
            | Row::RoofShadows => self.prefs.hd_shadows.enabled(),
            Row::ShadowMemory => {
                self.prefs.hd_shadows.enabled() && self.renderer.hd_shadow_map_bytes() > 0
            }
            _ => true,
        }
    }

    /// The window. Called every frame; draws nothing while closed.
    pub(super) fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            self.capturing_binding = None;
            return;
        }
        // A binding waiting for a key press: the first key pressed becomes the binding.
        if let Some(id) = self.capturing_binding {
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.capturing_binding = None;
            } else if let Some(binding) = ctx.input(Binding::capture) {
                self.keymap.set(id, binding);
                self.capturing_binding = None;
            }
        }
        let esc = self.capturing_binding.is_none();
        let ((), close) =
            super::modal::Modal::new("settings_window", format!("{} Settings", ph::GEAR))
                .size(780.0, 520.0)
                .esc_closes(esc)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(ph::MAGNIFYING_GLASS);
                        let search = ui.add(
                            egui::TextEdit::singleline(&mut self.settings_search)
                                .hint_text("Search settings")
                                .desired_width(260.0),
                        );
                        if self.settings_focus_search {
                            search.request_focus();
                            self.settings_focus_search = false;
                        }
                        if !self.settings_search.is_empty() && ui.small_button("clear").clicked() {
                            self.settings_search.clear();
                        }
                    });
                    ui.separator();
                    ui.horizontal_top(|ui| {
                        ui.vertical(|ui| {
                            ui.set_width(176.0);
                            ui.spacing_mut().item_spacing.y = 3.0;
                            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                            for (page, icon, title) in PAGES {
                                let selected = self.settings_search.is_empty()
                                    && self.prefs.settings_page == *page;
                                if nav_item(ui, selected, icon, title).clicked() {
                                    self.prefs.settings_page = *page;
                                    self.settings_search.clear();
                                }
                            }
                        });
                        ui.separator();
                        ui.vertical(|ui| {
                            egui::ScrollArea::vertical()
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    if self.settings_search.trim().is_empty() {
                                        self.page_ui(ui, self.prefs.settings_page);
                                    } else {
                                        self.search_ui(ui);
                                    }
                                });
                        });
                    });
                });
        if close {
            self.settings_open = false;
        }
    }

    /// The colours the viewport draws its indicators, the grid and the HUD with.
    fn colors_page(&mut self, ui: &mut egui::Ui) {
        use crate::style::{ColorGroup, ColorKey};
        ui.weak(
            "Every outline, the grid and the HUD read these. The alpha is the opacity. \
             The UI theme (panels, text) is on the Interface page.",
        );
        ui.add_space(4.0);
        if ui.button("Reset all colours").clicked() {
            self.prefs.colors.reset_all();
        }
        for group in ColorGroup::ALL {
            crate::theme::section_heading(ui, group.label());
            egui::Grid::new(("colors", group.label()))
                .num_columns(3)
                .spacing([14.0, 6.0])
                .show(ui, |ui| {
                    for key in ColorKey::ALL.iter().filter(|k| k.group() == group) {
                        ui.label(key.label());
                        let [r, g, b, a] = self.prefs.colors.rgba(*key);
                        let mut color = egui::Color32::from_rgba_unmultiplied(r, g, b, a);
                        if ui.color_edit_button_srgba(&mut color).changed() {
                            let [r, g, b, a] = color.to_srgba_unmultiplied();
                            self.prefs.colors.set(*key, [r, g, b, a]);
                        }
                        if ui
                            .add_enabled(
                                !self.prefs.colors.is_default(*key),
                                egui::Button::new(ph::ARROW_COUNTER_CLOCKWISE).small(),
                            )
                            .on_hover_text("Back to the default")
                            .clicked()
                        {
                            self.prefs.colors.reset(*key);
                        }
                        ui.end_row();
                    }
                });
        }
    }

    /// One page.
    fn page_ui(&mut self, ui: &mut egui::Ui, page: SettingsPage) {
        let title = PAGES
            .iter()
            .find(|(candidate, _, _)| *candidate == page)
            .map_or("", |(_, _, title)| *title);
        ui.heading(title);
        ui.add_space(4.0);
        match page {
            Controls => self.controls_page(ui),
            Colors => self.colors_page(ui),
            Advanced => {
                self.rows_ui_for_page(ui, page, "");
                self.advanced_page(ui);
            }
            _ => {
                if matches!(page, HdLighting | HdShadowsWater)
                    && self.prefs.render_profile != ProfileChoice::Hd
                {
                    ui.horizontal(|ui| {
                        ui.colored_label(
                            egui::Color32::from_rgb(240, 180, 80),
                            "These options act on the HD profile.",
                        );
                        if ui.button("Switch to HD").clicked() {
                            self.prefs.render_profile = ProfileChoice::Hd;
                        }
                    });
                    ui.add_space(4.0);
                }
                self.rows_ui_for_page(ui, page, "");
            }
        }
    }

    /// Search results across every page, with the page named on each section.
    fn search_ui(&mut self, ui: &mut egui::Ui) {
        let query = self.settings_search.trim().to_lowercase();
        let found = ROWS
            .iter()
            .filter(|def| matches(def, &query) && self.row_visible(def.row))
            .count();
        ui.heading("Search results");
        if found == 0 {
            ui.weak("No setting matches.");
            return;
        }
        for (page, _, title) in PAGES {
            if ROWS
                .iter()
                .any(|def| def.page == *page && matches(def, &query) && self.row_visible(def.row))
            {
                ui.add_space(8.0);
                ui.label(egui::RichText::new(*title).weak());
                self.rows_ui_for_page(ui, *page, &query);
            }
        }
    }

    fn rows_ui_for_page(&mut self, ui: &mut egui::Ui, page: SettingsPage, query: &str) {
        let rows: Vec<&RowDef> = ROWS
            .iter()
            .filter(|def| {
                def.page == page
                    && (query.is_empty() || matches(def, query))
                    && self.row_visible(def.row)
            })
            .collect();
        let mut sections: Vec<&str> = Vec::new();
        for def in &rows {
            if !sections.contains(&def.section) {
                sections.push(def.section);
            }
        }
        for section in sections {
            crate::theme::section_heading(ui, section);
            ui.spacing_mut().slider_width = 220.0;
            egui::Grid::new(("settings_grid", section, page as u8))
                .num_columns(2)
                .spacing([14.0, 7.0])
                .min_col_width(190.0)
                .show(ui, |ui| {
                    for def in rows.iter().filter(|def| def.section == section) {
                        self.draw_labelled_row(ui, def);
                    }
                });
        }
    }

    /// A label and its widget, greyed out when the setting acts on the other render profile.
    fn draw_labelled_row(&mut self, ui: &mut egui::Ui, def: &RowDef) {
        let profile_hd = self.prefs.render_profile == ProfileChoice::Hd;
        let enabled = match def.applies {
            Any => true,
            Reference => !profile_hd,
            Hd => profile_hd,
        };
        let suffix = match (def.applies, enabled) {
            (Reference, false) => "  (Reference)",
            (Hd, false) => "  (HD)",
            _ => "",
        };
        ui.label(format!("{}{}", def.label, suffix));
        ui.add_enabled_ui(enabled, |ui| self.draw_row(ui, def.row));
        ui.end_row();
    }

    /// The widget of one row. This is the only place a setting's control exists.
    #[allow(clippy::too_many_lines)]
    fn draw_row(&mut self, ui: &mut egui::Ui, row: Row) {
        let prefs: &mut Settings = &mut self.prefs;
        match row {
            Row::Theme => {
                egui::ComboBox::from_id_salt("setting_theme")
                    .selected_text(crate::theme::NAMES[prefs.appearance])
                    .show_ui(ui, |ui| {
                        for (index, name) in crate::theme::NAMES.iter().enumerate() {
                            if ui
                                .selectable_value(&mut prefs.appearance, index, *name)
                                .clicked()
                            {
                                crate::theme::set_theme(ui.ctx(), crate::theme::ALL[index]);
                            }
                        }
                    });
            }
            Row::SidePanel => {
                crate::theme::toggle(ui, &mut prefs.properties_panel, "Show");
            }
            Row::DockRight => {
                crate::theme::toggle(ui, &mut prefs.dock_right, "Right (off: left)");
            }
            Row::StatsOverlay => {
                crate::theme::toggle(ui, &mut prefs.stats_overlay, "Show");
            }
            Row::ShowMinimap => {
                crate::theme::toggle(ui, &mut prefs.show_minimap, "Show");
            }
            Row::MinimapSize => {
                let mut at = MinimapSize::ALL
                    .iter()
                    .position(|size| *size == prefs.minimap_size)
                    .unwrap_or(0);
                let labels: Vec<&str> = MinimapSize::ALL.iter().map(|size| size.label()).collect();
                if crate::theme::segmented(ui, "minimap_size", &mut at, &labels) {
                    prefs.minimap_size = MinimapSize::ALL[at];
                }
            }
            Row::ContextMenu => {
                let mut at = ContextMenuStyle::ALL
                    .iter()
                    .position(|style| *style == prefs.context_menu)
                    .unwrap_or(0);
                let labels: Vec<&str> = ContextMenuStyle::ALL
                    .iter()
                    .map(|style| style.short_label())
                    .collect();
                if crate::theme::segmented(ui, "context_menu_style", &mut at, &labels) {
                    prefs.context_menu = ContextMenuStyle::ALL[at];
                }
            }
            Row::MouseTooltips => {
                crate::theme::toggle(
                    ui,
                    &mut prefs.mouse_tooltips,
                    "Show the top menu entry at the cursor",
                );
            }
            Row::HoverTile => {
                crate::theme::toggle(
                    ui,
                    &mut prefs.hover_tile,
                    "Outline the tile under the cursor",
                );
            }
            Row::TileInfoHud => {
                crate::theme::toggle(
                    ui,
                    &mut prefs.tile_info_hud,
                    "Show the tile under the cursor",
                );
            }
            Row::MapIcons3d => {
                crate::theme::toggle(
                    ui,
                    &mut prefs.map_icons_3d,
                    "Draw map icons over their tiles",
                );
            }
            Row::ObjectGizmo => {
                crate::theme::toggle(
                    ui,
                    &mut prefs.object_gizmo,
                    "Arrows and a turn ring on selected objects",
                );
            }
            Row::RenderScale => {
                ui.add(
                    egui::Slider::new(&mut prefs.render_scale, 0.5..=2.0)
                        .step_by(0.25)
                        .custom_formatter(|v, _| format!("{:.0}%", v * 100.0))
                        .custom_parser(|text| {
                            text.trim_end_matches('%').trim().parse::<f64>().ok().map(|v| v / 100.0)
                        }),
                )
                .on_hover_text("Above 100% renders more pixels than the window has and smooths edges; below saves GPU time");
            }
            Row::CameraMode => {
                let mut at = usize::from(self.camera.mode == CameraMode::Orbit);
                if crate::theme::segmented(ui, "camera_mode", &mut at, &["Fly", "Orbit"]) {
                    self.camera.mode = if at == 0 {
                        CameraMode::Fly
                    } else {
                        CameraMode::Orbit
                    };
                }
            }
            Row::FieldOfView => {
                ui.add(egui::Slider::new(&mut self.camera.fov_degrees, 30.0..=100.0).suffix("°"));
            }
            Row::FlySpeed => {
                ui.add(
                    egui::Slider::new(&mut self.camera.speed, 50.0..=20_000.0)
                        .logarithmic(true)
                        .fixed_decimals(0)
                        .suffix(" u/s"),
                );
            }
            Row::LoadRadius => {
                ui.add(
                    egui::Slider::new(&mut prefs.load_radius, 0..=4).custom_formatter(
                        |radius, _| {
                            let side = radius as u32 * 2 + 1;
                            format!("{side}\u{d7}{side} regions")
                        },
                    ),
                );
            }
            Row::DrawDistance => {
                ui.add(egui::Slider::new(&mut prefs.draw_distance_tiles, 8..=184).suffix(" tiles"));
            }
            Row::AnimateObjects => {
                crate::theme::toggle(ui, &mut prefs.animate_objects, "Play object animations");
            }
            Row::SmoothTerrain => {
                if ui
                    .checkbox(&mut prefs.smooth_terrain, "Blend terrain heights")
                    .changed()
                {
                    self.rebuild_all();
                }
            }
            Row::SkyColor => {
                ui.horizontal(|ui| {
                    ui.color_edit_button_rgb(&mut prefs.sky_color);
                    ui.weak("Reference profile; HD: Default sky > Editor sky colour");
                });
            }
            Row::Profile => {
                egui::ComboBox::from_id_salt("setting_profile")
                    .selected_text(prefs.render_profile.label())
                    .show_ui(ui, |ui| {
                        for profile in ProfileChoice::ALL {
                            ui.selectable_value(
                                &mut prefs.render_profile,
                                profile,
                                profile.label(),
                            );
                        }
                    });
            }
            Row::ReferenceBrightness => {
                ui.add(egui::Slider::new(&mut prefs.brightness, 0.5..=1.0));
            }
            Row::ReferenceFog => {
                ui.add(egui::Slider::new(&mut prefs.fog_depth_tiles, 0..=100).suffix(" tiles"));
            }
            Row::ColorBanding => {
                crate::theme::toggle(ui, &mut prefs.remove_color_banding, "On");
            }
            Row::BrightTextures => {
                crate::theme::toggle(ui, &mut prefs.bright_textures, "On");
            }
            Row::ColorblindMode => {
                egui::ComboBox::from_id_salt("setting_colorblind")
                    .selected_text(format!("{:?}", prefs.colorblind_mode))
                    .show_ui(ui, |ui| {
                        for mode in ColorblindChoice::ALL {
                            ui.selectable_value(
                                &mut prefs.colorblind_mode,
                                mode,
                                format!("{mode:?}"),
                            );
                        }
                    });
            }
            Row::ColorblindStrength => {
                ui.add(egui::Slider::new(
                    &mut prefs.colorblind_intensity,
                    0.0..=100.0,
                ));
            }
            Row::HdBrightness => {
                ui.add(egui::Slider::new(&mut prefs.hd_brightness, 25.0..=400.0).suffix(" %"))
                    .on_hover_text("A gamma on the finished image; 100 % = as authored.");
            }
            Row::AtmosphericLighting => {
                crate::theme::toggle(
                    ui,
                    &mut prefs.hd_environment.atmospheric_lighting,
                    "Light each area with its own environment",
                );
            }
            Row::NormalMapping => {
                crate::theme::toggle(ui, &mut prefs.hd_normal_mapping, "On");
            }
            Row::Parallax => {
                crate::theme::toggle(ui, &mut prefs.hd_parallax_mapping, "On (higher GPU cost)");
            }
            Row::Lightning => {
                crate::theme::toggle(
                    ui,
                    &mut prefs.hd_environment.flashing_effects,
                    "Flash in storm areas",
                );
            }
            Row::CurrentArea => {
                ui.label(
                    self.hd
                        .current_name()
                        .unwrap_or_else(|| "(loading)".to_owned()),
                );
            }
            Row::DistanceFog => {
                crate::theme::toggle(
                    ui,
                    &mut prefs.hd_environment.distance_fog,
                    "On (off removes all fog)",
                );
            }
            Row::FogDepthMode => {
                egui::ComboBox::from_id_salt("setting_fog_mode")
                    .selected_text(prefs.hd_environment.fog_depth_mode.label())
                    .show_ui(ui, |ui| {
                        for mode in FogDepthMode::ALL {
                            ui.selectable_value(
                                &mut prefs.hd_environment.fog_depth_mode,
                                mode,
                                mode.label(),
                            );
                        }
                    });
            }
            Row::StaticFogDepth => {
                ui.add(
                    egui::Slider::new(&mut prefs.hd_environment.static_fog_depth, 0.0..=100.0)
                        .suffix(" tiles"),
                );
            }
            Row::GroundFog => {
                crate::theme::toggle(ui, &mut prefs.hd_environment.ground_fog, "On");
            }
            Row::DefaultSky => {
                egui::ComboBox::from_id_salt("setting_default_sky")
                    .selected_text(prefs.hd_environment.default_sky.label())
                    .show_ui(ui, |ui| {
                        for sky in DefaultSky::ALL {
                            ui.selectable_value(
                                &mut prefs.hd_environment.default_sky,
                                sky,
                                sky.label(),
                            );
                        }
                    });
            }
            Row::OverrideSky => {
                crate::theme::toggle(
                    ui,
                    &mut prefs.hd_environment.override_sky,
                    "Use the default sky in every area",
                );
            }
            Row::DecoupleSky => {
                crate::theme::toggle(ui, &mut prefs.hd_environment.decouple_sky_and_water, "On");
            }
            Row::PohThemes => {
                crate::theme::toggle(ui, &mut prefs.hd_environment.poh_themes, "On");
            }
            Row::LegacyTob => {
                crate::theme::toggle(ui, &mut prefs.hd_environment.legacy_tob, "On");
            }
            Row::ShadowMode => {
                egui::ComboBox::from_id_salt("setting_shadow_mode")
                    .selected_text(prefs.hd_shadows.mode.label())
                    .show_ui(ui, |ui| {
                        for mode in ShadowMode::ALL {
                            ui.selectable_value(&mut prefs.hd_shadows.mode, mode, mode.label());
                        }
                    });
            }
            Row::ShadowQuality => {
                egui::ComboBox::from_id_salt("setting_shadow_quality")
                    .selected_text(prefs.hd_shadows.resolution.label())
                    .show_ui(ui, |ui| {
                        for resolution in ShadowResolution::ALL {
                            ui.selectable_value(
                                &mut prefs.hd_shadows.resolution,
                                resolution,
                                resolution.label(),
                            );
                        }
                    });
            }
            Row::ShadowFiltering => {
                egui::ComboBox::from_id_salt("setting_shadow_filtering")
                    .selected_text(prefs.hd_shadows.filtering.label())
                    .show_ui(ui, |ui| {
                        for filtering in ShadowFiltering::ALL {
                            ui.selectable_value(
                                &mut prefs.hd_shadows.filtering,
                                filtering,
                                filtering.label(),
                            );
                        }
                    });
            }
            Row::ShadowDistance => {
                ui.add(
                    egui::Slider::new(&mut prefs.hd_shadows.distance_tiles, 4..=184)
                        .suffix(" tiles"),
                );
            }
            Row::ShadowTransparency => {
                crate::theme::toggle(
                    ui,
                    &mut prefs.hd_shadows.transparency,
                    "Take model transparency into account",
                );
            }
            Row::TerrainShadows => {
                crate::theme::toggle(
                    ui,
                    &mut prefs.hd_shadows.terrain_shadows,
                    "Hills cast shadows",
                );
            }
            Row::RoofShadows => {
                crate::theme::toggle(
                    ui,
                    &mut prefs.hd_shadows.roof_shadows,
                    "Hidden roofs still cast shadows",
                );
            }
            Row::ShadowMemory => {
                ui.weak(format!(
                    "{:.0} MiB",
                    self.renderer.hd_shadow_map_bytes() as f64 / 1_048_576.0
                ));
            }
            Row::Caustics => {
                crate::theme::toggle(ui, &mut prefs.hd_underwater_caustics, "On");
            }
            Row::Exposure => {
                ui.add(egui::Slider::new(&mut prefs.hd_exposure, 0.25..=4.0))
                    .on_hover_text("A multiplier on the finished image; the plugin has none.");
            }
            Row::Vignette => {
                ui.add(egui::Slider::new(&mut prefs.hd_vignette, 0.0..=1.0));
            }
        }
    }

    /// Mouse controls and the editable keyboard shortcuts.
    fn controls_page(&mut self, ui: &mut egui::Ui) {
        crate::theme::section_heading(ui, "Mouse");
        egui::Grid::new("mouse_controls")
            .num_columns(2)
            .spacing([14.0, 6.0])
            .min_col_width(190.0)
            .show(ui, |ui| {
                for (input, action) in MOUSE_CONTROLS {
                    ui.weak(*input);
                    ui.label(*action);
                    ui.end_row();
                }
            });
        crate::theme::section_heading(ui, "Keyboard");
        ui.horizontal(|ui| {
            ui.weak("Click a shortcut, then press the new keys. Esc cancels.");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Reset all").clicked() {
                    self.keymap.reset_all();
                }
            });
        });
        ui.add_space(4.0);
        let actions: Vec<_> = self.keymap.actions().to_vec();
        let mut categories: Vec<&str> = Vec::new();
        for action in &actions {
            if !categories.contains(&action.category) {
                categories.push(action.category);
            }
        }
        for category in categories {
            ui.add_space(6.0);
            ui.label(egui::RichText::new(category).strong());
            egui::Grid::new(("key_bindings", category))
                .num_columns(3)
                .spacing([14.0, 5.0])
                .min_col_width(190.0)
                .show(ui, |ui| {
                    for action in actions.iter().filter(|a| a.category == category) {
                        ui.weak(action.label);
                        let waiting = self.capturing_binding == Some(action.id);
                        let text = if waiting {
                            "Press keys...".to_owned()
                        } else {
                            self.keymap
                                .binding(action.id)
                                .map_or_else(|| "Unbound".to_owned(), |b| b.describe())
                        };
                        if ui
                            .add_sized(
                                [150.0, 24.0],
                                egui::Button::new(egui::RichText::new(text).monospace().size(12.5))
                                    .selected(waiting),
                            )
                            .clicked()
                        {
                            self.capturing_binding = Some(action.id);
                        }
                        ui.horizontal(|ui| {
                            if self.keymap.is_overridden(action.id)
                                && ui.small_button("Reset").clicked()
                            {
                                self.keymap.reset(action.id);
                            }
                            if let Some(binding) = self.keymap.binding(action.id) {
                                let conflicts = self.keymap.conflicts(action.id, binding);
                                if !conflicts.is_empty() {
                                    ui.colored_label(
                                        ui.visuals().warn_fg_color,
                                        format!("also: {}", conflicts.join(", ")),
                                    );
                                }
                            }
                        });
                        ui.end_row();
                    }
                });
        }
    }

    /// Reset buttons and where the settings file lives.
    fn advanced_page(&mut self, ui: &mut egui::Ui) {
        crate::theme::section_heading(ui, "Reset");
        ui.horizontal(|ui| {
            if ui
                .button("Reset the window layout")
                .on_hover_text("Side panel, minimap and statistics back to their defaults.")
                .clicked()
            {
                let defaults = Settings::default();
                self.prefs.properties_panel = defaults.properties_panel;
                self.prefs.dock_right = defaults.dock_right;
                self.prefs.stats_overlay = defaults.stats_overlay;
                self.prefs.show_minimap = defaults.show_minimap;
                self.prefs.minimap_size = defaults.minimap_size;
                self.prefs.rail_tab = defaults.rail_tab;
            }
            if ui
                .button("Reset the HD options")
                .on_hover_text(
                    "Lighting, fog, sky, shadows, water and brightness of the HD profile.",
                )
                .clicked()
            {
                let defaults = Settings::default();
                self.prefs.hd_brightness = defaults.hd_brightness;
                self.prefs.hd_exposure = defaults.hd_exposure;
                self.prefs.hd_vignette = defaults.hd_vignette;
                self.prefs.hd_normal_mapping = defaults.hd_normal_mapping;
                self.prefs.hd_parallax_mapping = defaults.hd_parallax_mapping;
                self.prefs.hd_underwater_caustics = defaults.hd_underwater_caustics;
                self.prefs.hd_environment = defaults.hd_environment;
                self.prefs.hd_shadows = defaults.hd_shadows;
            }
        });
        ui.add_space(6.0);
        crate::theme::section_heading(ui, "Settings file");
        match crate::settings::path() {
            Some(path) => {
                ui.label(path.display().to_string());
            }
            None => {
                ui.weak("not saved (no settings path)");
            }
        }
        ui.weak(
            "Settings are saved automatically; unknown or missing fields never break the file.",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_is_declared_once_with_a_distinct_label_per_section() {
        for (index, def) in ROWS.iter().enumerate() {
            assert!(
                !ROWS[..index].iter().any(|other| other.row == def.row),
                "{} is listed twice",
                def.label
            );
            assert!(
                !ROWS[..index].iter().any(|other| {
                    other.page == def.page
                        && other.section == def.section
                        && other.label == def.label
                }),
                "{} appears twice in {}",
                def.label,
                def.section
            );
        }
    }

    #[test]
    fn rows_of_a_section_are_contiguous_so_the_page_draws_one_grid_per_section() {
        let mut seen: Vec<(SettingsPage, &str)> = Vec::new();
        let mut last: Option<(SettingsPage, &str)> = None;
        for def in ROWS {
            let key = (def.page, def.section);
            if last != Some(key) {
                assert!(!seen.contains(&key), "section {} is split", def.section);
                seen.push(key);
                last = Some(key);
            }
        }
    }

    #[test]
    fn every_page_with_settings_has_rows() {
        for (page, _, title) in PAGES {
            let has_rows = ROWS.iter().any(|def| def.page == *page);
            // Controls (shortcuts), Colours and Advanced (reset buttons) are drawn by their own code.
            assert!(
                has_rows || matches!(page, Controls | Colors | Advanced),
                "{title} has no settings"
            );
        }
    }

    #[test]
    fn search_matches_labels_sections_and_keywords() {
        let find = |query: &str| -> Vec<&'static str> {
            ROWS.iter()
                .filter(|def| matches(def, query))
                .map(|def| def.label)
                .collect()
        };
        assert!(find("shadow").contains(&"Shadow quality"));
        assert!(find("shadow").contains(&"Terrain shadows"));
        // Keywords: the sky colour is found as "background" and "fog colour".
        assert!(find("background").contains(&"Sky colour"));
        // Every word has to match, in any order.
        assert_eq!(
            find("fog depth"),
            vec!["Fog depth", "Fog depth mode", "Static fog depth"]
        );
        // The context menu setting is discoverable by its old names.
        assert!(find("runelite").contains(&"Context menu"));
        assert!(find("choose option").contains(&"Context menu"));
        assert!(find("zzzz").is_empty());
    }

    #[test]
    fn the_quick_settings_are_real_rows_listed_once() {
        let mut seen: Vec<Row> = Vec::new();
        for (group, rows) in QUICK {
            assert!(!rows.is_empty(), "{group} is empty");
            for row in *rows {
                assert!(
                    ROWS.iter().any(|def| def.row == *row),
                    "a quick row has no table entry"
                );
                assert!(!seen.contains(row), "a quick row is listed twice");
                seen.push(*row);
            }
        }
        // The window keeps every setting; the quick tab is a subset.
        assert!(seen.len() < ROWS.len());
    }

    #[test]
    fn hd_only_rows_are_not_offered_on_the_reference_pages() {
        for def in ROWS {
            if def.applies == Hd {
                assert!(
                    matches!(def.page, HdLighting | HdShadowsWater),
                    "{} is an HD option on a general page",
                    def.label
                );
            }
        }
    }
}
