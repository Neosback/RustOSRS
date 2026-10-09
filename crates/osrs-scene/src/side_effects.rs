//! Definition-driven semantic side-effect planning for M6 scene construction.
//!
//! This module deliberately plans semantic side effects without committing to the
//! reference client's private collision, shadow, or occlusion array layout. The
//! resulting plan preserves the canonical definition inputs and the placement
//! facts required for later exact side-grid mutation.

use crate::placement::{Footprint, PlacementKind, PlacementPlan};
use osrs_core::definitions::{LocType, ObjectDefinition, ObjectPlacementFlags};

/// Canonical definition inputs that can affect non-renderable scene state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DefinitionSideEffectInputs {
    pub interact_type: u8,
    pub blocks_projectiles: bool,
    pub clipped: bool,
    pub model_clipped: bool,
    pub obstructs_ground: bool,
    pub solid: bool,
    pub decoration_displacement: u16,
}

impl DefinitionSideEffectInputs {
    pub const fn new(flags: ObjectPlacementFlags, decoration_displacement: u16) -> Self {
        Self {
            interact_type: flags.interact_type,
            blocks_projectiles: flags.blocks_projectiles,
            clipped: flags.clipped,
            model_clipped: flags.model_clipped,
            obstructs_ground: flags.obstructs_ground,
            solid: flags.solid,
            decoration_displacement,
        }
    }

    pub fn from_definition(definition: &ObjectDefinition) -> Self {
        Self::new(definition.placement, definition.decoration_displacement)
    }
}

/// Exact collision operation selected by the audited initial-placement path.
///
/// This is an operation-level semantic plan, not a copy of CollisionMap's bit
/// layout. The later collision-grid owner can translate these operations into
/// exact side-grid mutations without re-interpreting loc placement semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollisionSideEffect {
    None,
    FloorDecorationBlock,
    Boundary {
        loc_type: LocType,
        orientation: u8,
        blocks_projectiles: bool,
    },
    GameObject {
        footprint: Footprint,
        blocks_projectiles: bool,
    },
}

/// Deterministic side-effect recipe retained alongside semantic placement data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneSideEffectPlan {
    pub definition: DefinitionSideEffectInputs,
    pub collision: CollisionSideEffect,
    /// Reference scene wall-displacement metadata update. `None` means the
    /// default value remains in force or the loc type does not update it.
    pub wall_displacement: Option<u16>,
}

/// Build the semantic side-effect recipe from canonical definition inputs.
pub fn plan_definition_side_effects(
    definition: &ObjectDefinition,
    placement: PlacementPlan,
) -> SceneSideEffectPlan {
    plan_side_effects(
        DefinitionSideEffectInputs::from_definition(definition),
        placement,
    )
}

/// Build the semantic side-effect recipe from already-normalized definition
/// fields and an exact placement plan.
pub fn plan_side_effects(
    definition: DefinitionSideEffectInputs,
    placement: PlacementPlan,
) -> SceneSideEffectPlan {
    let collision = match placement.kind {
        PlacementKind::FloorDecoration(_) if definition.interact_type == 1 => {
            CollisionSideEffect::FloorDecorationBlock
        }
        PlacementKind::Boundary(_) if definition.interact_type != 0 => {
            CollisionSideEffect::Boundary {
                loc_type: placement.source_loc_type,
                orientation: placement.source_orientation,
                blocks_projectiles: definition.blocks_projectiles,
            }
        }
        PlacementKind::GameObject(_) if definition.interact_type != 0 => {
            CollisionSideEffect::GameObject {
                // The reference collision call uses the rotated definition
                // dimensions even for visually 1x1 storage paths such as type 9.
                footprint: placement.rotated_definition_footprint,
                blocks_projectiles: definition.blocks_projectiles,
            }
        }
        PlacementKind::FloorDecoration(_)
        | PlacementKind::Boundary(_)
        | PlacementKind::WallDecoration(_)
        | PlacementKind::GameObject(_) => CollisionSideEffect::None,
    };

    let wall_displacement = match placement.source_loc_type.get() {
        0 | 2 | 9 if definition.decoration_displacement != 16 => {
            Some(definition.decoration_displacement)
        }
        _ => None,
    };

    SceneSideEffectPlan {
        definition,
        collision,
        wall_displacement,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::placement::{PlacementInput, plan_placement};
    use osrs_core::coords::SceneTile;

    fn flags(interact_type: u8, blocks_projectiles: bool) -> ObjectPlacementFlags {
        ObjectPlacementFlags {
            interact_type,
            blocks_projectiles,
            clipped: true,
            model_clipped: true,
            obstructs_ground: true,
            solid: false,
        }
    }

    fn placement(
        loc_type: u8,
        orientation: u8,
        size_x: u16,
        size_y: u16,
    ) -> Result<PlacementPlan, crate::placement::PlacementError> {
        plan_placement(PlacementInput {
            loc_type: LocType::new(loc_type),
            orientation,
            tile: SceneTile::new(10, 20),
            size_x,
            size_y,
            sampled_height: 0,
            existing_wall_displacement: None,
        })
    }

    #[test]
    fn floor_decoration_collision_only_uses_interact_type_one()
    -> Result<(), crate::placement::PlacementError> {
        let floor = placement(22, 0, 1, 1)?;

        let blocked = plan_side_effects(DefinitionSideEffectInputs::new(flags(1, true), 16), floor);
        assert_eq!(blocked.collision, CollisionSideEffect::FloorDecorationBlock);

        for interact_type in [0, 2, 3] {
            let plan = plan_side_effects(
                DefinitionSideEffectInputs::new(flags(interact_type, true), 16),
                floor,
            );
            assert_eq!(plan.collision, CollisionSideEffect::None);
        }
        Ok(())
    }

    #[test]
    fn boundary_collision_preserves_type_orientation_and_projectile_flag()
    -> Result<(), crate::placement::PlacementError> {
        let boundary = placement(2, 3, 1, 1)?;
        let plan = plan_side_effects(
            DefinitionSideEffectInputs::new(flags(2, true), 24),
            boundary,
        );
        assert_eq!(
            plan.collision,
            CollisionSideEffect::Boundary {
                loc_type: LocType::new(2),
                orientation: 3,
                blocks_projectiles: true,
            }
        );
        assert_eq!(plan.wall_displacement, Some(24));
        Ok(())
    }

    #[test]
    fn game_object_collision_uses_definition_footprint_for_one_by_one_storage()
    -> Result<(), crate::placement::PlacementError> {
        let object = placement(9, 1, 2, 3)?;
        let PlacementKind::GameObject(game) = object.kind else {
            panic!("type 9 must use game-object storage");
        };
        assert_eq!(game.storage_footprint, Footprint::ONE_BY_ONE);
        assert_eq!(object.rotated_definition_footprint, Footprint::new(3, 2));

        let plan = plan_side_effects(DefinitionSideEffectInputs::new(flags(1, false), 16), object);
        assert_eq!(
            plan.collision,
            CollisionSideEffect::GameObject {
                footprint: Footprint::new(3, 2),
                blocks_projectiles: false,
            }
        );
        Ok(())
    }

    #[test]
    fn wall_decorations_do_not_invent_collision_operations()
    -> Result<(), crate::placement::PlacementError> {
        for loc_type in 4..=8 {
            let decor = placement(loc_type, 2, 1, 1)?;
            let plan =
                plan_side_effects(DefinitionSideEffectInputs::new(flags(2, true), 31), decor);
            assert_eq!(plan.collision, CollisionSideEffect::None);
            assert_eq!(plan.wall_displacement, None);
        }
        Ok(())
    }

    #[test]
    fn wall_displacement_metadata_is_limited_to_audited_loc_types()
    -> Result<(), crate::placement::PlacementError> {
        for loc_type in [0, 2, 9] {
            let plan = plan_side_effects(
                DefinitionSideEffectInputs::new(flags(0, false), 37),
                placement(loc_type, 0, 1, 1)?,
            );
            assert_eq!(plan.wall_displacement, Some(37), "loc type {loc_type}");
        }

        for loc_type in [1, 3, 4, 10, 12, 22] {
            let plan = plan_side_effects(
                DefinitionSideEffectInputs::new(flags(0, false), 37),
                placement(loc_type, 0, 1, 1)?,
            );
            assert_eq!(plan.wall_displacement, None, "loc type {loc_type}");
        }

        for loc_type in [0, 2, 9] {
            let plan = plan_side_effects(
                DefinitionSideEffectInputs::new(flags(0, false), 16),
                placement(loc_type, 0, 1, 1)?,
            );
            assert_eq!(plan.wall_displacement, None, "loc type {loc_type}");
        }
        Ok(())
    }

    #[test]
    fn clipping_and_obstruction_inputs_are_preserved_without_guessing_grid_bits()
    -> Result<(), crate::placement::PlacementError> {
        let definition = DefinitionSideEffectInputs::new(
            ObjectPlacementFlags {
                interact_type: 1,
                blocks_projectiles: true,
                clipped: false,
                model_clipped: true,
                obstructs_ground: true,
                solid: true,
            },
            48,
        );
        let plan = plan_side_effects(definition, placement(10, 0, 2, 2)?);
        assert_eq!(plan.definition, definition);
        assert!(!plan.definition.clipped);
        assert!(plan.definition.model_clipped);
        assert!(plan.definition.obstructs_ground);
        assert!(plan.definition.solid);
        Ok(())
    }
}
