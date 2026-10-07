//! OSRS cache transport, target-profile, and revision-aware decoding boundary.
//!
//! M1 accepts a private `rune-fs` transport layer while RustOSRS retains
//! ownership of all revision-aware semantic decoders. The transport stays
//! private; `TargetProfile` is the validated cache/revision input contract for
//! later decoding milestones.

pub mod profile;

#[allow(dead_code)]
mod transport;
