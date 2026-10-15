//! Export orchestration. Workers own immutable inputs; only current-job results enter the UI.

mod inspection;
mod review;

use super::EditorApp;
use crate::export_flow::{
    ExportMode, ExportSnapshot, ExportStamp, ExportVerificationReport, PreparedExport, prepare,
    publish,
};
use eframe::egui;
use osrs_cache::source::CacheSource;
use osrs_cache::verified_export::{CACHE_MANIFEST, ExportManifest, VerifiedExport};
use std::path::PathBuf;
use std::sync::{
    Arc,
    mpsc::{Receiver, TryRecvError},
};

pub(super) enum ExportEvent {
    Prepared(PreparedExport),
    Published(Result<ExportVerificationReport, String>),
    Inspected(Result<InspectionSource, String>),
}

pub(super) struct InspectionSource {
    source: CacheSource,
    path: PathBuf,
    manifest: ExportManifest,
}

struct ExportJob {
    stamp: ExportStamp,
    receiver: Receiver<ExportEvent>,
}

pub(super) struct ExportRuntime {
    epoch: u64,
    next: u64,
    active: Option<ExportStamp>,
    job: Option<ExportJob>,
    prepared: Option<Arc<PreparedExport>>,
    report: Option<ExportVerificationReport>,
    failure: Option<String>,
    mode: ExportMode,
    pub(super) open: bool,
    inspection: Option<Box<inspection::ExportInspection>>,
    pending_inspection: Option<InspectionSource>,
}

impl Default for ExportRuntime {
    fn default() -> Self {
        Self {
            epoch: 0,
            next: 0,
            active: None,
            job: None,
            prepared: None,
            report: None,
            failure: None,
            mode: ExportMode::CacheCopy,
            open: false,
            inspection: None,
            pending_inspection: None,
        }
    }
}

impl ExportRuntime {
    pub(in crate::app) fn reset_source(&mut self) {
        let epoch = self.epoch + 1;
        let next = self.next;
        *self = Self {
            epoch,
            next,
            ..Self::default()
        };
    }

    fn stamp(&mut self, edit_revision: u64) -> ExportStamp {
        self.next += 1;
        let stamp = ExportStamp {
            epoch: self.epoch,
            job: self.next,
            edit_revision,
        };
        self.active = Some(stamp);
        stamp
    }

    fn accepts(&self, stamp: ExportStamp) -> bool {
        stamp.epoch == self.epoch && self.active == Some(stamp)
    }

    fn spawn(&mut self, stamp: ExportStamp, work: impl FnOnce() -> ExportEvent + Send + 'static) {
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(work());
        });
        self.job = Some(ExportJob { stamp, receiver });
    }

    fn poll(&mut self) {
        let Some(job) = &self.job else { return };
        let stamp = job.stamp;
        let result = job.receiver.try_recv();
        if matches!(result, Err(TryRecvError::Empty)) {
            return;
        }
        self.job = None;
        if !self.accepts(stamp) {
            return;
        }
        match result {
            Ok(ExportEvent::Prepared(prepared)) => self.prepared = Some(Arc::new(prepared)),
            Ok(ExportEvent::Published(Ok(report))) => self.report = Some(report),
            Ok(ExportEvent::Inspected(Ok(source))) => self.pending_inspection = Some(source),
            Ok(ExportEvent::Published(Err(error)) | ExportEvent::Inspected(Err(error))) => {
                self.failure = Some(error)
            }
            Err(_) => {
                self.failure =
                    Some("Export worker stopped unexpectedly. Validate again to retry.".to_owned())
            }
        }
    }
}

impl EditorApp {
    pub(super) fn export_menu(&mut self, ui: &mut egui::Ui) {
        let changed =
            self.session.dirty_regions().next().is_some() || self.project.has_pending_exports();
        let enabled = changed && self.exports.job.is_none() && self.project.cache_build().is_some();
        for (label, mode) in [
            ("Export edited regions...", ExportMode::Loose),
            ("Export as cache copy...", ExportMode::CacheCopy),
        ] {
            if ui
                .add_enabled(enabled, egui::Button::new(label))
                .on_disabled_hover_text(
                    "Open a cache and edit the map first; wait for active export work to finish.",
                )
                .clicked()
            {
                ui.close();
                self.prepare_export(mode);
            }
        }
        if ui.button("Export review and report...").clicked() {
            ui.close();
            self.exports.open = true;
        }
        if ui
            .add_enabled(
                self.project.cache_build().is_some() && self.exports.job.is_none(),
                egui::Button::new("Inspect exported cache..."),
            )
            .clicked()
        {
            ui.close();
            if let Some(path) = rfd::FileDialog::new()
                .set_title("Choose an exported cache copy")
                .pick_folder()
            {
                self.inspect_export(path);
            }
        }
    }

    fn prepare_export(&mut self, mode: ExportMode) {
        let Some((_, profile)) = &self.project.cache_build() else {
            return;
        };
        let profile = profile.clone();
        let stamp = self.exports.stamp(self.session.edit_revision());
        let snapshot = ExportSnapshot {
            stamp,
            profile,
            source: CacheSource::Dir(self.project.cache_label().clone()),
            document: self.session.document().clone(),
            authored: self.session.dirty_regions().collect(),
            pending: self.project.export_pending(),
        };
        self.exports.prepared = None;
        self.exports.report = None;
        self.exports.failure = None;
        self.exports.mode = mode;
        self.exports.open = true;
        self.exports
            .spawn(stamp, move || ExportEvent::Prepared(prepare(snapshot)));
    }

    fn publish_export(&mut self) {
        let Some(prepared) = self.exports.prepared.clone() else {
            return;
        };
        if !prepared.can_publish()
            || prepared.stamp.edit_revision != self.session.edit_revision()
            || self.exports.job.is_some()
        {
            return;
        }
        let Some(out) = rfd::FileDialog::new()
            .set_title("Choose an empty folder for the verified export")
            .pick_folder()
        else {
            return;
        };
        let _ = self.publish_export_to(out);
    }

    fn publish_export_to(&mut self, out: PathBuf) -> Result<(), String> {
        let prepared = self
            .exports
            .prepared
            .clone()
            .ok_or("Validate the map before exporting.")?;
        if !prepared.can_publish()
            || prepared.stamp.edit_revision != self.session.edit_revision()
            || self.exports.job.is_some()
        {
            return Err("Validation is blocked, outdated, or still running. Validate again before exporting.".to_owned());
        }
        let mode = self.exports.mode;
        self.exports.failure = None;
        self.exports.report = None;
        let stamp = self.exports.stamp(prepared.stamp.edit_revision);
        self.exports.spawn(stamp, move || {
            ExportEvent::Published(publish(&prepared, mode, &out))
        });
        Ok(())
    }

    fn inspect_export(&mut self, path: PathBuf) {
        let original = self.project.cache_label().clone();
        let stamp = self.exports.stamp(self.session.edit_revision());
        self.exports.failure = None;
        self.exports.open = true;
        self.exports.spawn(stamp, move || {
            let result = (|| {
                let bytes = std::fs::read(path.join(CACHE_MANIFEST)).map_err(|e| {
                    format!("Read {CACHE_MANIFEST}: {e}. Choose a completed cache-copy export.")
                })?;
                let manifest: ExportManifest = serde_json::from_slice(&bytes).map_err(|e| {
                    format!("Unsupported or invalid export manifest: {e}. Export a new cache copy.")
                })?;
                let profiles =
                    osrs_cache::registry::bundled_profiles().map_err(|e| e.to_string())?;
                let verified = VerifiedExport::validate(&original, &path, &manifest, &profiles)
                    .map_err(|e| e.to_string())?;
                Ok(InspectionSource {
                    source: verified.source(),
                    path,
                    manifest,
                })
            })();
            ExportEvent::Inspected(result)
        });
    }

    pub(super) fn poll_export(&mut self) {
        self.exports.poll();
    }

    /// Recorded UI operations for repeatable native export smoke tests.
    pub(super) fn script_export(&mut self, action: &str, path: &str) -> Result<bool, String> {
        if self.exports.job.is_some() || self.project.cache_build().is_none() {
            return Ok(false);
        }
        if let Some(error) = &self.exports.failure {
            return Err(error.clone());
        }
        match action {
            "validate" => {
                self.prepare_export(ExportMode::CacheCopy);
                self.exports.open = false;
            }
            "write" if !path.is_empty() => self.publish_export_to(PathBuf::from(path))?,
            "inspect" if !path.is_empty() => self.inspect_export(PathBuf::from(path)),
            "review" => self.exports.open = true,
            _ => {
                return Err(
                    "Use export validate|review, export write PATH, or export inspect PATH."
                        .to_owned(),
                );
            }
        }
        Ok(true)
    }
}

#[cfg(test)]
mod settlement_tests;
#[cfg(test)]
mod tests;
