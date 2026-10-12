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
settings.write_text(text[:start] + replacement + text[end:])

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
