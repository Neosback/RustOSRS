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

fn points(count: usize) -> Vec<(f32, f32)> {
    (0..count)
        .map(|i| (3200.5 + i as f32 * 7.25, 3210.5 + i as f32 * 3.5))
        .collect()
}

/// Run one frame of `show` at a panel width and display scale; returns the actions and the
/// rectangle the content used, with the available right edge for overflow checks.
fn run(
    width: f32,
    scale: f32,
    sections: SplineSections,
    status: SplineStatus,
    point_count: usize,
    click: Option<egui::Pos2>,
) -> (Vec<SplineAction>, egui::Rect, f32) {
    let ctx = crate::theme::layout_test_context(scale);
    let input = |events: Vec<egui::Event>| egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(width, 900.0),
        )),
        events,
        ..Default::default()
    };
    let pts = points(point_count);
    let mut settings = SplineSettings::default();
    let mut result = (Vec::new(), egui::Rect::NOTHING, 0.0);
    let mut frame = |events: Vec<egui::Event>,
                     result: &mut (Vec<SplineAction>, egui::Rect, f32)| {
        let mut output = ctx.run_ui(input(events), |ui| {
            let right = ui.max_rect().right();
            let view = SplineView {
                points: &pts,
                selected: Some(0),
                status,
                also: AlsoApplies {
                    height: Some(-240),
                    flags: Some(2),
                },
            };
            let scope = ui.scope(|ui| {
                show(ui, sections, &mut settings, &view, |ui| {
                    ui.label("Overlay #1");
                    ui.checkbox(&mut true, "Paint overlay");
                })
            });
            *result = (scope.inner, scope.response.rect, right);
        });
        output.textures_delta.clear();
    };
    frame(Vec::new(), &mut result);
    if let Some(pos) = click {
        let press = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(vec![egui::Event::PointerMoved(pos)], &mut result);
        frame(vec![press(true)], &mut result);
        frame(vec![press(false)], &mut result);
    }
    result
}

#[test]
fn every_section_fits_every_panel_width_and_scale_for_every_status() {
    for width in [240.0, 320.0, 480.0, 700.0, 1100.0] {
        for scale in [1.0, 1.5, 2.0] {
            for status in STATUSES {
                for count in [0, 2, 12] {
                    let (_, used, right) =
                        run(width, scale, SplineSections::FULL, status, count, None);
                    assert!(
                        used.right() <= right + 0.5,
                        "width {width}, scale {scale}, {status:?}, {count} points: {used:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn the_shelf_width_still_fits_the_compact_sections() {
    for scale in [1.0, 2.0] {
        let (_, used, right) = run(172.0, scale, SplineSections::COMPACT, STATUSES[4], 3, None);
        assert!(used.right() <= right + 0.5, "scale {scale}: {used:?}");
    }
}

#[test]
fn status_text_names_every_state_distinctly() {
    let texts: std::collections::BTreeSet<_> = STATUSES.iter().map(|s| status_text(*s)).collect();
    assert_eq!(texts.len(), STATUSES.len());
}

#[test]
fn lay_is_offered_only_for_a_ready_preview() {
    for status in STATUSES {
        assert_eq!(
            can_lay(status),
            matches!(status, SplineStatus::Ready { .. }),
            "{status:?}"
        );
    }
}

#[test]
fn nothing_is_emitted_without_interaction() {
    for status in STATUSES {
        let (actions, ..) = run(320.0, 1.0, SplineSections::FULL, status, 3, None);
        assert!(actions.is_empty(), "{status:?}: {actions:?}");
    }
}
