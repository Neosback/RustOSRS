//! File > Export: write edited map squares as JS5 group containers.
//!
//! Export never modifies the source cache. Each edited square becomes `5/<group>.dat` (a gzip
//! container holding the original group's files with terrain and locations replaced) next to a
//! `manifest.json` recording the source cache identity and what changed. Untouched squares are
//! not written. Reading, diffing, encoding, and writing run on a worker thread against a snapshot
//! of the document, so the editor stays responsive and keeps its own history.

use super::EditorApp;
use crate::session::{ExportChanges, export_region};
use eframe::egui;
use osrs_cache::encode::{ContainerCompression, ContainerError, encode_container, pack_group};
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
}

/// Replace files 0 (terrain) and 1 (locations) of a stored group and wrap it for writing.
fn package_group(
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
    encode_container(pack_group(&parts)?, ContainerCompression::Gzip, None)
}

fn run_export(
    source: osrs_cache::source::CacheSource,
    out_dir: &Path,
    document: &MapDocument,
    regions: &[RegionCoord],
) -> Result<ExportReport, String> {
    let squares = SourceSquares::open(source).map_err(|error| error.to_string())?;
    let group_dir = out_dir.join("5");
    std::fs::create_dir_all(&group_dir).map_err(|error| format!("creating output: {error}"))?;

    let (mut entries, mut unchanged) = (Vec::new(), 0);
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
        let bytes = package_group(&square.files, &exported.terrain, &exported.locations)
            .map_err(|error| error.to_string())?;
        let file = format!("5/{}.dat", square.group_id);
        std::fs::write(out_dir.join(&file), bytes)
            .map_err(|error| format!("writing {file}: {error}"))?;
        entries.push(entry_json(region, square.group_id, &file, exported.changes));
    }

    let manifest = json!({
        "format": "rustosrs-map-export-v1",
        "note": "gzip JS5 group containers for edited map squares only; the source cache is unchanged",
        "profile": squares.profile_id(),
        "cache_fingerprint": squares.fingerprint(),
        "regions": entries,
    });
    let text = serde_json::to_string_pretty(&manifest).map_err(|error| error.to_string())?;
    std::fs::write(out_dir.join("manifest.json"), text)
        .map_err(|error| format!("writing manifest: {error}"))?;
    Ok(ExportReport {
        out_dir: out_dir.to_path_buf(),
        written: entries.len(),
        unchanged,
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
    /// File menu entry. Disabled until an edit exists, or when the cache is not a folder.
    pub(super) fn export_menu(&mut self, ui: &mut egui::Ui) {
        let editable = self.session.dirty_regions().next().is_some();
        let enabled =
            editable && self.export_job.is_none() && !self.cache_dir.as_os_str().is_empty();
        let response = ui
            .add_enabled(
                enabled,
                egui::Button::new(format!(
                    "{}  Export edited regions...",
                    egui_phosphor::regular::EXPORT
                )),
            )
            .on_disabled_hover_text("Edit the map first; export writes only changed squares");
        if response.clicked() {
            ui.close();
            self.start_export();
        }
    }

    fn start_export(&mut self) {
        let Some(out_dir) = rfd::FileDialog::new()
            .set_title("Choose an empty folder for the exported map squares")
            .pick_folder()
        else {
            return;
        };
        let regions: Vec<RegionCoord> = self.session.dirty_regions().collect();
        let document = self.session.document().clone();
        let source = osrs_cache::source::CacheSource::Dir(self.cache_dir.clone());
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(run_export(source, &out_dir, &document, &regions));
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
                "Exported {} region{} to {} ({} unchanged)",
                report.written,
                if report.written == 1 { "" } else { "s" },
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
mod tests {
    use super::*;
    use osrs_cache::encode::{decode_container, pack_group};

    #[test]
    fn packaging_replaces_only_the_terrain_and_location_files() {
        let original = vec![
            (0, vec![1, 1, 1]),
            (1, vec![2, 2]),
            (2, vec![9, 9, 9, 9]),
            (3, vec![]),
        ];
        let container =
            package_group(&original, &[7; 5], &[8]).unwrap_or_else(|e| unreachable!("{e}"));
        let payload = decode_container(container).unwrap_or_else(|e| unreachable!("{e}"));
        let expected = pack_group(&[&[7; 5], &[8], &[9, 9, 9, 9], &[]])
            .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(payload, expected);
    }

    /// Needs the pinned cache (`RUSTOSRS_TARGET_CACHE_DIR`); skipped when it is not configured.
    #[test]
    fn exporting_a_real_edit_writes_a_container_that_decodes_back_to_the_edit() {
        use osrs_cache::decode::{DecoderContext, decode_terrain};
        use osrs_core::coords::{MapTile, SourcePlane};
        use osrs_core::ids::FloorUnderlayId;
        use osrs_edit::{MapOp, MapPatch, MapPosition, TerrainTile};

        let Some(dir) = std::env::var_os("RUSTOSRS_TARGET_CACHE_DIR") else {
            return;
        };
        let source = || osrs_cache::source::CacheSource::Dir(PathBuf::from(&dir));
        let squares = SourceSquares::open(source()).unwrap_or_else(|e| unreachable!("{e}"));
        let region = RegionCoord::new(50, 50);
        let square = squares
            .read(region)
            .unwrap_or_else(|e| unreachable!("{e}"))
            .unwrap_or_else(|| unreachable!("square 50,50 exists"));
        assert_eq!(
            square.files.len(),
            5,
            "build-241 map groups hold five files"
        );

        // The loaded document of a freshly streamed region, then one painted tile.
        let mut document = crate::session::baseline_document(&square.terrain, &square.locations)
            .unwrap_or_else(|e| unreachable!("{e:?}"));
        let plane = SourcePlane::new(0).unwrap_or_else(|| unreachable!("plane zero"));
        let position = MapPosition::new(MapTile::new(3210, 3210), plane);
        let before = document.terrain_at(position);
        let after = TerrainTile {
            underlay: Some(FloorUnderlayId::new(5)),
            ..before
        };
        document
            .apply_patch(&MapPatch::new(vec![MapOp::SetTerrain {
                coord: position,
                before,
                after,
            }]))
            .unwrap_or_else(|e| unreachable!("{e:?}"));

        let out = std::env::temp_dir().join(format!("rustosrs-export-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        let report = run_export(source(), &out, &document, &[region])
            .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!((report.written, report.unchanged), (1, 0));

        let file = out.join(format!("5/{}.dat", square.group_id));
        let container = std::fs::read(&file).unwrap_or_else(|e| unreachable!("{e}"));
        let payload = decode_container(container).unwrap_or_else(|e| unreachable!("{e}"));
        let files =
            osrs_cache::encode::split_group(&payload, 5).unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(files[2..], square.files[2..], "files 2..=4 are preserved");

        let profiles =
            osrs_cache::registry::bundled_profiles().unwrap_or_else(|e| unreachable!("{e:?}"));
        let profile = profiles
            .iter()
            .find(|p| p.id == squares.profile_id())
            .unwrap_or_else(|| unreachable!("profile"));
        let context =
            DecoderContext::from_profile(profile).unwrap_or_else(|e| unreachable!("{e:?}"));
        let decoded = decode_terrain(
            &files[0].1,
            &context,
            &osrs_cache::decode::ArchiveFileProvenance::new(5, square.group_id, Some(0)),
            region,
        )
        .unwrap_or_else(|e| unreachable!("{e:?}"));
        let index = (10 * 64) + 10;
        assert_eq!(decoded.tiles()[index].underlay_id, 5);
        let manifest = std::fs::read_to_string(out.join("manifest.json")).unwrap_or_default();
        assert!(manifest.contains("rustosrs-map-export-v1") && manifest.contains("tiles_changed"));
        let _ = std::fs::remove_dir_all(&out);
    }
}
