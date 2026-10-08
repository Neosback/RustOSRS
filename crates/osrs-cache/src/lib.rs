//! OSRS cache transport, target-profile, and revision-aware decoding boundary.
//!
//! `rune-fs` remains a private read-only transport dependency. RustOSRS owns
//! target validation, provenance, binary decoding, and all revision-aware
//! semantic codecs exposed by this crate.

pub mod decode;
pub mod profile;
pub mod transport;
