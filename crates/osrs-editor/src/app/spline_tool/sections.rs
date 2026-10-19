//! The Spline workflow as sections shared by the drawer, the side shelf and the quick HUD:
//! appearance, shape and terrain settings, preview status and actions. Pure widgets: they read a
//! view of the interaction state and return actions; the caller applies them to their owners.
use super::panel;
use crate::app::interaction_runtime::SplineStatus;
use crate::paint::SplineSettings;
use eframe::egui;

/// Which sections a surface shows. Narrow surfaces (the shelf) show less; the drawer shows all.
#[derive(Clone, Copy)]
pub(in crate::app) struct SplineSections {
    pub appearance: bool,
    pub shape: bool,
    pub help: bool,
    pub preview: bool,
    /// The AI helper: join the points the way a trained model would.
    pub ai: bool,
}

impl SplineSections {
    pub(in crate::app) const FULL: Self = Self {
        appearance: true,
        shape: true,
        help: true,
        preview: true,
        ai: true,
    };
    /// The quick HUD: everything needed to lay a path except the how-to.
    pub(in crate::app) const COMPACT: Self = Self {
        appearance: true,
        shape: true,
        help: false,
        preview: true,
        ai: false,
    };
}

pub(in crate::app) struct SplineView {
    /// Points placed so far.
    pub points: usize,
    pub status: SplineStatus,
    /// There is a point and an overlay to lay it with.
    pub can_lay: bool,
    /// The trained models the AI helper can use, and which one is chosen (none: hide it).
    pub ai: Option<AiView>,
}

pub(in crate::app) struct AiView {
    pub models: Vec<String>,
    pub selected: usize,
}

#[derive(Debug, PartialEq)]
pub(in crate::app) enum SplineAction {
    Lay,
    RemoveLast,
    Clear,
    CancelPlan,
    ResumePlan,
    CopyDetails,
    /// Replace the points with the route the chosen model prefers.
    AiRoute,
    AiModel(usize),
}

/// Human-readable text of a status, shared by every surface and the tests.
pub(in crate::app) fn status_text(status: SplineStatus) -> String {
    match status {
        SplineStatus::NeedsPoints => "Click points on the terrain".to_owned(),
        SplineStatus::NeedsOverlay => "Choose and enable an overlay".to_owned(),
        SplineStatus::Updating => "Planning the preview…".to_owned(),
        SplineStatus::Cancelled => "Planning cancelled".to_owned(),
        SplineStatus::Ready { tiles } => format!("Preview · {tiles} tiles"),
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
/// Order: appearance, shape and terrain settings, then the preview status and actions. Wide
/// panels put the first two and the last two side by side.
pub(in crate::app) fn show(
    ui: &mut egui::Ui,
    sections: SplineSections,
    settings: &mut SplineSettings,
    view: &SplineView,
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
                crate::theme::section_heading(ui, "Path");
                panel::settings(ui, settings);
            }
        };
    let appearance: AppearanceSlot<'_> = sections
        .appearance
        .then(|| Box::new(appearance) as Box<dyn FnOnce(&mut egui::Ui)>);
    if ui.available_width() >= TWO_COLUMNS && sections.preview {
        ui.columns(2, |columns| {
            setup(&mut columns[0], appearance, settings);
            preview_sections(&mut columns[1], sections, view, &mut actions);
        });
    } else {
        setup(ui, appearance, settings);
        if sections.preview {
            preview_sections(ui, sections, view, &mut actions);
        }
    }
    actions
}

fn preview_sections(
    ui: &mut egui::Ui,
    sections: SplineSections,
    view: &SplineView,
    actions: &mut Vec<SplineAction>,
) {
    crate::theme::section_heading(ui, "Preview");
    status_row(ui, view, actions);
    crate::theme::section_heading(ui, "Actions");
    action_row(ui, view, actions);
    if sections.ai
        && let Some(ai) = &view.ai
    {
        ai_row(ui, view, ai, actions);
    }
    if sections.help {
        egui::CollapsingHeader::new("How to draw the path")
            .id_salt("spline_how_to")
            .default_open(true)
            .show(ui, |ui| {
                for line in [
                    "Click: add a point (the tile's centre)",
                    "Enter: lay the path · Esc: cancel it",
                    "Backspace: remove the last point",
                    "The overlay comes from the Paint tool's Overlay",
                ] {
                    ui.label(line);
                }
            });
    }
}

fn status_row(ui: &mut egui::Ui, view: &SplineView, actions: &mut Vec<SplineAction>) {
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
}

fn action_row(ui: &mut egui::Ui, view: &SplineView, actions: &mut Vec<SplineAction>) {
    let any = view.points > 0;
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(view.can_lay, egui::Button::new("Lay path"))
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
}

/// The AI helper: pick a trained model and let it route between the points.
fn ai_row(ui: &mut egui::Ui, view: &SplineView, ai: &AiView, actions: &mut Vec<SplineAction>) {
    crate::theme::section_heading(ui, "AI helper");
    ui.weak("Click where the path starts and ends (and any place it must pass). The AI draws the way a model trained on real paths would: round water, along streets, to doors. Then you can still move its points before laying.");
    let selected = ai.models.get(ai.selected).cloned().unwrap_or_default();
    egui::ComboBox::from_id_salt("spline_ai_model")
        .selected_text(selected)
        .width(ui.available_width().min(260.0))
        .show_ui(ui, |ui| {
            for (index, name) in ai.models.iter().enumerate() {
                if ui.selectable_label(index == ai.selected, name).clicked() {
                    actions.push(SplineAction::AiModel(index));
                }
            }
        });
    if ui
        .add_enabled(view.points >= 2, egui::Button::new("Let the AI route it"))
        .on_hover_text(
            "Needs at least two points. Train your own model in the Workshop (Paths tab).",
        )
        .clicked()
    {
        actions.push(SplineAction::AiRoute);
    }
}

#[cfg(test)]
mod tests;
