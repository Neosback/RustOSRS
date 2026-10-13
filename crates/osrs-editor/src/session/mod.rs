//! Persistent semantic editing session owned by the editor application layer.
//!
//! The authoritative state and history live here. Infrastructure-specific decoded region data is
//! adapted at the module boundary before it enters the pure `osrs-edit` domain.

mod edit_session;
mod export;
mod region_ingest;

pub(crate) use edit_session::EditorSession;
#[cfg(test)]
pub(crate) use export::baseline_document;
pub(crate) use export::{ExportChanges, export_region};
pub(crate) use region_ingest::adapt_region_map;
