from pathlib import Path

settings = Path('crates/osrs-editor/src/app/settings_io.rs')
text = settings.read_text()
start = text.index('fn finite_f32_or(')
end = text.index('impl EditorApp {', start)
replacement = '''fn finite_f32_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

/// Clamp persisted Spline settings to the same narrow domain the simplified UI exposes.
fn sanitize_loaded_settings(settings: Settings) -> Settings {
    let mut settings = settings.sanitized();
    let defaults = crate::paint::SplineSettings::default();
    let spline = &mut settings.spline;
    spline.width = finite_f32_or(spline.width, defaults.width).clamp(0.5, 24.0);
    spline.edge_weight =
        finite_f32_or(spline.edge_weight, defaults.edge_weight).clamp(0.0, 2.0);
    settings
}

'''
text = text[:start] + replacement + text[end:]

tests_start = text.index('#[cfg(test)]\nmod tests {')
text = text[:tests_start] + '''#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loaded_spline_settings_are_clamped_to_simple_fitter_ranges() {
        let mut settings = Settings::default();
        settings.spline.width = 999.0;
        settings.spline.edge_weight = -5.0;
        settings.spline.snap_points = true;

        let settings = sanitize_loaded_settings(settings);

        assert_eq!(settings.spline.width, 24.0);
        assert_eq!(settings.spline.edge_weight, 0.0);
        assert!(settings.spline.snap_points);
    }

    #[test]
    fn loaded_spline_settings_recover_from_non_finite_values() {
        let mut settings = Settings::default();
        settings.spline.width = f32::NAN;
        settings.spline.edge_weight = f32::INFINITY;
        let defaults = crate::paint::SplineSettings::default();

        let settings = sanitize_loaded_settings(settings);

        assert_eq!(settings.spline.width, defaults.width);
        assert_eq!(settings.spline.edge_weight, defaults.edge_weight);
    }
}
'''
settings.write_text(text)

workspace = Path('crates/osrs-editor/src/app/workspace.rs')
text = workspace.read_text()
start = text.index('            Tool::Spline => {')
end = text.index('            Tool::Height => {', start)
replacement = '''            Tool::Spline => {
                crate::theme::section_heading(ui, "Path");
                ui.horizontal(|ui| {
                    ui.weak("Width");
                    ui.add(
                        egui::DragValue::new(&mut self.prefs.spline.width)
                            .range(0.5..=24.0)
                            .speed(0.1)
                            .suffix(" tiles"),
                    );
                    ui.weak("Edge");
                    ui.add(
                        egui::DragValue::new(&mut self.prefs.spline.edge_weight)
                            .range(0.0..=2.0)
                            .speed(0.05),
                    );
                });
            }
'''
workspace.write_text(text[:start] + replacement + text[end:])

editing = Path('crates/osrs-editor/src/app/editing.rs')
text = editing.read_text()
text = text.replace(
    '''    pub(super) fn current_tile_values(&self, key: TileKey) -> Option<TileValues> {
        self.current_tile(key)
    }

''',
    '',
    1,
)
editing.write_text(text)

Path('scripts/fix_simple_path_followups.py').unlink(missing_ok=True)
