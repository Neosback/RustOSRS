//! Development-only reference, fixture, and differential-verification tooling.
//!
//! Production crates must never depend on this crate. M0 establishes that one-way
//! boundary before oracle adapters and substantive fixture families are implemented.

pub mod fixture;
