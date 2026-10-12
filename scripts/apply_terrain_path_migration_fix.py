from pathlib import Path

path = Path('crates/osrs-editor/src/app/workspace.rs')
text = path.read_text()
old = '''            Tool::Spline => {
                crate::theme::section_heading(ui, "Path");
                ui.horizontal(|ui| {
                    ui.weak("Width");
                    ui.add(
                        egui::DragValue::new(&mut self.prefs.spline.width)
                            .range(1.0..=32.0)
                            .speed(0.1)
                            .suffix(" tiles"),
                    );
                    for style in crate::autotile::EdgeStyle::ALL {
                        ui.selectable_value(&mut self.prefs.spline.style, style, style.label());
                    }
                });
            }
'''
new = '''            Tool::Spline => {
                crate::theme::section_heading(ui, "Path");
                ui.horizontal(|ui| {
                    ui.weak("Width");
                    ui.add(
                        egui::DragValue::new(&mut self.prefs.spline.profile.base_width)
                            .range(0.5..=24.0)
                            .speed(0.1)
                            .suffix(" tiles"),
                    );
                    for mode in crate::paint::PathMode::ALL {
                        ui.selectable_value(&mut self.prefs.spline.mode, mode, mode.label());
                    }
                });
            }
'''
assert old in text, 'workspace spline quick options changed unexpectedly'
path.write_text(text.replace(old, new))

path = Path('crates/osrs-editor/src/app/editing.rs')
text = path.read_text()
old = 'use crate::paint::{Brush, Fill, TileValues, Tool};'
new = 'use crate::paint::{Brush, TileValues, Tool};'
assert old in text, 'editing import changed unexpectedly'
path.write_text(text.replace(old, new))

path = Path('crates/osrs-editor/src/terrain_path/mod.rs')
text = path.read_text()
old = '''pub use model::{
    CostGrid, Occupancy, PathEdge, PathEnvironment, PathNetwork, PathNode, PathPlan, PathPoint,
    PathWarning, Severity, TileBounds, TilePaint,
};
'''
new = 'pub use model::{PathEnvironment, PathPoint, PathWarning, Severity, TilePaint};\n'
assert old in text, 'terrain_path model reexports changed unexpectedly'
text = text.replace(old, new)
old = 'pub use settings::{AutoConnectPolicy, MaterialVariant, ObstaclePolicy, PathProfile, PathSettings};'
new = 'pub use settings::{AutoConnectPolicy, ObstaclePolicy, PathProfile, PathSettings};'
assert old in text, 'terrain_path settings reexports changed unexpectedly'
path.write_text(text.replace(old, new))

path = Path('crates/osrs-editor/src/app/spline_tool.rs')
text = path.read_text()
old = 'let mut tile_polygon = |tile: (i32, i32), shape: u8, rotation: u8|'
new = 'let tile_polygon = |tile: (i32, i32), shape: u8, rotation: u8|'
assert old in text, 'spline polygon closure changed unexpectedly'
path.write_text(text.replace(old, new))
