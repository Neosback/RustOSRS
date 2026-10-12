from pathlib import Path
import subprocess

subprocess.run(['python3', 'scripts/add_path_continuity_repair.py'], check=True)

settings = Path('crates/osrs-editor/src/terrain_path/settings.rs')
text = settings.read_text()
old = '''    /// Keep tiles that already carry an overlay.\n    pub preserve_existing: bool,\n'''
new = '''    /// Repair small raster/shape disconnects after normal Terraini planning. Disabled in the\n    /// reference-compatible planner defaults; the interactive editor enables it by default.\n    pub repair_continuity: bool,\n    /// Keep tiles that already carry an overlay.\n    pub preserve_existing: bool,\n'''
assert old in text, 'PathSettings field insertion point changed'
text = text.replace(old, new, 1)
old = '''            maximum_slope: f64::MAX,\n            preserve_existing: true,\n'''
new = '''            maximum_slope: f64::MAX,\n            repair_continuity: false,\n            preserve_existing: true,\n'''
assert old in text, 'PathSettings default insertion point changed'
text = text.replace(old, new, 1)
settings.write_text(text)

paint = Path('crates/osrs-editor/src/paint.rs')
text = paint.read_text()
old = '''            planner: crate::terrain_path::PathSettings::default(),\n'''
new = '''            planner: crate::terrain_path::PathSettings {\n                repair_continuity: true,\n                ..crate::terrain_path::PathSettings::default()\n            },\n'''
assert old in text, 'SplineSettings planner default changed'
text = text.replace(old, new, 1)
paint.write_text(text)

shelf = Path('crates/osrs-editor/src/app/shelf.rs')
text = shelf.read_text()
old = '''        ui.checkbox(&mut planner.preserve_existing, "Preserve existing overlays");\n\n        egui::CollapsingHeader::new("Advanced")\n'''
new = '''        ui.checkbox(&mut planner.preserve_existing, "Preserve existing overlays");\n        ui.checkbox(&mut planner.repair_continuity, "Repair path continuity")\n            .on_hover_text("Restore tiny raster gaps and repair side-adjacent shape disconnects");\n\n        egui::CollapsingHeader::new("Advanced")\n'''
assert old in text, 'Path shelf continuity insertion point changed'
text = text.replace(old, new, 1)
shelf.write_text(text)

planner = Path('crates/osrs-editor/src/terrain_path/planner.rs')
text = planner.read_text()
old = '''    repair_coverage_continuity(&strong, &mut weak, &continuity_candidates);\n    // The plan's tiles, in plane, x, y order.\n'''
new = '''    if settings.repair_continuity {\n        repair_coverage_continuity(&strong, &mut weak, &continuity_candidates);\n    }\n    // The plan's tiles, in plane, x, y order.\n'''
assert old in text, 'coverage continuity call changed'
text = text.replace(old, new, 1)
old = '''    repair_shape_continuity(&merged, &index_of, &mut shapes, atlas);\n\n    // The paints.\n'''
new = '''    if settings.repair_continuity {\n        repair_shape_continuity(&merged, &index_of, &mut shapes, atlas);\n    }\n\n    // The paints.\n'''
assert old in text, 'shape continuity call changed'
text = text.replace(old, new, 1)
planner.write_text(text)
