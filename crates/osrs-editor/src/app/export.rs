//! File > Export: write edited map squares as JS5 group containers, or as a whole cache copy.
//!
//! Export never modifies the source cache. *Loose* mode writes each edited square as
//! `5/<group>.dat` (a gzip container holding the original group's files with terrain and locations
//! replaced) next to a `manifest.json` recording the source cache identity and what changed.
//! *Cache copy* mode copies the cache folder into an empty destination and repacks the edited
//! squares into the copy (`osrs_cache::encode::repack_groups`), keeping each reference-table row
//! consistent, so the result is a cache other tools can open. Untouched squares are not written. Reading, diffing, encoding, and writing run on a worker thread against a snapshot
//! of the document, so the editor stays responsive and keeps its own history.

use super::EditorApp;
use crate::session::{ExportChanges, export_region};
use eframe::egui;
use osrs_cache::encode::{
    ContainerCompression, ContainerError, GroupUpdate, encode_container, pack_group, repack_groups,
};
use osrs_core::coords::RegionCoord;
use osrs_edit::MapDocument;
use osrs_world::SourceSquares;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError};
use web_time::Instant;

pub(super) struct ExportJob {
    receiver: Receiver<Result<ExportReport, String>>,
}

struct ExportReport {
    out_dir: PathBuf,
    written: usize,
    unchanged: usize,
    mode: ExportMode,
}

/// How an export lands on disk.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum ExportMode {
    /// `5/<group>.dat` files plus a manifest.
    Loose,
    /// A copy of the cache folder with the edited squares repacked into it.
    CacheCopy,
}

/// Replace files 0 (terrain) and 1 (locations) of a stored group and pack the result.
fn package_payload(
    original: &[(u32, Vec<u8>)],
    terrain: &[u8],
    locations: &[u8],
) -> Result<Vec<u8>, ContainerError> {
    let parts: Vec<&[u8]> = original
        .iter()
        .map(|(id, bytes)| match id {
            0 => terrain,
            1 => locations,
            _ => bytes.as_slice(),
        })
        .collect();
    pack_group(&parts)
}

/// `package_payload`, wrapped in a gzip container for writing as a loose file.
fn package_group(
    original: &[(u32, Vec<u8>)],
    terrain: &[u8],
    locations: &[u8],
) -> Result<Vec<u8>, ContainerError> {
    encode_container(
        package_payload(original, terrain, locations)?,
        ContainerCompression::Gzip,
        None,
    )
}

/// Copy every file of a cache folder into an empty `dest`.
fn copy_cache_folder(source: &Path, dest: &Path) -> Result<(), String> {
    let occupied = std::fs::read_dir(dest)
        .map_err(|error| format!("reading {}: {error}", dest.display()))?
        .next()
        .is_some();
    if occupied {
        return Err(format!(
            "{} is not empty; choose an empty folder",
            dest.display()
        ));
    }
    for entry in std::fs::read_dir(source).map_err(|error| format!("reading cache: {error}"))? {
        let entry = entry.map_err(|error| error.to_string())?;
        if entry.path().is_file() {
            std::fs::copy(entry.path(), dest.join(entry.file_name())).map_err(|error| {
                format!("copying {}: {error}", entry.file_name().to_string_lossy())
            })?;
        }
    }
    Ok(())
}

fn run_export(
    source: osrs_cache::source::CacheSource,
    out_dir: &Path,
    document: &MapDocument,
    regions: &[RegionCoord],
    mode: ExportMode,
) -> Result<ExportReport, String> {
    let source_dir = match &source {
        osrs_cache::source::CacheSource::Dir(dir) => Some(dir.clone()),
        _ => None,
    };
    let squares = SourceSquares::open(source).map_err(|error| error.to_string())?;
    std::fs::create_dir_all(out_dir.join("5"))
        .map_err(|error| format!("creating output: {error}"))?;
    if mode == ExportMode::CacheCopy {
        // Created above only for the loose layout; a cache copy is the folder itself.
        std::fs::remove_dir(out_dir.join("5")).map_err(|error| error.to_string())?;
    }

    let (mut entries, mut unchanged, mut updates) = (Vec::new(), 0, Vec::new());
    for &region in regions {
        let Some(square) = squares.read(region).map_err(|error| error.to_string())? else {
            unchanged += 1;
            continue;
        };
        let exported = export_region(&square.terrain, &square.locations, document)
            .map_err(|error| format!("region {},{}: {error:?}", region.x, region.y))?;
        let Some(exported) = exported else {
            unchanged += 1;
            continue;
        };
        let file = format!("5/{}.dat", square.group_id);
        match mode {
            ExportMode::Loose => {
                let bytes = package_group(&square.files, &exported.terrain, &exported.locations)
                    .map_err(|error| error.to_string())?;
                std::fs::write(out_dir.join(&file), bytes)
                    .map_err(|error| format!("writing {file}: {error}"))?;
            }
            ExportMode::CacheCopy => {
                let payload =
                    package_payload(&square.files, &exported.terrain, &exported.locations)
                        .map_err(|error| error.to_string())?;
                updates.push(GroupUpdate {
                    index_id: 5,
                    group_id: square.group_id,
                    payload,
                });
            }
        }
        entries.push(entry_json(region, square.group_id, &file, exported.changes));
    }
    if mode == ExportMode::CacheCopy && !updates.is_empty() {
        let source_dir = source_dir.ok_or("cache copy needs a cache folder as the source")?;
        copy_cache_folder(&source_dir, out_dir)?;
        repack_groups(out_dir, &updates).map_err(|error| format!("repacking: {error}"))?;
    }

    let manifest = json!({
        "format": "rustosrs-map-export-v1",
        "mode": if mode == ExportMode::Loose { "loose" } else { "cache-copy" },
        "note": "edited map squares only; the source cache is unchanged",
        "profile": squares.profile_id(),
        "cache_fingerprint": squares.fingerprint(),
        "regions": entries,
    });
    let text = serde_json::to_string_pretty(&manifest).map_err(|error| error.to_string())?;
    let manifest_name = match mode {
        ExportMode::Loose => "manifest.json",
        ExportMode::CacheCopy => "rustosrs-export.json",
    };
    std::fs::write(out_dir.join(manifest_name), text)
        .map_err(|error| format!("writing manifest: {error}"))?;
    Ok(ExportReport {
        out_dir: out_dir.to_path_buf(),
        written: entries.len(),
        unchanged,
        mode,
    })
}

fn entry_json(
    region: RegionCoord,
    group_id: u32,
    file: &str,
    changes: ExportChanges,
) -> serde_json::Value {
    json!({
        "region": [region.x, region.y],
        "index": 5,
        "group": group_id,
        "file": file,
        "tiles_changed": changes.tiles,
        "heights_changed": changes.heights,
        "objects_added": changes.objects_added,
        "objects_removed": changes.objects_removed,
    })
}

impl EditorApp {
    /// File menu entries. Disabled until an edit exists, or when the cache is not a folder.
    pub(super) fn export_menu(&mut self, ui: &mut egui::Ui) {
        let editable = self.session.dirty_regions().next().is_some();
        let enabled =
            editable && self.export_job.is_none() && !self.cache_dir.as_os_str().is_empty();
        let loose = ui
            .add_enabled(
                enabled,
                egui::Button::new(format!(
                    "{}  Export edited regions...",
                    egui_phosphor::regular::EXPORT
                )),
            )
            .on_disabled_hover_text("Edit the map first; export writes only changed squares");
        if loose.clicked() {
            ui.close();
            self.start_export(ExportMode::Loose);
        }
        let copy = ui
            .add_enabled(enabled, egui::Button::new("Export as cache copy..."))
            .on_hover_text("A copy of the cache folder with your edits repacked into it")
            .on_disabled_hover_text("Edit the map first");
        if copy.clicked() {
            ui.close();
            self.start_export(ExportMode::CacheCopy);
        }
    }

    fn start_export(&mut self, mode: ExportMode) {
        let title = match mode {
            ExportMode::Loose => "Choose an empty folder for the exported map squares",
            ExportMode::CacheCopy => "Choose an empty folder for the edited cache copy",
        };
        let Some(out_dir) = rfd::FileDialog::new().set_title(title).pick_folder() else {
            return;
        };
        let regions: Vec<RegionCoord> = self.session.dirty_regions().collect();
        let document = self.session.document().clone();
        let source = osrs_cache::source::CacheSource::Dir(self.cache_dir.clone());
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(run_export(source, &out_dir, &document, &regions, mode));
        });
        self.export_job = Some(ExportJob { receiver });
        self.notice = Some(("Exporting edited regions...".to_owned(), Instant::now()));
    }

    /// Report a finished export once per frame.
    pub(super) fn poll_export(&mut self) {
        let Some(job) = &self.export_job else {
            return;
        };
        let message = match job.receiver.try_recv() {
            Ok(Ok(report)) => format!(
                "Exported {} region{} {} {} ({} unchanged)",
                report.written,
                if report.written == 1 { "" } else { "s" },
                if report.mode == ExportMode::CacheCopy {
                    "into a cache copy at"
                } else {
                    "to"
                },
                report.out_dir.display(),
                report.unchanged
            ),
            Ok(Err(error)) => format!("Export failed: {error}"),
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => "Export stopped unexpectedly".to_owned(),
        };
        self.export_job = None;
        self.notice = Some((message, Instant::now()));
    }
}

#[cfg(test)]
mod tests;
