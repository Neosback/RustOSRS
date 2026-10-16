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

/// High-contrast graphite built for reading: near-white text, clearly stepped surfaces, a bright
/// blue accent. The default.
pub const INK: Theme = Theme {
    name: "Ink",
    dark: true,
    panel: rgb(19, 22, 28),
    window: rgb(28, 32, 40),
    inset: rgb(12, 14, 19),
    widget: rgb(42, 48, 60),
    widget_hover: rgb(55, 63, 79),
    widget_active: rgb(68, 79, 99),
    border: rgb(52, 59, 73),
    text: rgb(241, 244, 249),
    text_dim: rgb(176, 185, 200),
    accent: rgb(98, 168, 255),
    danger: rgb(255, 117, 117),
    warning: rgb(250, 190, 72),
};

/// The egui-elegance "slate" look: cool dark blue with a sky focus ring.
pub const SLATE: Theme = Theme {
    name: "Slate",
    dark: true,
    panel: rgb(15, 23, 42),
    window: rgb(30, 41, 59),
    inset: rgb(11, 17, 32),
    widget: rgb(40, 52, 72),
    widget_hover: rgb(51, 65, 85),
    widget_active: rgb(71, 85, 105),
    border: rgb(51, 65, 85),
    text: rgb(226, 232, 240),
    text_dim: rgb(148, 163, 184),
    accent: rgb(56, 189, 248),
    danger: rgb(248, 113, 113),
    warning: rgb(251, 191, 36),
};

/// The egui-elegance "charcoal" look: neutral dark grey with a cyan accent.
pub const CHARCOAL: Theme = Theme {
    name: "Charcoal",
    dark: true,
    panel: rgb(20, 20, 22),
    window: rgb(28, 28, 30),
    inset: rgb(15, 15, 16),
    widget: rgb(42, 42, 45),
    widget_hover: rgb(56, 56, 58),
    widget_active: rgb(72, 72, 76),
    border: rgb(56, 56, 58),
    text: rgb(250, 250, 250),
    text_dim: rgb(161, 161, 170),
    accent: rgb(34, 211, 238),
    danger: rgb(248, 113, 113),
    warning: rgb(251, 191, 36),
};

/// The egui-elegance "frost" look: slate-tinted off-white, light.
pub const FROST: Theme = Theme {
    name: "Frost",
    dark: false,
    panel: rgb(226, 232, 240),
    window: rgb(248, 250, 252),
    inset: rgb(255, 255, 255),
    widget: rgb(213, 221, 232),
    widget_hover: rgb(200, 210, 224),
    widget_active: rgb(182, 195, 214),
    border: rgb(190, 200, 214),
    text: rgb(15, 23, 42),
    text_dim: rgb(71, 85, 105),
    accent: rgb(3, 116, 176),
    danger: rgb(220, 38, 38),
    warning: rgb(217, 119, 6),
};

/// The egui-elegance "paper" look: warm neutral off-white, light.
pub const PAPER: Theme = Theme {
    name: "Paper",
    dark: false,
    panel: rgb(236, 233, 228),
    window: rgb(250, 248, 243),
    inset: rgb(255, 255, 255),
    widget: rgb(226, 222, 214),
    widget_hover: rgb(214, 209, 199),
    widget_active: rgb(199, 193, 180),
    border: rgb(188, 182, 168),
    text: rgb(28, 26, 22),
    text_dim: rgb(87, 82, 74),
    accent: rgb(12, 128, 158),
    danger: rgb(220, 38, 38),
    warning: rgb(217, 119, 6),
};

/// Variant names, parallel to [`ALL`].
pub const NAMES: [&str; 8] = [
    GRAPHITE.name,
    MIDNIGHT.name,
    DAYLIGHT.name,
    SLATE.name,
    CHARCOAL.name,
    FROST.name,
    PAPER.name,
    INK.name,
];
/// All variants.
pub const ALL: [Theme; 8] = [
    GRAPHITE, MIDNIGHT, DAYLIGHT, SLATE, CHARCOAL, FROST, PAPER, INK,
];

/// The theme a fresh install starts with (an index into [`ALL`]).
pub const DEFAULT: usize = 7;

/// The egui-elegance theme that matches `theme`, so its widgets (switches, segmented controls,
/// buttons) wear the same colours as everything else.
fn elegance_theme(theme: &Theme) -> elegance::Theme {
    let mut base = if theme.dark {
        elegance::Theme::slate()
    } else {
        elegance::Theme::frost()
    };
    let p = &mut base.palette;
    p.bg = theme.panel;
    p.card = theme.window;
    p.input_bg = theme.inset;
    p.border = theme.border;
    p.text = theme.text;
    p.text_muted = theme.text_dim;
    p.text_faint = theme.text_dim.gamma_multiply(0.75);
    p.focus = theme.accent;
    p.danger = theme.danger;
    p.warning = theme.warning;
    // Filled buttons carry white text, so the primary blue is the accent pulled darker on dark themes.
    let primary = if theme.dark {
        theme.accent.gamma_multiply(0.62)
    } else {
        theme.accent
    };
    p.blue = primary;
    p.blue_hover = primary.gamma_multiply(0.85);
    base
}

/// A labelled on/off switch (the editor's one boolean control; replaces checkboxes).
pub fn toggle(
    ui: &mut egui::Ui,
    value: &mut bool,
    label: impl Into<egui::WidgetText>,
) -> egui::Response {
    ui.add(elegance::Switch::new(value, label))
}

/// One choice out of a few, as a segmented control; returns whether the choice changed.
pub fn segmented(ui: &mut egui::Ui, id: &str, selected: &mut usize, labels: &[&str]) -> bool {
    let before = *selected;
    ui.add(elegance::SegmentedControl::new(selected, labels.iter().copied()).id_salt(id));
    *selected != before
}

/// A filled call-to-action button (Save, Run, Go).
pub fn primary_button(text: impl Into<egui::WidgetText>) -> elegance::Button {
    elegance::Button::new(text).accent(elegance::Accent::Blue)
}

/// A quiet outlined button (Cancel).
pub fn outline_button(text: impl Into<egui::WidgetText>) -> elegance::Button {
    elegance::Button::new(text).outline()
}

/// Black or white, whichever reads better on `fill` (checkmarks and labels on the accent).
pub fn contrast_on(fill: Color32) -> Color32 {
    let channel = |c: u8| {
        let c = f32::from(c) / 255.0;
        if c <= 0.039_28 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let luminance =
        0.2126 * channel(fill.r()) + 0.7152 * channel(fill.g()) + 0.0722 * channel(fill.b());
    if luminance > 0.35 {
        Color32::from_rgb(14, 17, 22)
    } else {
        Color32::WHITE
    }
}

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
            .color(ui.visuals().weak_text_color()),
    );
    ui.add_space(6.0);
}

/// Apply `theme` (visuals and the shared style) to the context.
pub fn set_theme(ctx: &egui::Context, theme: Theme) {
    // Elegance first (it also restyles the context), then ours on top, so only its widgets keep
    // its palette.
    elegance_theme(&theme).install(ctx);
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

/// A context with the app's spacing and type scale (default fonts), for panel layout tests.
#[cfg(test)]
pub(crate) fn layout_test_context(scale: f32) -> egui::Context {
    let ctx = egui::Context::default();
    ctx.set_pixels_per_point(scale);
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.style_mut_of(egui::Theme::Dark, apply_layout);
    ctx
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
            FontId::new(12.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(14.5, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(14.5, FontFamily::Proportional),
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
            // Plain labels use the noninteractive foreground, so it is the full text colour; only
            // `weak()` text (hints, captions) is dimmed.
            weak_text_color: Some(self.text_dim),
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
                noninteractive: self.widget_visuals(self.panel, hairline, self.text, 6),
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
                bg_fill: self.accent.gamma_multiply(0.42),
                stroke: Stroke::new(1.0, self.accent),
            },
            striped: false,
            slider_trailing_fill: true,
            interact_cursor: Some(egui::CursorIcon::PointingHand),
            ..base
        }
    }
}
