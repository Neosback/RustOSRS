//! Immutable save snapshots and atomic file publication; no UI or live document mutation.
use super::*;
use crate::session::{ProjectFile, diff_region};
use osrs_edit::MapDocument;
use osrs_world::SourceSquares;
use std::{collections::BTreeSet, sync::mpsc::TryRecvError};
/// Build the project file contents for `document`, keeping unapplied `pending` diffs for regions
/// that are not being re-diffed. Pure apart from reading the source cache.
fn build_project(
    source: CacheSource,
    document: &MapDocument,
    dirty: &[RegionCoord],
    pending: &[RegionDiff],
) -> Result<ProjectFile, String> {
    let squares = SourceSquares::open(source).map_err(|error| error.to_string())?;
    let mut diffs = Vec::new();
    for &region in dirty {
        let square = squares
            .read(region)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| {
                format!(
                    "Cannot save authored region {},{}: its original map data is missing.",
                    region.x, region.y
                )
            })?;
        let diff = diff_region(&square.terrain, &square.locations, document)
            .map_err(|error| format!("region {},{}: {error:?}", region.x, region.y))?;
        diffs.push(diff);
    }
    let covered: BTreeSet<RegionCoord> = dirty.iter().copied().collect();
    diffs.extend(
        pending
            .iter()
            .filter(|diff| !covered.contains(&diff.region))
            .cloned(),
    );
    diffs.sort_by_key(|diff| (diff.region.x, diff.region.y));
    Ok(ProjectFile::from_diffs(squares.profile_id(), &diffs))
}

/// Write `text` next to `path` first, so a crash never leaves a half-written project.
fn write_atomically(path: &Path, text: &str) -> Result<(), String> {
    let temp = path.with_extension("tmp");
    std::fs::write(&temp, text).map_err(|error| format!("writing {}: {error}", temp.display()))?;
    std::fs::rename(&temp, path).map_err(|error| format!("saving {}: {error}", path.display()))
}

impl ProjectRuntime {
    pub(in crate::app) fn start_save(&mut self, path: PathBuf, session: &EditRuntime) {
        debug_assert!(!self.save_running());
        self.persistence.path = Some(path.clone());
        self.persistence.saving_revision = session.edit_revision();
        let source = self.source.clone();
        let document = session.document().clone();
        let dirty: Vec<_> = session.dirty_regions().collect();
        let pending = self.export_pending();
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let result = build_project(source, &document, &dirty, &pending).and_then(|file| {
                write_atomically(&path, &file.to_json())?;
                Ok((path, file.regions.len()))
            });
            let _ = sender.send(result);
        });
        self.persistence.save = Some(receiver);
    }
    pub(in crate::app) fn poll_save(&mut self) -> Option<Result<(PathBuf, usize), String>> {
        let receiver = self.persistence.save.as_ref()?;
        let result = match receiver.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => Err("Save stopped unexpectedly".to_owned()),
        };
        self.persistence.save = None;
        if result.is_ok() {
            self.persistence.saved_edit_revision = self.persistence.saving_revision;
        } else {
            self.persistence.close_when_saved = false;
            self.persistence.transition_after_save = false;
            self.persistence.transition_prompt = self.persistence.transition.is_some();
        }
        Some(result)
    }
}
