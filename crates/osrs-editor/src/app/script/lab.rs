//! Host controls while the Lab owns semantic edits. Ordinary editor scripts pause at this boundary.
use super::*;
/// Lab scripts may navigate/replay, but may never edit the temporary scene projection.
pub(in crate::app) fn controls(app: &mut EditorApp, ctx: &egui::Context) {
    app.script.lab_controls_only = true;
    app.script_frame(ctx);
    app.script.lab_controls_only = false;
}
pub(super) fn allows(line: &str) -> bool {
    let mut words = line.split_whitespace();
    match words.next() {
        Some("wait" | "sleep" | "at" | "status" | "mark") => true,
        Some("prefab") => words.next() == Some("studio"),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lab_controls_exclude_all_direct_projection_edits() {
        for line in [
            "paint 1 1",
            "place 10 1 1",
            "move 1 0",
            "undo",
            "prefab place test 1 1",
            "prefab capture test 0 0 2 2",
            "project open map.json",
        ] {
            assert!(!allows(line), "{line}");
        }
        for line in [
            "sleep 2",
            "wait 30",
            "prefab studio demo",
            "prefab studio play",
            "prefab studio exit",
            "prefab studio report",
        ] {
            assert!(allows(line), "{line}");
        }
    }
}
