//! Immutable export preparation, publication, and verification, independent of editor widgets.

mod output;
mod prepare;
#[cfg(test)]
pub(crate) mod test_support;

#[cfg(test)]
pub(crate) use output::publish_fixture;
pub(crate) use output::{ExportVerificationReport, publish};
pub(crate) use prepare::prepare;
#[cfg(test)]
pub(crate) use prepare::prepare_with;

use crate::session::{ExportChanges, RegionDiff};
use osrs_cache::source::CacheSource;
use osrs_core::coords::RegionCoord;
use osrs_edit::{MapDocument, MapPosition};
use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ExportStamp {
    pub epoch: u64,
    pub job: u64,
    pub edit_revision: u64,
}

/// Owns every authored input, including project regions not yet hydrated by streaming.
#[derive(Debug, Clone)]
pub(crate) struct ExportSnapshot {
    pub stamp: ExportStamp,
    pub source: CacheSource,
    pub profile: String,
    pub document: MapDocument,
    pub authored: BTreeSet<RegionCoord>,
    pub pending: Vec<RegionDiff>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExportMode {
    Loose,
    CacheCopy,
}

impl ExportMode {
    pub fn name(self) -> &'static str {
        match self {
            Self::Loose => "loose",
            Self::CacheCopy => "cache-copy",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IssueSeverity {
    Warning,
    Blocker,
}

#[derive(Debug, Clone)]
pub(crate) struct ExportIssue {
    pub severity: IssueSeverity,
    pub code: &'static str,
    pub region: Option<RegionCoord>,
    pub location: Option<MapPosition>,
    pub message: String,
    pub remedy: String,
}

impl ExportIssue {
    pub fn blocker(code: &'static str, message: impl Into<String>, remedy: &str) -> Self {
        Self {
            severity: IssueSeverity::Blocker,
            code,
            region: None,
            location: None,
            message: message.into(),
            remedy: remedy.to_owned(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct PreparedRegion {
    pub region: RegionCoord,
    pub group: u32,
    pub files: Vec<(u32, Vec<u8>)>,
    pub changes: ExportChanges,
    pub expected: MapDocument,
}

#[derive(Debug)]
pub(crate) struct PreparedExport {
    pub stamp: ExportStamp,
    pub source: CacheSource,
    pub profile: String,
    pub fingerprint: String,
    pub regions: Vec<PreparedRegion>,
    pub unchanged: usize,
    pub issues: Vec<ExportIssue>,
}

impl PreparedExport {
    pub fn can_publish(&self) -> bool {
        !self.regions.is_empty()
            && !self
                .issues
                .iter()
                .any(|i| i.severity == IssueSeverity::Blocker)
    }
}
