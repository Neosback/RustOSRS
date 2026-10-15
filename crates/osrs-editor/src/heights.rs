//! Editor-facing names for the canonical headless Height tool.
//! Settings serialization and sculpt arithmetic are owned by `osrs-edit`.

pub use osrs_edit::height::*;
pub(crate) mod commit {
    pub(crate) use osrs_edit::{HeightStrokeSpec, build_terrain_height_patch};
}
