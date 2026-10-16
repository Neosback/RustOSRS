//! Small labeled controls, grouped by the appearance they change.
use eframe::egui;
use osrs_hd::theme::{
    ThemeDefinition, ThemeNumber, ThemeOverrides,
    shader::{ShaderExtension, ShaderStage},
};

pub(super) fn draw(
    ui: &mut egui::Ui,
    theme: &mut ThemeDefinition,
    search: &str,
    shader_edit: &mut bool,
    tile: osrs_core::coords::MapTile,
    plane: osrs_core::coords::PlaneIndex,
) {
    let search = search.to_lowercase();
    for section in ["Lighting", "Sky & Fog", "Water", "Effects"] {
        let keywords = match section {
            "Lighting" => "lighting ambient sunlight sun color strength",
            "Sky & Fog" => "sky fog color depth",
            "Water" => "water tint underwater caustics",
            _ => "effects exposure brightness normal maps parallax depth",
        };
        if !search.is_empty() && !keywords.contains(&search) {
            continue;
        }
        egui::CollapsingHeader::new(section)
            .default_open(true)
            .show(ui, |ui| {
                parameters(ui, &mut theme.overrides, section);
                if ui.small_button(format!("Reset {section}")).clicked() {
                    reset(&mut theme.overrides, section);
                }
            });
    }
    egui::CollapsingHeader::new("Materials").show(ui, |ui| {
        ui.label("Using stock 117HD materials. Image replacement authoring is coming in the next checkpoint.");
        for material in &theme.materials {
            ui.label(format!("{} · based on {}", material.name, material.parent));
        }
        if theme.materials.is_empty() { ui.weak("Using stock 117HD materials"); }
    });
    super::rules::draw(ui, theme, tile, plane);
    egui::CollapsingHeader::new("Advanced · Shaders").show(ui, |ui| {
        ui.small("WGSL templates run inside the renderer's fixed stages. Apply compiles a candidate before changing the viewport.");
        for stage in ShaderStage::ALL {
            if !theme.shaders.iter().any(|s| s.stage == stage) && ui.button(format!("Add {stage:?} template")).clicked() {
                theme.shaders.push(ShaderExtension::new(stage)); *shader_edit = true;
            }
        }
        let mut remove = None;
        for (index, shader) in theme.shaders.iter_mut().enumerate() {
            ui.push_id(index, |ui| {
                ui.horizontal(|ui| { ui.label(format!("{:?}.wgsl", shader.stage)); if ui.small_button("Remove").clicked() { remove = Some(index); } });
                if ui.add(egui::TextEdit::multiline(&mut shader.source).code_editor().desired_width(f32::INFINITY).desired_rows(9)).changed() { *shader_edit = true; }
            });
        }
        if let Some(index) = remove { theme.shaders.remove(index); *shader_edit = true; }
    });
}
pub(super) fn parameters(ui: &mut egui::Ui, v: &mut ThemeOverrides, section: &str) {
    match section {
        "Lighting" => {
            color(ui, "Ambient color", &mut v.ambient_color, [151, 186, 255]);
            scalar(
                ui,
                "Ambient strength",
                &mut v.ambient_strength,
                1.0,
                0.0..=8.0,
            );
            color(ui, "Sun color", &mut v.sunlight_color, [255; 3]);
            scalar(
                ui,
                "Sun strength",
                &mut v.sunlight_strength,
                4.0,
                0.0..=12.0,
            );
        }
        "Sky & Fog" => {
            color(ui, "Fog color", &mut v.fog_color, [185, 210, 255]);
            scalar(ui, "Fog depth", &mut v.fog_depth, 6.5, 0.0..=100.0);
        }
        "Water" => {
            color(ui, "Water tint", &mut v.water_color, [0, 117, 142]);
            toggle(ui, "Underwater caustics", &mut v.underwater_caustics, true);
        }
        "Effects" => {
            scalar(ui, "Exposure", &mut v.exposure, 1.0, 0.05..=4.0);
            scalar(ui, "Brightness (%)", &mut v.brightness, 100.0, 20.0..=200.0);
            toggle(ui, "Normal maps", &mut v.normal_mapping, true);
            toggle(ui, "Parallax depth", &mut v.parallax_mapping, true);
        }
        _ => {}
    }
}
fn scalar(
    ui: &mut egui::Ui,
    label: &str,
    slot: &mut Option<ThemeNumber>,
    fallback: f32,
    range: std::ops::RangeInclusive<f32>,
) {
    ui.horizontal(|ui| {
        let mut enabled = slot.is_some();
        if ui.checkbox(&mut enabled, label).changed() {
            *slot = if enabled {
                ThemeNumber::new(fallback).ok()
            } else {
                None
            };
        }
    });
    if let Some(value) = slot {
        let mut number = value.get();
        if ui
            .add(egui::Slider::new(&mut number, range).text(label))
            .changed()
            && let Ok(n) = ThemeNumber::new(number)
        {
            *value = n;
        }
    }
}
fn color(ui: &mut egui::Ui, label: &str, slot: &mut Option<[u8; 3]>, fallback: [u8; 3]) {
    ui.horizontal(|ui| {
        let mut enabled = slot.is_some();
        if ui.checkbox(&mut enabled, label).changed() {
            *slot = enabled.then_some(fallback);
        }
        if let Some(color) = slot {
            ui.color_edit_button_srgb(color);
        }
    });
}
fn toggle(ui: &mut egui::Ui, label: &str, slot: &mut Option<bool>, fallback: bool) {
    ui.horizontal(|ui| {
        let mut value = slot.unwrap_or(fallback);
        if ui.checkbox(&mut value, label).changed() {
            *slot = Some(value);
        }
        if slot.is_some() && ui.small_button("Use base").clicked() {
            *slot = None;
        }
    });
}
fn reset(v: &mut ThemeOverrides, section: &str) {
    match section {
        "Lighting" => {
            v.ambient_color = None;
            v.ambient_strength = None;
            v.sunlight_color = None;
            v.sunlight_strength = None;
        }
        "Sky & Fog" => {
            v.fog_color = None;
            v.fog_depth = None;
        }
        "Water" => {
            v.water_color = None;
            v.underwater_caustics = None;
        }
        "Effects" => {
            v.exposure = None;
            v.brightness = None;
            v.normal_mapping = None;
            v.parallax_mapping = None;
        }
        _ => {}
    }
}
