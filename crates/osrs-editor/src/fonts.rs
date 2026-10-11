//! Fonts: the platform's UI font for text (crisp, familiar, nothing to ship) plus the Phosphor icon
//! font. Missing system fonts fall back to egui's built-in ones.

use eframe::egui;

/// UI font candidates per platform, best first.
const SYSTEM_FONTS: &[&str] = &[
    // macOS
    "/System/Library/Fonts/SFNS.ttf",
    "/System/Library/Fonts/Helvetica.ttc",
    // Windows
    "C:\\Windows\\Fonts\\segoeui.ttf",
    // Linux
    "/usr/share/fonts/truetype/inter/Inter-Regular.ttf",
    "/usr/share/fonts/opentype/inter/Inter-Regular.otf",
    "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
];

pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    if let Some(bytes) = SYSTEM_FONTS
        .iter()
        .find_map(|path| std::fs::read(path).ok())
    {
        fonts.font_data.insert(
            "system-ui".to_owned(),
            egui::FontData::from_owned(bytes).into(),
        );
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "system-ui".to_owned());
    }
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    ctx.set_fonts(fonts);
    ctx.global_style_mut(|style| {
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
    });
}
