//! Canonical cache-independent definition representations.
//!
//! These structures preserve semantic distinctions needed by M3-M8 without
//! exposing cache codec structs or transport types. Revision-specific sentinels
//! are normalized to `Option`/enums at the decoder boundary rather than stored
//! as magic integers here.

use crate::floor_color::{OverlayHsl, UnderlayHsl};
use crate::ids::{
    CategoryId, FloorOverlayId, FloorUnderlayId, FrameId, ItemId, MapIconId, MapSceneId, ModelId,
    ObjectId, SequenceId, SkeletalAnimationId, SpriteId, TextureId, VarbitId, VarpId,
};
use crate::provenance::TargetProvenance;
use core::fmt;

/// Definition identity plus the exact target/cache provenance that produced it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DefinitionIdentity<I> {
    pub id: I,
    pub provenance: TargetProvenance,
}

impl<I> DefinitionIdentity<I> {
    pub const fn new(id: I, provenance: TargetProvenance) -> Self {
        Self { id, provenance }
    }
}

/// Validated 24-bit RGB semantic value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Rgb24(u32);

impl Rgb24 {
    pub const MAX: u32 = 0x00ff_ffff;

    pub const fn new(value: u32) -> Option<Self> {
        if value <= Self::MAX {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

impl fmt::Display for Rgb24 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "#{:06x}", self.0)
    }
}

/// Loc/object model type byte. Placement semantics are owned by M6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocType(u8);

impl LocType {
    pub const fn new(raw: u8) -> Self {
        Self(raw)
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

/// A model entry from an object definition with an explicit loc type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypedObjectModel {
    pub loc_type: LocType,
    pub model_id: ModelId,
}

/// Canonical distinction between the typed opcode-1-style and untyped
/// opcode-5-style object model tables.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ObjectModels {
    Untyped(Vec<ModelId>),
    Typed(Vec<TypedObjectModel>),
}

/// Raw recolor replacement pair. Values are source model color/HSL entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RecolorPair {
    pub from: u16,
    pub to: u16,
}

/// Raw retexture replacement pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RetexturePair {
    pub from: u16,
    pub to: u16,
}

/// Definition-authored model scale. `128` is the reference identity value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModelScale {
    pub x: u16,
    pub y: u16,
    pub z: u16,
}

impl ModelScale {
    pub const IDENTITY: Self = Self {
        x: 128,
        y: 128,
        z: 128,
    };
}

/// Definition-authored signed model translation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModelTranslation {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl ModelTranslation {
    pub const ZERO: Self = Self { x: 0, y: 0, z: 0 };
}

/// Collision/placement flags retained without performing scene dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectPlacementFlags {
    pub interact_type: u8,
    pub blocks_projectiles: bool,
    pub clipped: bool,
    pub model_clipped: bool,
    pub obstructs_ground: bool,
    pub solid: bool,
}

/// Morph selector and explicit fallback behavior from object opcodes 77/92.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ObjectMorphs {
    pub transform_varbit: Option<VarbitId>,
    pub transform_varp: Option<VarpId>,
    /// Non-fallback transform entries. `None` is a semantic null transform.
    pub transforms: Vec<Option<ObjectId>>,
    /// Explicit/final fallback transform. `None` means null fallback.
    pub fallback: Option<ObjectId>,
}

/// Canonical object/loc definition inputs required by later semantic milestones.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ObjectDefinition {
    pub identity: DefinitionIdentity<ObjectId>,
    pub name: Option<String>,
    pub models: Option<ObjectModels>,
    pub size_x: u16,
    pub size_y: u16,
    pub placement: ObjectPlacementFlags,
    pub decoration_displacement: u16,
    pub support_items: Option<u8>,
    pub is_rotated: bool,
    pub non_flat_shading: bool,
    pub contour_clip: Option<u32>,
    /// Opcode 42: whole-model recolor value (`fullRecolor`). Decoded; render semantics unverified.
    pub full_recolor: Option<u16>,
    /// Opcode 96: ground raise byte (`raise`). Decoded; render semantics unverified.
    pub ground_raise: u8,
    pub animation: Option<SequenceId>,
    pub ambient: i16,
    pub contrast: i16,
    pub scale: ModelScale,
    pub translation: ModelTranslation,
    pub recolors: Vec<RecolorPair>,
    pub retextures: Vec<RetexturePair>,
    pub morphs: Option<ObjectMorphs>,
    pub map_scene: Option<MapSceneId>,
    pub map_icon: Option<MapIconId>,
    pub category: Option<CategoryId>,
    pub actions: [Option<String>; 5],
}

/// Canonical floor-underlay definition including exact post-decode HSL state.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FloorUnderlayDefinition {
    pub identity: DefinitionIdentity<FloorUnderlayId>,
    pub rgb: Rgb24,
    pub hsl: UnderlayHsl,
}

/// Canonical floor-overlay definition including primary/secondary post-decode HSL.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FloorOverlayDefinition {
    pub identity: DefinitionIdentity<FloorOverlayId>,
    pub primary_rgb: Rgb24,
    pub texture: Option<TextureId>,
    pub hide_underlay: bool,
    pub secondary_rgb: Option<Rgb24>,
    pub primary_hsl: OverlayHsl,
    pub secondary_hsl: Option<OverlayHsl>,
}

/// Canonical varbit definition. Bit-range interpretation is verified by M3.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VarbitDefinition {
    pub identity: DefinitionIdentity<VarbitId>,
    pub base_varp: VarpId,
    pub start_bit: u8,
    pub end_bit: u8,
}

/// Canonical varp definition fields required by current semantic consumers.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VarpDefinition {
    pub identity: DefinitionIdentity<VarpId>,
    pub client_type: u16,
}

/// Texture/material definition inputs retained for later pixel/material building.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TextureDefinition {
    pub identity: DefinitionIdentity<TextureId>,
    pub average_rgb: u16,
    pub opaque: bool,
    pub source_sprites: Vec<SpriteId>,
    pub combine_modes: Vec<u8>,
    pub combine_directions: Vec<u8>,
    pub color_transforms: Vec<u32>,
    pub animation_direction: u8,
    pub animation_speed: u8,
}

/// Skeletal animation range metadata used by build-241-era sequences.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SequenceRange {
    pub start: u16,
    pub end: u16,
}

/// Canonical sequence definition inputs required by later animation ownership.
///
/// Frame-based and skeletal fields are intentionally not modeled as mutually
/// exclusive because the decoder must preserve target data rather than force a
/// convenience representation unsupported by source evidence.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SequenceDefinition {
    pub identity: DefinitionIdentity<SequenceId>,
    pub frame_ids: Vec<FrameId>,
    pub frame_delays: Vec<u16>,
    pub frame_step: Option<u16>,
    pub interleave: Option<Vec<u16>>,
    pub max_loops: u16,
    pub precedence_animating: u8,
    pub priority: u8,
    /// Build-241 `restartMode` (opcode 11). Dynamic-object replacement only
    /// preserves primary playback state for equal sequence ids when this is 0.
    pub restart_mode: u8,
    pub left_hand_item: Option<ItemId>,
    pub right_hand_item: Option<ItemId>,
    pub skeletal_animation: Option<SkeletalAnimationId>,
    pub skeletal_range: Option<SequenceRange>,
    pub skeletal_mask: Option<Vec<bool>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provenance::{CacheFingerprint, ProfileDigest, TargetProvenance};
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    const PROFILE_DIGEST: &str = "cfdefa9ef99eff799fcef4fdf0ec78d9fdcd72d8e5be78e1c154d018ab4575b7";
    const CACHE_FINGERPRINT: &str =
        "ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38";

    fn provenance() -> Result<TargetProvenance, Box<dyn std::error::Error>> {
        Ok(TargetProvenance::new(
            "osrs-live-241-2026-09-30-openrs2-2727",
            ProfileDigest::from_lower_hex(PROFILE_DIGEST)?,
            CacheFingerprint::from_lower_hex(CACHE_FINGERPRINT)?,
            1,
        )?)
    }

    #[test]
    fn rgb24_rejects_values_outside_semantic_width() {
        assert_eq!(Rgb24::new(0), Some(Rgb24(0)));
        assert_eq!(Rgb24::new(0x00ff_ffff), Some(Rgb24(0x00ff_ffff)));
        assert_eq!(Rgb24::new(0x0100_0000), None);
    }

    #[test]
    fn typed_and_untyped_object_model_tables_remain_distinct() {
        let untyped = ObjectModels::Untyped(vec![ModelId::new(10), ModelId::new(11)]);
        let typed = ObjectModels::Typed(vec![TypedObjectModel {
            loc_type: LocType::new(10),
            model_id: ModelId::new(10),
        }]);

        assert_ne!(untyped, typed);
    }

    #[test]
    fn morphs_preserve_null_entries_and_explicit_fallback() {
        let morphs = ObjectMorphs {
            transform_varbit: Some(VarbitId::new(7)),
            transform_varp: Some(VarpId::new(8)),
            transforms: vec![Some(ObjectId::new(100)), None],
            fallback: Some(ObjectId::new(200)),
        };

        assert_eq!(morphs.transforms[1], None);
        assert_eq!(morphs.fallback, Some(ObjectId::new(200)));
        assert_eq!(morphs.transform_varbit, Some(VarbitId::new(7)));
    }

    #[test]
    fn overlay_absence_is_not_replaced_with_zero_ids() -> Result<(), Box<dyn std::error::Error>> {
        let primary_rgb = Rgb24(0x123456);
        let overlay = FloorOverlayDefinition {
            identity: DefinitionIdentity::new(FloorOverlayId::new(5), provenance()?),
            primary_rgb,
            texture: None,
            hide_underlay: true,
            secondary_rgb: None,
            primary_hsl: OverlayHsl::from_rgb(primary_rgb),
            secondary_hsl: None,
        };

        assert_eq!(overlay.texture, None);
        assert_eq!(overlay.secondary_rgb, None);
        assert_eq!(overlay.secondary_hsl, None);
        Ok(())
    }

    #[test]
    fn texture_definition_has_no_fixed_256_entry_semantic_limit()
    -> Result<(), Box<dyn std::error::Error>> {
        let source_sprites = (0..300).map(SpriteId::new).collect::<Vec<_>>();
        let texture = TextureDefinition {
            identity: DefinitionIdentity::new(TextureId::new(400), provenance()?),
            average_rgb: 1234,
            opaque: true,
            source_sprites,
            combine_modes: Vec::new(),
            combine_directions: Vec::new(),
            color_transforms: Vec::new(),
            animation_direction: 0,
            animation_speed: 0,
        };

        assert_eq!(texture.source_sprites.len(), 300);
        assert_eq!(texture.identity.id, TextureId::new(400));
        Ok(())
    }

    #[test]
    fn sequence_retains_frame_and_skeletal_inputs_without_false_exclusivity()
    -> Result<(), Box<dyn std::error::Error>> {
        let sequence = SequenceDefinition {
            identity: DefinitionIdentity::new(SequenceId::new(12), provenance()?),
            frame_ids: vec![FrameId::new(0x0012_0034)],
            frame_delays: vec![5],
            frame_step: Some(1),
            interleave: Some(vec![0, 2, 4]),
            max_loops: 99,
            precedence_animating: 2,
            priority: 2,
            restart_mode: 2,
            left_hand_item: Some(ItemId::new(4151)),
            right_hand_item: None,
            skeletal_animation: Some(SkeletalAnimationId::new(900)),
            skeletal_range: Some(SequenceRange { start: 10, end: 20 }),
            skeletal_mask: Some(vec![true, false, true]),
        };

        assert_eq!(sequence.frame_ids, vec![FrameId::new(0x0012_0034)]);
        assert_eq!(
            sequence.skeletal_animation,
            Some(SkeletalAnimationId::new(900))
        );
        Ok(())
    }

    #[test]
    fn definition_values_are_hashable_and_clone_without_shared_mutation()
    -> Result<(), Box<dyn std::error::Error>> {
        let original = VarbitDefinition {
            identity: DefinitionIdentity::new(VarbitId::new(3), provenance()?),
            base_varp: VarpId::new(17),
            start_bit: 2,
            end_bit: 5,
        };
        let cloned = original.clone();

        let mut original_hasher = DefaultHasher::new();
        original.hash(&mut original_hasher);
        let mut cloned_hasher = DefaultHasher::new();
        cloned.hash(&mut cloned_hasher);

        assert_eq!(original, cloned);
        assert_eq!(original_hasher.finish(), cloned_hasher.finish());
        Ok(())
    }
}
