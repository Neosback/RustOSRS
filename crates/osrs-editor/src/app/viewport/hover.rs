//! What the hover means for the active tool.

use super::*;
use crate::paint::Tool;

/// Whether hovering an object stands for the tile it stands on (and shows its outline): the
/// selecting tools in an object-picking mode, with no placement armed. Everything else (brushes,
/// tile mode, an armed prefab, NPC, object or site pick) reads the ground under the cursor.
pub(super) fn anchors_on_objects(app: &EditorApp) -> bool {
    let armed = app.prefabs.armed.is_some()
        || app.prefabs.build.choosing
        || app.npcs.arm != npc_runtime::Arm::None
        || app.placer.is_armed();
    anchors(
        app.settings.prefs.tool,
        app.settings.prefs.select_mode.objects(),
        armed,
    )
}

fn anchors(tool: Tool, objects_mode: bool, armed: bool) -> bool {
    !armed
        && match tool {
            Tool::Objects => true,
            Tool::Select => objects_mode,
            _ => false,
        }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_selecting_tools_in_an_object_mode_with_nothing_armed_anchor_on_objects() {
        assert!(anchors(Tool::Select, true, false));
        assert!(anchors(Tool::Objects, false, false));
        assert!(
            !anchors(Tool::Select, false, false),
            "tiles mode reads the ground"
        );
        assert!(
            !anchors(Tool::Select, true, true),
            "an armed placement reads the ground"
        );
        assert!(!anchors(Tool::Paint, true, false));
    }
}
