from pathlib import Path

path = Path('crates/osrs-editor/src/app/shelf.rs')
text = path.read_text()
old = '''        egui::CollapsingHeader::new("Advanced")
            .default_open(false)
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Sample spacing").weak());
                ui.add(
                    egui::Slider::new(&mut planner.sample_spacing, 0.05..=1.0)
                        .max_decimals(2)
                        .suffix(" tiles"),
                );
                ui.label(egui::RichText::new("Supersampling").weak());
                ui.add(egui::Slider::new(&mut planner.supersample, 2..=16));
                ui.label(egui::RichText::new("Noise strength").weak());
                ui.add(egui::Slider::new(&mut planner.noise_strength, 0.0..=3.0).max_decimals(2));
                ui.horizontal(|ui| {
                    ui.label("Seed");
                    ui.add(egui::DragValue::new(&mut planner.seed).speed(1));
                });
            });
'''
new = old + '''
        let notes = self.spline_notes();
        if !notes.is_empty() {
            crate::theme::section_heading(ui, "Preview");
            for (_, note) in notes {
                ui.label(note);
            }
        }
'''
assert old in text, 'path advanced controls changed unexpectedly'
path.write_text(text.replace(old, new, 1))

path = Path('crates/osrs-editor/src/terrain_path/atlas.rs')
text = path.read_text()
for old in [
    '''    pub fn len(&self) -> usize {\n        self.bits\n    }\n\n''',
    '''    pub fn words(&self) -> &[u64] {\n        &self.words\n    }\n''',
]:
    assert old in text, f'atlas helper changed unexpectedly: {old!r}'
    text = text.replace(old, '', 1)
path.write_text(text)

path = Path('crates/osrs-editor/src/terrain_path/model.rs')
text = path.read_text()
old = '''pub struct PathNetwork {
    pub id: u64,
    pub name: String,
'''
new = '''pub struct PathNetwork {
    #[allow(dead_code, reason = "stable identity carried by the network model")]
    pub id: u64,
    #[allow(dead_code, reason = "human-readable network metadata")]
    pub name: String,
'''
assert old in text, 'PathNetwork fields changed unexpectedly'
text = text.replace(old, new, 1)
old = '''impl TilePaint {
    pub fn new(
'''
new = '''impl TilePaint {
    #[cfg(test)]
    pub fn new(
'''
assert old in text, 'TilePaint::new changed unexpectedly'
text = text.replace(old, new, 1)
old = '''    pub affected: TileBounds,
'''
new = '''    #[allow(dead_code, reason = "part of the plan API for incremental rebuild consumers")]
    pub affected: TileBounds,
'''
assert old in text, 'PathPlan::affected changed unexpectedly'
text = text.replace(old, new, 1)
old = '''/// A grid with nothing on it.
pub struct OpenGround;

impl CostGrid for OpenGround {}
'''
new = '''/// A grid with nothing on it, used by planner tests.
#[cfg(test)]
pub struct OpenGround;

#[cfg(test)]
impl CostGrid for OpenGround {}
'''
assert old in text, 'OpenGround changed unexpectedly'
text = text.replace(old, new, 1)
old = '''impl PathEnvironment {
    pub fn empty(bounds: TileBounds) -> Self {
'''
new = '''impl PathEnvironment {
    #[cfg(test)]
    pub fn empty(bounds: TileBounds) -> Self {
'''
assert old in text, 'PathEnvironment::empty changed unexpectedly'
text = text.replace(old, new, 1)
path.write_text(text)

path = Path('crates/osrs-editor/src/terrain_path/golden.rs')
text = path.read_text()
old = '''fn numbers(line: &str) -> Vec<f64> {
    line.split_whitespace()
        .filter_map(|t| t.parse().ok())
        .collect()
}

'''
assert old in text, 'unused golden numbers helper changed unexpectedly'
path.write_text(text.replace(old, '', 1))
