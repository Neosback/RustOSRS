//! The editor's look: a calm, flat, modern interface.
//!
//! A theme is a small palette (background, surfaces, text, one accent); [`set_theme`] turns it
//! into egui visuals plus the shared spacing, rounding and type scale, so every theme has the same
//! layout and only the colours change. Widgets are flat (no outlines at rest), corners are soft,
//! the accent colour is used sparingly for selection and focus.

use eframe::egui::{
    self, Color32, CornerRadius, FontFamily, FontId, Margin, Shadow, Stroke, TextStyle, style,
};

/// The colours of one theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub name: &'static str,
    pub dark: bool,
    /// Side panels and the status bar.
    pub panel: Color32,
    /// Windows, menus and popups (a step above the panels).
    pub window: Color32,
    /// Text fields and the viewport surround (a step below the panels).
    pub inset: Color32,
    /// Buttons and inputs at rest, hovered, and pressed.
    pub widget: Color32,
    pub widget_hover: Color32,
    pub widget_active: Color32,
    /// Hairlines and separators.
    pub border: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub accent: Color32,
    pub danger: Color32,
    pub warning: Color32,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// The default: neutral graphite with a soft blue accent.
pub const GRAPHITE: Theme = Theme {
    name: "Graphite",
    dark: true,
    panel: rgb(31, 34, 40),
    window: rgb(38, 42, 49),
    inset: rgb(24, 26, 31),
    widget: rgb(50, 55, 64),
    widget_hover: rgb(61, 67, 78),
    widget_active: rgb(72, 79, 92),
    border: rgb(58, 63, 73),
    text: rgb(228, 231, 236),
    text_dim: rgb(147, 155, 168),
    accent: rgb(108, 142, 255),
    danger: rgb(255, 107, 107),
    warning: rgb(245, 165, 36),
};

/// Deep blue-black with a teal accent, for dark rooms.
pub const MIDNIGHT: Theme = Theme {
    name: "Midnight",
    dark: true,
    panel: rgb(14, 17, 24),
    window: rgb(20, 24, 33),
    inset: rgb(9, 11, 16),
    widget: rgb(31, 37, 50),
    widget_hover: rgb(40, 48, 65),
    widget_active: rgb(50, 60, 80),
    border: rgb(36, 43, 58),
    text: rgb(222, 228, 240),
    text_dim: rgb(128, 140, 163),
    accent: rgb(72, 207, 190),
    danger: rgb(255, 120, 120),
    warning: rgb(240, 180, 70),
};

/// A clean light theme.
pub const DAYLIGHT: Theme = Theme {
    name: "Daylight",
    dark: false,
    panel: rgb(243, 244, 247),
    window: rgb(255, 255, 255),
    inset: rgb(231, 233, 238),
    widget: rgb(232, 235, 241),
    widget_hover: rgb(220, 224, 233),
    widget_active: rgb(205, 211, 224),
    border: rgb(211, 215, 224),
    text: rgb(30, 35, 48),
    text_dim: rgb(98, 107, 125),
    accent: rgb(52, 104, 240),
    danger: rgb(214, 55, 62),
    warning: rgb(196, 118, 0),
};

/// Variant names, parallel to [`ALL`].
pub const NAMES: [&str; 3] = [GRAPHITE.name, MIDNIGHT.name, DAYLIGHT.name];
/// All variants.
pub const ALL: [Theme; 3] = [GRAPHITE, MIDNIGHT, DAYLIGHT];

/// A section heading: small, upper case, in the accent colour, with a hairline under it. The one
/// heading style for the side panel and the settings window, so groups read the same everywhere.
pub fn section_heading(ui: &mut egui::Ui, text: &str) {
    ui.add_space(10.0);
    ui.label(
        egui::RichText::new(text.to_uppercase())
            .size(11.0)
            .strong()
            .color(ui.visuals().selection.stroke.color.gamma_multiply(0.9)),
    );
    ui.add(egui::Separator::default().spacing(3.0));
}

/// A keycap: a key or mouse gesture in a small rounded box, followed by its action in dim text.
pub fn key_hint(ui: &mut egui::Ui, key: &str, action: &str) {
    egui::Frame::new()
        .fill(ui.visuals().widgets.inactive.bg_fill)
        .corner_radius(4.0)
        .inner_margin(egui::Margin::symmetric(5, 1))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(key).size(11.5).strong());
        });
    ui.label(
        egui::RichText::new(action)
            .size(12.0)
            .color(ui.visuals().widgets.noninteractive.fg_stroke.color),
    );
    ui.add_space(6.0);
}

/// Apply `theme` (visuals and the shared style) to the context.
pub fn set_theme(ctx: &egui::Context, theme: Theme) {
    let slot = if theme.dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    };
    ctx.set_theme(if theme.dark {
        egui::ThemePreference::Dark
    } else {
        egui::ThemePreference::Light
    });
    ctx.style_mut_of(slot, |style| {
        apply_layout(style);
        style.visuals = theme.visuals();
    });
}

/// Spacing, type scale and interaction sizes shared by every theme.
fn apply_layout(style: &mut egui::Style) {
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    style.spacing.window_margin = Margin::same(12);
    style.spacing.menu_margin = Margin::same(6);
    style.spacing.interact_size = egui::vec2(36.0, 26.0);
    style.spacing.icon_width = 16.0;
    style.spacing.icon_spacing = 6.0;
    style.spacing.indent = 18.0;
    style.spacing.slider_width = 150.0;
    style.spacing.combo_width = 120.0;
    style.spacing.scroll.bar_width = 8.0;
    style.spacing.scroll.floating = true;
    style.interaction.tooltip_delay = 0.35;
    style.text_styles = [
        (
            TextStyle::Small,
            FontId::new(11.5, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(14.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Heading,
            FontId::new(18.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(13.0, FontFamily::Monospace),
        ),
    ]
    .into();
}

impl Theme {
    fn widget_visuals(
        &self,
        fill: Color32,
        stroke: Stroke,
        text: Color32,
        radius: u8,
    ) -> style::WidgetVisuals {
        style::WidgetVisuals {
            bg_fill: fill,
            weak_bg_fill: fill,
            bg_stroke: stroke,
            corner_radius: CornerRadius::same(radius),
            fg_stroke: Stroke::new(1.0, text),
            expansion: 0.0,
        }
    }

    fn visuals(&self) -> egui::Visuals {
        let base = if self.dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        let shadow = Color32::from_black_alpha(if self.dark { 110 } else { 38 });
        let hairline = Stroke::new(1.0, self.border);
        egui::Visuals {
            dark_mode: self.dark,
            override_text_color: None,
            hyperlink_color: self.accent,
            faint_bg_color: self.window,
            extreme_bg_color: self.inset,
            code_bg_color: self.inset,
            warn_fg_color: self.warning,
            error_fg_color: self.danger,
            window_fill: self.window,
            panel_fill: self.panel,
            window_stroke: hairline,
            window_corner_radius: CornerRadius::same(10),
            menu_corner_radius: CornerRadius::same(8),
            window_shadow: Shadow {
                offset: [0, 8],
                blur: 24,
                spread: 0,
                color: shadow,
            },
            popup_shadow: Shadow {
                offset: [0, 6],
                blur: 18,
                spread: 0,
                color: shadow,
            },
            widgets: style::Widgets {
                noninteractive: self.widget_visuals(self.panel, hairline, self.text_dim, 6),
                inactive: self.widget_visuals(self.widget, Stroke::NONE, self.text, 6),
                hovered: self.widget_visuals(
                    self.widget_hover,
                    Stroke::new(1.0, self.accent.gamma_multiply(0.45)),
                    self.text,
                    6,
                ),
                active: self.widget_visuals(
                    self.widget_active,
                    Stroke::new(1.0, self.accent),
                    self.text,
                    6,
                ),
                open: self.widget_visuals(self.widget_hover, Stroke::NONE, self.text, 6),
            },
            selection: style::Selection {
                bg_fill: self.accent.gamma_multiply(0.38),
                stroke: Stroke::new(1.0, self.accent),
            },
            striped: false,
            slider_trailing_fill: true,
            interact_cursor: Some(egui::CursorIcon::PointingHand),
            ..base
        }
    }
}
