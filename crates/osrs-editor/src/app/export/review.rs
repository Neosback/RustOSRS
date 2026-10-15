//! Persistent preflight/results dialog. Drawing returns actions; orchestration executes afterward.
use super::*;
use crate::export_flow::{ExportIssue, IssueSeverity};
use crate::hud::Destination;

enum ReviewAction {
    Refresh,
    Publish,
    Focus(Destination),
    Inspect(PathBuf),
}

impl EditorApp {
    pub(in crate::app) fn export_modal_open(&self) -> bool {
        self.exports.open
    }

    pub(in crate::app) fn export_window(&mut self, ctx: &egui::Context) {
        if !self.exports.open {
            return;
        }
        let revision = self.session.edit_revision();
        let (action, closed) = super::super::modal::Modal::new("export-review", "Export review and verification")
            .size(730.0, 470.0).show(ctx, |ui| {
                let mut action = None;
                if self.exports.job.is_some() {
                    ui.horizontal(|ui| { ui.spinner(); ui.label("Reading and verifying the captured map..."); });
                    ui.label("You can close this review and continue editing. The result stays tied to the captured revision.");
                }
                if let Some(error) = &self.exports.failure {
                    ui.colored_label(ui.visuals().error_fg_color, format!("Export did not complete: {error}"));
                    ui.label("Correct the reported problem, then validate or export again. The original cache is unchanged.");
                }
                egui::ScrollArea::vertical().id_salt("export-details").max_height(350.0).show(ui, |ui| {
                    if let Some(prepared) = &self.exports.prepared {
                        ui.strong(format!("{} · captured edit revision {}", prepared.profile, prepared.stamp.edit_revision));
                        ui.label(format!("Source fingerprint: {}", prepared.fingerprint));
                        let outdated = prepared.stamp.edit_revision != revision;
                        if outdated { ui.colored_label(ui.visuals().warn_fg_color, "Outdated: the map changed after validation. Validate again before exporting."); }
                        ui.label(format!("{} changed regions · {} unchanged", prepared.regions.len(), prepared.unchanged));
                        egui::Grid::new("export-regions").striped(true).show(ui, |ui| {
                            for label in ["Region", "Tiles", "Heights", "Objects + / −"] { ui.strong(label); }
                            ui.end_row();
                            for region in &prepared.regions {
                                if ui.link(format!("{}, {}", region.region.x, region.region.y)).clicked() {
                                    action = Some(ReviewAction::Focus(Destination::Region { x: region.region.x, y: region.region.y }));
                                }
                                ui.label(region.changes.tiles.to_string());
                                ui.label(region.changes.heights.to_string());
                                ui.label(format!("{} / {}", region.changes.objects_added, region.changes.objects_removed));
                                ui.end_row();
                            }
                        });
                        for issue in &prepared.issues {
                            if let Some(destination) = draw_issue(ui, issue) { action = Some(ReviewAction::Focus(destination)); }
                        }
                    } else if self.exports.job.is_none() && self.exports.report.is_none() {
                        ui.label("Validate the current project to review every authored region, including unloaded project changes.");
                    }
                    if let Some(report) = &self.exports.report {
                        ui.separator();
                        ui.strong(format!("Verified export · revision {}", report.stamp.edit_revision));
                        ui.label(report.out_dir.display().to_string());
                        ui.label(format!("{} changed regions verified; {} unchanged regions omitted.", report.manifest.regions.len(), report.unchanged));
                        ui.label("Written streams match captured terrain, resolved heights, object placements, and preserved group files.");
                        if report.manifest.mode == "cache-copy" {
                            ui.label("Original logical cache content and reference-table updates verified.");
                            if ui.button("Inspect exported cache").clicked() && self.exports.job.is_none() {
                                action = Some(ReviewAction::Inspect(report.out_dir.clone()));
                            }
                        }
                        if report.stamp.edit_revision != revision {
                            ui.label("This export represents an earlier revision. Newer edits remain in your authoring project.");
                        }
                    }
                });
                ui.separator();
                ui.horizontal(|ui| {
                    let idle = self.exports.job.is_none();
                    if ui.add_enabled(idle && self.project.cache_build().is_some(), egui::Button::new("Validate current map")).clicked() { action = Some(ReviewAction::Refresh); }
                    let ready = idle && self.exports.prepared.as_ref().is_some_and(|p| p.can_publish() && p.stamp.edit_revision == revision);
                    if ui.add_enabled(ready, egui::Button::new(format!("Export {}...", self.exports.mode.name()))).clicked() { action = Some(ReviewAction::Publish); }
                });
                action
            });
        if closed {
            self.exports.open = false;
        }
        match action {
            Some(ReviewAction::Refresh) => self.prepare_export(self.exports.mode),
            Some(ReviewAction::Publish) => self.publish_export(),
            Some(ReviewAction::Focus(destination)) => {
                self.exports.open = false;
                self.teleport_to(destination);
            }
            Some(ReviewAction::Inspect(path)) => self.inspect_export(path),
            None => {}
        }
    }
}

fn draw_issue(ui: &mut egui::Ui, issue: &ExportIssue) -> Option<Destination> {
    let color = if issue.severity == IssueSeverity::Blocker {
        ui.visuals().error_fg_color
    } else {
        ui.visuals().warn_fg_color
    };
    let label = if issue.severity == IssueSeverity::Blocker {
        "Blocker"
    } else {
        "Warning"
    };
    ui.colored_label(
        color,
        format!("{label} · {}: {}", issue.code, issue.message),
    );
    ui.label(&issue.remedy);
    let destination = issue
        .location
        .map(|p| Destination::Tile {
            x: p.tile.x,
            y: p.tile.y,
            plane: p.plane.index().get(),
        })
        .or_else(|| issue.region.map(|r| Destination::Region { x: r.x, y: r.y }));
    destination.filter(|_| ui.link("Show affected location").clicked())
}
