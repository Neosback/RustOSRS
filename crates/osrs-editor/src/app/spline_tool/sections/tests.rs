use super::*;

const STATUSES: [SplineStatus; 7] = [
    SplineStatus::NeedsPoints,
    SplineStatus::NeedsOverlay,
    SplineStatus::Updating,
    SplineStatus::Cancelled,
    SplineStatus::Ready { tiles: 128 },
    SplineStatus::Empty,
    SplineStatus::Failed("Preview worker stopped; retrying"),
];

/// Run one frame of `show` at a panel width and display scale; returns the actions and the
/// rectangle the content used, with the available right edge for overflow checks.
fn run(
    width: f32,
    scale: f32,
    sections: SplineSections,
    status: SplineStatus,
    points: usize,
    smooth_height: bool,
) -> (Vec<SplineAction>, egui::Rect, f32) {
    let ctx = crate::theme::layout_test_context(scale);
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(width, 900.0),
        )),
        ..Default::default()
    };
    let mut settings = SplineSettings {
        smooth_height,
        ..SplineSettings::default()
    };
    let mut result = (Vec::new(), egui::Rect::NOTHING, 0.0);
    let mut output = ctx.run_ui(input, |ui| {
        let right = ui.max_rect().right();
        let view = SplineView {
            points,
            status,
            can_lay: points > 0,
        };
        let scope = ui.scope(|ui| {
            show(ui, sections, &mut settings, &view, |ui| {
                ui.label("Overlay #1");
                ui.checkbox(&mut true, "Paint overlay");
            })
        });
        result = (scope.inner, scope.response.rect, right);
    });
    output.textures_delta.clear();
    result
}

#[test]
fn every_section_fits_every_panel_width_and_scale_for_every_status() {
    for width in [240.0, 320.0, 480.0, 700.0, 1100.0] {
        for scale in [1.0, 1.5, 2.0] {
            for status in STATUSES {
                for points in [0, 3] {
                    for smooth_height in [false, true] {
                        let (_, used, right) = run(
                            width,
                            scale,
                            SplineSections::FULL,
                            status,
                            points,
                            smooth_height,
                        );
                        assert!(
                            used.right() <= right + 0.5,
                            "width {width}, scale {scale}, {status:?}, {points} points, height {smooth_height}: {used:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn the_shelf_width_still_fits_the_compact_sections() {
    for scale in [1.0, 2.0] {
        for smooth_height in [false, true] {
            let (_, used, right) = run(
                172.0,
                scale,
                SplineSections::COMPACT,
                STATUSES[4],
                3,
                smooth_height,
            );
            assert!(
                used.right() <= right + 0.5,
                "scale {scale}, height {smooth_height}: {used:?}"
            );
        }
    }
}

#[test]
fn status_text_names_every_state_distinctly() {
    let texts: std::collections::BTreeSet<_> = STATUSES.iter().map(|s| status_text(*s)).collect();
    assert_eq!(texts.len(), STATUSES.len());
}

#[test]
fn nothing_is_emitted_without_interaction() {
    for status in STATUSES {
        let (actions, ..) = run(320.0, 1.0, SplineSections::FULL, status, 3, false);
        assert!(actions.is_empty(), "{status:?}: {actions:?}");
    }
}
