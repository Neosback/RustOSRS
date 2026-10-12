from pathlib import Path

planner = Path('crates/osrs-editor/src/terrain_path/planner.rs')
text = planner.read_text()
text = text.replace(
    'edge_seed ^ 0x4E45_5457_4F52_4B_i64,',
    'edge_seed ^ 0x004E_4554_574F_524B_i64,',
    1,
)
text = text.replace(
    'let (a_shape, a_orientation, b_shape, b_orientation) = pair.expect("shape pair");',
    'assert!(pair.is_some(), "the atlas should contain a disconnected east/west shape pair");\n        let (a_shape, a_orientation, b_shape, b_orientation) = pair.unwrap_or_default();',
    1,
)
planner.write_text(text)

user_tools = Path('crates/osrs-editor/src/user_tools.rs')
text = user_tools.read_text()
needle = '#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]\npub enum UserToolKind {'
replacement = '#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]\n#[allow(\n    clippy::large_enum_variant,\n    reason = "Preset intentionally stores brush and spline settings inline to preserve the persisted user-tool data model"\n)]\npub enum UserToolKind {'
assert needle in text, 'UserToolKind declaration changed'
text = text.replace(needle, replacement, 1)
user_tools.write_text(text)
