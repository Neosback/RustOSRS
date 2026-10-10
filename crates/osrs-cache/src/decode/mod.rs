//! Revision-aware RustOSRS decoder infrastructure.
//!
//! This module owns cache-byte decoding mechanics and provenance. It must not
//! leak `rune-fs` transport types or move scene/render/editor semantics into the
//! cache layer.

mod animation;
mod context;
mod error;
mod floor;
mod map;
mod model;
mod object;
mod reader;
mod sequence;
mod sprite;
mod texture;
mod vars;

pub use animation::{
    decode_legacy_animation_frame, decode_legacy_skeleton, legacy_frame_skeleton_id,
};
pub use context::{DecoderContext, DecoderContextError};
pub use error::{
    ArchiveFileProvenance, ByteSpan, DecodeError, DecodeErrorKind, DecodeResult, DecodeSubject,
    XteaKeyProvenance, XteaProvenanceError,
};
pub use floor::{decode_floor_overlay, decode_floor_underlay};
pub use map::{
    DecodedLocation, DecodedLocations, DecodedTerrain, EncodedTerrainOverlay, EncodedTerrainTile,
    EncodedTileHeight, LOCATION_FILE_ID, MAP_INDEX_ID, MODERN_MAP_LAYOUT_MIN_BUILD, MapSquareFiles,
    MapSquareResolutionError, TERRAIN_FILE_ID, decode_locations, decode_terrain,
    resolve_map_square,
};
pub use model::decode_model_data;
pub use object::decode_object_definition;
pub use reader::BinaryReader;
pub use sequence::decode_sequence_definition;
pub use sprite::{IndexedSprite, decode_sprite_group};
pub use texture::decode_texture_definition;
pub use vars::{decode_varbit, decode_varp};

#[cfg(test)]
pub(crate) mod test_support {
    use super::{ArchiveFileProvenance, DecoderContext, DecoderContextError};
    use crate::profile::TargetProfile;

    const TARGET_PROFILE_YAML: &str =
        include_str!("../../../../profiles/osrs-live-241-2026-09-30-openrs2-2727.yaml");

    pub(crate) fn target_context() -> Result<DecoderContext, DecoderContextError> {
        let profile = TargetProfile::from_yaml_str(TARGET_PROFILE_YAML)
            .map_err(DecoderContextError::InvalidProfile)?;
        DecoderContext::from_profile(&profile)
    }

    pub(crate) fn source() -> ArchiveFileProvenance {
        ArchiveFileProvenance::new(2, 6, Some(0))
    }
}
