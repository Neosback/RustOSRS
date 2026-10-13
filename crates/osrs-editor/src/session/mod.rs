//! Persistent semantic editing session owned by the editor application layer.
//!
//! This module deliberately contains no UI, renderer, cache-codec, or world-worker state.
//! It owns authoritative semantic map state plus its undo/redo history and revision counter.

mod edit_session;

pub(crate) use edit_session::EditorSession;
