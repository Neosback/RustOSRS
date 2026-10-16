//! Persistent semantic editing session owned by the editor application layer.
//!
//! The authoritative state and history live here. Infrastructure-specific decoded region data is
//! adapted at the module boundary before it enters the pure `osrs-edit` domain.

mod appearance;
mod appearance_support;
pub(crate) use appearance_support::validate_appearance_pack;
#[cfg(test)]
mod appearance_tests;
mod edit_session;
#[cfg(not(target_arch = "wasm32"))]
mod export;
#[cfg(not(target_arch = "wasm32"))]
mod project_file;
#[cfg(not(target_arch = "wasm32"))]
mod region_diff;
mod region_ingest;

pub(crate) use edit_session::{
    EditCheckpoint, EditCheckpointError, EditorSession, RegionSyncOutcome,
};
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use export::{
    ExportChanges, ExportError, HeightConversion, HeightExportPolicy, export_region_with_policy,
};
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use project_file::{PROJECT_FORMAT, ProjectFile};
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use region_diff::baseline_document;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use region_diff::{RegionDiff, diff_region};
pub(crate) use region_ingest::adapt_region_map;
