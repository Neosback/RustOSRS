//! Development-only reference, fixture, and differential-verification tooling.
//!
//! Production crates must never depend on this crate. Reference source trees,
//! oracle adapters, fixture loaders, and regeneration tooling remain isolated
//! from runtime semantic crates.

pub mod comparator;
pub mod fixture;
pub mod inventory;
pub mod loader;
pub mod runner;
pub mod schema;
