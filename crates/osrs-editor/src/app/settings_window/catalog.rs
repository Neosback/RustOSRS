//! The settings table's vocabulary: rows, pages, and the quick-settings groups.

use crate::settings::SettingsPage;
use SettingsPage::{
    Advanced, CameraWorld, Colors, Controls, Graphics, HdLighting, HdShadowsWater, Interface,
    Plugins,
};
use egui_phosphor::regular as ph;

/// Which render profile a setting acts on.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Applies {
    Any,
    Reference,
    Hd,
}

/// One setting.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Row {
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

pub(super) struct RowDef {
    pub(super) row: Row,
    pub(super) page: SettingsPage,
    pub(super) section: &'static str,
    pub(super) label: &'static str,
    /// Extra words the search matches.
    pub(super) keywords: &'static str,
    pub(super) applies: Applies,
}

pub(super) const fn def(
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

/// The settings the side panel's Settings tab shows (the ones used all the time), in groups. Rows
/// that act on the other render profile are left out there; the window shows everything.
pub(super) const QUICK: &[(&str, &[Row])] = &[
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

pub(super) const PAGES: &[(SettingsPage, &str, &str)] = &[
    (Interface, ph::LAYOUT, "Interface"),
    (CameraWorld, ph::VIDEO_CAMERA, "Camera and world"),
    (Graphics, ph::MONITOR, "Graphics"),
    (HdLighting, ph::SUN, "Sky and lighting"),
    (HdShadowsWater, ph::DROP, "Shadows and water"),
    (Controls, ph::KEYBOARD, "Controls"),
    (Colors, ph::PALETTE, "Colours"),
    (Plugins, ph::PUZZLE_PIECE, "Plugin options"),
    (Advanced, ph::WRENCH, "Advanced"),
];

/// Mouse controls (keys are listed from the keymap).
pub(super) const MOUSE_CONTROLS: &[(&str, &str)] = &[
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

pub(super) fn matches(def: &RowDef, query: &str) -> bool {
    let haystack = format!("{} {} {}", def.label, def.section, def.keywords).to_lowercase();
    query.split_whitespace().all(|word| haystack.contains(word))
}
