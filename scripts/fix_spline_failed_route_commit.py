from pathlib import Path

path = Path('crates/osrs-editor/src/app/spline_tool.rs')
text = path.read_text()

anchor = '''fn distance(a: (f32, f32), b: (f32, f32)) -> f32 {\n'''
helper = '''fn commit_blocker(warnings: &[PathWarning]) -> Option<&PathWarning> {\n    warnings.iter().find(|warning| {\n        matches!(warning.severity, Severity::Error) || warning.code == "ROUTE_BLOCKED"\n    })\n}\n\n'''
if anchor not in text:
    raise SystemExit('distance anchor not found')
text = text.replace(anchor, helper + anchor, 1)

old = '''        let Some(preview) = self.spline.preview.take() else {\n            return;\n        };\n        if preview.paints.is_empty() {\n'''
new = '''        let Some(preview) = self.spline.preview.take() else {\n            return;\n        };\n        let blocked_message = commit_blocker(&preview.warnings).map(|warning| match warning.code {\n            "ROUTE_BLOCKED" => {\n                "The path could not route around an obstacle. Move the path or increase the route corridor before laying it.".to_owned()\n            }\n            _ => format!("The path was not laid: {}", warning.message),\n        });\n        if let Some(message) = blocked_message {\n            self.spline.preview = Some(preview);\n            self.notice = Some((message, web_time::Instant::now()));\n            return;\n        }\n        if preview.paints.is_empty() {\n'''
if old not in text:
    raise SystemExit('commit anchor not found')
text = text.replace(old, new, 1)

text += r'''

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_main_route_blocks_path_commit() {
        let warnings = [PathWarning::general(
            Severity::Warning,
            "ROUTE_BLOCKED",
            "No route exists inside the configured corridor",
        )];
        assert_eq!(
            commit_blocker(&warnings).map(|warning| warning.code),
            Some("ROUTE_BLOCKED")
        );
    }

    #[test]
    fn planner_errors_block_path_commit() {
        let warnings = [PathWarning::general(
            Severity::Error,
            "INVALID_EDGE",
            "Missing path data",
        )];
        assert!(commit_blocker(&warnings).is_some());
    }

    #[test]
    fn diagnostic_warnings_do_not_block_path_commit() {
        let warnings = [
            PathWarning::general(Severity::Info, "REROUTED", "Went round obstacles"),
            PathWarning::general(
                Severity::Warning,
                "EXISTING_OVERLAY",
                "Existing overlay was preserved",
            ),
            PathWarning::general(
                Severity::Warning,
                "NETWORK_CONNECT_BLOCKED",
                "An optional network branch could not connect",
            ),
        ];
        assert!(commit_blocker(&warnings).is_none());
    }
}
'''

path.write_text(text)
