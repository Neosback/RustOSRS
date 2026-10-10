use osrs_core::{
    definitions::{DefinitionIdentity, TextureDefinition},
    ids::{SpriteId, TextureId},
};
use std::{collections::BTreeMap, error::Error, fmt};

/// RuneLite-reference texture animation unit: one source texel per 128x128 texture tick unit.
pub const REFERENCE_TEXTURE_ANIMATION_UNIT: f32 = 1.0 / 128.0;

/// Dense renderer-owned handle into one immutable material table.
///
/// This is deliberately distinct from `TextureId`: sparse/full-width semantic
/// texture ids are never truncated to fit a fixed renderer texture capacity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MaterialHandle(u32);

impl MaterialHandle {
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Reference texture-animation vector before tick scaling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureAnimationVector {
    pub u_units_per_tick: i16,
    pub v_units_per_tick: i16,
}

impl TextureAnimationVector {
    pub const ZERO: Self = Self {
        u_units_per_tick: 0,
        v_units_per_tick: 0,
    };

    /// Convert decoded direction/speed into the imported RuneLite reference vector.
    pub const fn from_direction_speed(direction: u8, speed: u8) -> Self {
        let speed = speed as i16;
        match direction {
            1 => Self {
                u_units_per_tick: 0,
                v_units_per_tick: -speed,
            },
            2 => Self {
                u_units_per_tick: -speed,
                v_units_per_tick: 0,
            },
            3 => Self {
                u_units_per_tick: 0,
                v_units_per_tick: speed,
            },
            4 => Self {
                u_units_per_tick: speed,
                v_units_per_tick: 0,
            },
            _ => Self::ZERO,
        }
    }

    /// Reference UV displacement after `tick` animation steps.
    pub fn uv_offset_at_tick(self, tick: u32) -> (f32, f32) {
        let tick = tick as f32;
        (
            tick * f32::from(self.u_units_per_tick) * REFERENCE_TEXTURE_ANIMATION_UNIT,
            tick * f32::from(self.v_units_per_tick) * REFERENCE_TEXTURE_ANIMATION_UNIT,
        )
    }
}

/// Immutable renderer material input derived from one semantic texture definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderMaterial {
    identity: DefinitionIdentity<TextureId>,
    average_rgb: u16,
    opaque: bool,
    source_sprites: Box<[SpriteId]>,
    combine_modes: Box<[u8]>,
    combine_directions: Box<[u8]>,
    color_transforms: Box<[u32]>,
    animation_direction: u8,
    animation_speed: u8,
    animation_vector: TextureAnimationVector,
}

impl RenderMaterial {
    fn from_definition(definition: &TextureDefinition) -> Self {
        Self {
            identity: definition.identity.clone(),
            average_rgb: definition.average_rgb,
            opaque: definition.opaque,
            source_sprites: definition.source_sprites.clone().into_boxed_slice(),
            combine_modes: definition.combine_modes.clone().into_boxed_slice(),
            combine_directions: definition.combine_directions.clone().into_boxed_slice(),
            color_transforms: definition.color_transforms.clone().into_boxed_slice(),
            animation_direction: definition.animation_direction,
            animation_speed: definition.animation_speed,
            animation_vector: TextureAnimationVector::from_direction_speed(
                definition.animation_direction,
                definition.animation_speed,
            ),
        }
    }

    pub fn identity(&self) -> &DefinitionIdentity<TextureId> {
        &self.identity
    }

    pub const fn texture_id(&self) -> TextureId {
        self.identity.id
    }

    pub const fn average_rgb(&self) -> u16 {
        self.average_rgb
    }

    pub const fn opaque(&self) -> bool {
        self.opaque
    }

    pub fn source_sprites(&self) -> &[SpriteId] {
        &self.source_sprites
    }

    pub fn combine_modes(&self) -> &[u8] {
        &self.combine_modes
    }

    pub fn combine_directions(&self) -> &[u8] {
        &self.combine_directions
    }

    pub fn color_transforms(&self) -> &[u32] {
        &self.color_transforms
    }

    pub const fn animation_direction(&self) -> u8 {
        self.animation_direction
    }

    pub const fn animation_speed(&self) -> u8 {
        self.animation_speed
    }

    pub const fn animation_vector(&self) -> TextureAnimationVector {
        self.animation_vector
    }
}

/// Deterministic material table for one target provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialTable {
    materials: Box<[RenderMaterial]>,
    handles: BTreeMap<TextureId, MaterialHandle>,
}

impl MaterialTable {
    /// Build a deterministic table sorted by full-width semantic texture id.
    pub fn from_definitions(definitions: &[TextureDefinition]) -> Result<Self, MaterialTableError> {
        if let Some(first) = definitions.first() {
            for other in &definitions[1..] {
                if other.identity.provenance != first.identity.provenance {
                    return Err(MaterialTableError::MixedTargetProvenance {
                        first_texture: first.identity.id,
                        other_texture: other.identity.id,
                    });
                }
            }
        }

        let mut ordered = definitions.iter().collect::<Vec<_>>();
        ordered.sort_by_key(|definition| definition.identity.id);

        for pair in ordered.windows(2) {
            if pair[0].identity.id == pair[1].identity.id {
                return Err(MaterialTableError::DuplicateTextureId {
                    texture_id: pair[0].identity.id,
                });
            }
        }

        let mut materials = Vec::with_capacity(ordered.len());
        let mut handles = BTreeMap::new();
        for (index, definition) in ordered.into_iter().enumerate() {
            let handle_index = u32::try_from(index)
                .map_err(|_| MaterialTableError::TooManyMaterials { count: index + 1 })?;
            let handle = MaterialHandle(handle_index);
            handles.insert(definition.identity.id, handle);
            materials.push(RenderMaterial::from_definition(definition));
        }

        Ok(Self {
            materials: materials.into_boxed_slice(),
            handles,
        })
    }

    pub fn materials(&self) -> &[RenderMaterial] {
        &self.materials
    }

    pub fn handle_for_texture(&self, texture_id: TextureId) -> Option<MaterialHandle> {
        self.handles.get(&texture_id).copied()
    }

    pub fn material(&self, handle: MaterialHandle) -> Option<&RenderMaterial> {
        usize::try_from(handle.get())
            .ok()
            .and_then(|index| self.materials.get(index))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialTableError {
    DuplicateTextureId {
        texture_id: TextureId,
    },
    MixedTargetProvenance {
        first_texture: TextureId,
        other_texture: TextureId,
    },
    TooManyMaterials {
        count: usize,
    },
}

impl fmt::Display for MaterialTableError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateTextureId { texture_id } => {
                write!(
                    formatter,
                    "duplicate texture id {texture_id} in material table"
                )
            }
            Self::MixedTargetProvenance {
                first_texture,
                other_texture,
            } => write!(
                formatter,
                "material table cannot mix target provenance between texture {first_texture} and texture {other_texture}"
            ),
            Self::TooManyMaterials { count } => write!(
                formatter,
                "material table contains {count} entries, exceeding renderer handle capacity"
            ),
        }
    }
}

impl Error for MaterialTableError {}
