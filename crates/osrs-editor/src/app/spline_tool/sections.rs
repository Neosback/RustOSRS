//! The Spline workflow as sections shared by the drawer, the side shelf and the quick HUD:
//! appearance, shape, control points, preview status and actions. Pure widgets: they read a view
//! of the interaction state and return actions; the caller applies them to their owners.
use super::panel::{self, PointAction};
use crate::app::interaction_runtime::SplineStatus;
use crate::paint::SplineSettings;
use eframe::egui;

/// Which sections a surface shows. Narrow surfaces (the shelf) show less; the drawer shows all.
#[derive(Clone, Copy)]
pub(in crate::app) struct SplineSections {
    pub appearance: bool,
    pub shape: bool,
    pub points: bool,
    pub preview: bool,
}

impl SplineSections {
    pub(in crate::app) const FULL: Self = Self {
        appearance: true,
        shape: true,
        points: true,
        preview: true,
    };
    /// The quick HUD: everything needed to lay a path except the point list.
    pub(in crate::app) const COMPACT: Self = Self {
        appearance: true,
        shape: true,
        points: false,
        preview: true,
    };
}

/// What laying the path will also change besides the overlay (the armed Paint channels).
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub(in crate::app) struct AlsoApplies {
    pub height: Option<i32>,
    pub flags: Option<u8>,
}

pub(in crate::app) struct SplineView<'a> {
    pub points: &'a [crate::terrain_path::ControlPoint],
    pub selected: Option<usize>,
    pub status: SplineStatus,
    pub also: AlsoApplies,
}

#[derive(Debug, PartialEq)]
pub(in crate::app) enum SplineAction {
    Point(PointAction),
    Lay,
    RemoveLast,
    Clear,
    CancelPlan,
    ResumePlan,
    CopyDetails,
}

/// Laying needs a current, valid, non-empty preview: never a stale, cancelled or failed one.
pub(in crate::app) fn can_lay(status: SplineStatus) -> bool {
    matches!(status, SplineStatus::Ready { .. })
}

/// Human-readable text of a status, shared by every surface and the tests.
pub(in crate::app) fn status_text(status: SplineStatus) -> String {
    match status {
        SplineStatus::NeedsPoints => "Add at least two control points".to_owned(),
        SplineStatus::NeedsOverlay => "Choose and enable an overlay".to_owned(),
        SplineStatus::Updating => "Planning the preview…".to_owned(),
        SplineStatus::Cancelled => "Planning cancelled".to_owned(),
        SplineStatus::Ready { tiles, clipped: 0 } => format!("Ready · {tiles} tiles"),
        SplineStatus::Ready { tiles, clipped } => {
            format!("Ready · {tiles} tiles · {clipped} left alone (other overlays)")
        }
        SplineStatus::Empty => "The fitted path contains no tiles".to_owned(),
        SplineStatus::Failed(reason) => reason.to_owned(),
    }
}

/// The overlay chooser the app supplies, drawn once in whichever column holds Appearance.
type AppearanceSlot<'a> = Option<Box<dyn FnOnce(&mut egui::Ui) + 'a>>;

/// Panel width from which appearance/shape and preview/actions sit in two columns.
const TWO_COLUMNS: f32 = 640.0;

/// Draw the selected sections. `appearance` draws the overlay chooser, which needs the app's
/// palette; everything else is self-contained.
///
/// Order: appearance, shape, then the preview status and actions (so "Lay path" is never below the
/// fold of a short drawer), then the control points. Wide panels put the first two and the next
/// two side by side.
pub(in crate::app) fn show(
    ui: &mut egui::Ui,
    sections: SplineSections,
    settings: &mut SplineSettings,
    view: &SplineView<'_>,
    appearance: impl FnOnce(&mut egui::Ui),
) -> Vec<SplineAction> {
    let mut actions = Vec::new();
    let setup =
        |ui: &mut egui::Ui, appearance: AppearanceSlot<'_>, settings: &mut SplineSettings| {
            if let Some(appearance) = appearance {
                crate::theme::section_heading(ui, "Appearance");
                ui.horizontal_wrapped(appearance);
            }
            if sections.shape {
                crate::theme::section_heading(ui, "Shape");
                panel::settings(ui, settings);
            }
        };
    let appearance: AppearanceSlot<'_> = sections
        .appearance
        .then(|| Box::new(appearance) as Box<dyn FnOnce(&mut egui::Ui)>);
    if ui.available_width() >= TWO_COLUMNS && sections.preview {
        ui.columns(2, |columns| {
            setup(&mut columns[0], appearance, settings);
            preview_sections(&mut columns[1], view, &mut actions);
        });
    } else {
        setup(ui, appearance, settings);
        if sections.preview {
            preview_sections(ui, view, &mut actions);
        }
    }
    if sections.points
        && let Some(action) = panel::points(ui, view.points, view.selected)
    {
        actions.push(SplineAction::Point(action));
    }
    actions
}

fn preview_sections(ui: &mut egui::Ui, view: &SplineView<'_>, actions: &mut Vec<SplineAction>) {
    crate::theme::section_heading(ui, "Preview");
    status_row(ui, view, actions);
    crate::theme::section_heading(ui, "Actions");
    action_row(ui, view, actions);
}

fn status_row(ui: &mut egui::Ui, view: &SplineView<'_>, actions: &mut Vec<SplineAction>) {
    ui.horizontal_wrapped(|ui| {
        match view.status {
            SplineStatus::Updating => {
                ui.spinner();
                ui.weak(status_text(view.status));
                if ui
                    .small_button("Cancel")
                    .on_hover_text("Stop planning; the points stay")
                    .clicked()
                {
                    actions.push(SplineAction::CancelPlan);
                }
            }
            SplineStatus::Cancelled => {
                ui.weak(status_text(view.status));
                if ui.small_button("Plan again").clicked() {
                    actions.push(SplineAction::ResumePlan);
                }
            }
            SplineStatus::Ready { .. } => {
                ui.label(status_text(view.status));
            }
            SplineStatus::Failed(_) => {
                ui.colored_label(ui.visuals().error_fg_color, status_text(view.status));
            }
            _ => {
                ui.weak(status_text(view.status));
            }
        };
    });
    let mut also = Vec::new();
    if let Some(height) = view.also.height {
        also.push(format!("sets height {height}"));
    }
    if let Some(flags) = view.also.flags {
        also.push(format!("sets tile flags {flags}"));
    }
    if !also.is_empty() {
        ui.weak(format!(
            "Lay path also {} (armed in the Paint tool). The preview shows current heights.",
            also.join(" and ")
        ));
    }
}

fn action_row(ui: &mut egui::Ui, view: &SplineView<'_>, actions: &mut Vec<SplineAction>) {
    let any = !view.points.is_empty();
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(can_lay(view.status), egui::Button::new("Lay path"))
            .on_hover_text("Enter · one undo step")
            .clicked()
        {
            actions.push(SplineAction::Lay);
        }
        if ui
            .add_enabled(any, egui::Button::new("Remove last"))
            .on_hover_text("Backspace")
            .clicked()
        {
            actions.push(SplineAction::RemoveLast);
        }
        if ui
            .add_enabled(any, egui::Button::new("Clear"))
            .on_hover_text("Esc · cancel this path without changing the map")
            .clicked()
        {
            actions.push(SplineAction::Clear);
        }
        if ui
            .add_enabled(any, egui::Button::new("Copy details"))
            .clicked()
        {
            actions.push(SplineAction::CopyDetails);
        }
    });
    egui::CollapsingHeader::new("How to edit the path")
        .id_salt("spline_how_to")
        .show(ui, |ui| {
            for line in [
                "Click: add a point",
                "Drag: move a point",
                "Shift-click a segment: insert",
                "Cmd-click or Delete: remove a point",
                "Backspace: remove the last point",
                "Enter: lay the preview · Esc: cancel",
            ] {
                ui.label(line);
            }
        });
}

#[cfg(test)]
mod tests;
