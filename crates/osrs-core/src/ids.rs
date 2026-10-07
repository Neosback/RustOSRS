//! Stable semantic identifiers used across cache, scene, renderer, and editor layers.
//!
//! Absence is represented with `Option<IdType>` at the owning field rather than
//! by teaching an ID wrapper about revision-specific sentinel values.

use core::fmt;

macro_rules! semantic_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u32);

        impl $name {
            /// Construct an ID from its canonical non-negative numeric value.
            pub const fn new(value: u32) -> Self {
                Self(value)
            }

            /// Return the canonical numeric value.
            pub const fn get(self) -> u32 {
                self.0
            }
        }

        impl From<u32> for $name {
            fn from(value: u32) -> Self {
                Self::new(value)
            }
        }

        impl From<$name> for u32 {
            fn from(value: $name) -> Self {
                value.get()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }
    };
}

semantic_id!(ObjectId, "Canonical object/loc definition ID.");
semantic_id!(
    ModelId,
    "Canonical model definition ID, including target-era 32-bit values."
);
semantic_id!(TextureId, "Canonical semantic texture definition ID.");
semantic_id!(SequenceId, "Canonical animation sequence definition ID.");
semantic_id!(VarbitId, "Canonical varbit definition ID.");
semantic_id!(VarpId, "Canonical varp definition ID.");
semantic_id!(FloorUnderlayId, "Canonical floor-underlay definition ID.");
semantic_id!(FloorOverlayId, "Canonical floor-overlay definition ID.");
semantic_id!(SpriteId, "Canonical texture source sprite/file ID.");
semantic_id!(FrameId, "Canonical packed animation frame reference.");
semantic_id!(
    SkeletalAnimationId,
    "Canonical skeletal animation definition/resource ID."
);
semantic_id!(SoundId, "Canonical object sound-effect ID.");
semantic_id!(MapSceneId, "Canonical object map-scene metadata ID.");
semantic_id!(MapIconId, "Canonical object map-icon metadata ID.");
semantic_id!(CategoryId, "Canonical object category metadata ID.");

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn model_id_preserves_full_u32_range() {
        let id = ModelId::new(u32::MAX);
        assert_eq!(id.get(), u32::MAX);
    }

    #[test]
    fn distinct_id_types_keep_numeric_identity_without_conflation() {
        let object = ObjectId::new(42);
        let model = ModelId::new(42);

        assert_eq!(object.get(), model.get());
        assert_eq!(object.to_string(), "42");
        assert_eq!(model.to_string(), "42");
    }

    #[test]
    fn ids_are_orderable_and_hashable_value_types() {
        let mut ids = BTreeSet::new();
        ids.insert(TextureId::new(9));
        ids.insert(TextureId::new(2));
        ids.insert(TextureId::new(9));

        assert_eq!(
            ids.into_iter().collect::<Vec<_>>(),
            vec![TextureId::new(2), TextureId::new(9)]
        );
    }

    #[test]
    fn ancillary_definition_ids_preserve_full_numeric_identity() {
        assert_eq!(SpriteId::new(u32::MAX).get(), u32::MAX);
        assert_eq!(FrameId::new(0x1234_5678).get(), 0x1234_5678);
        assert_eq!(SkeletalAnimationId::new(900_000).get(), 900_000);
    }
}
