//! OSRS cache transport and revision-aware decoding boundary.
//!
//! M1 is evaluating a private `rune-fs` transport layer while RustOSRS retains
//! ownership of all revision-aware semantic decoders. The spike remains private
//! until the M1 dependency ADR accepts or rejects it.

#[allow(dead_code)]
mod transport;
