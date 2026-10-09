//! Explicit semantic plane-domain handling for M6 scene construction.
//!
//! Source, collision, storage, and render/height levels are deliberately kept
//! distinct. Bridge collision projection is not a universal plane rewrite.

use osrs_core::coords::{CollisionPlane, PlaneIndex, RenderLevel, SourcePlane, StoragePlane};

/// Plane identities used by one initial decoded location placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlacementPlanes {
    pub source: SourcePlane,
    pub collision: CollisionPlane,
    pub storage: StoragePlane,
    pub render_level: RenderLevel,
}

impl PlacementPlanes {
    /// Build the initial placement plane domains while preserving the encoded
    /// source plane. A bridge bit only projects collision downward.
    pub fn initial(source: SourcePlane, bridge_bit_set: bool) -> Self {
        let value = source.index().get();
        let collision = collision_plane(source, bridge_bit_set);
        let storage = StoragePlane::new(value)
            .expect("validated source plane must map to a valid storage plane");
        let render_level = RenderLevel::new(value)
            .expect("validated source plane must map to a valid render level");
        Self {
            source,
            collision,
            storage,
            render_level,
        }
    }
}

/// Resolve only the collision-plane bridge rule from `PLANES-002`.
pub fn collision_plane(source: SourcePlane, bridge_bit_set: bool) -> CollisionPlane {
    let value = source.index().get();
    if bridge_bit_set {
        if value == 0 {
            CollisionPlane::None
        } else {
            CollisionPlane::Plane(
                PlaneIndex::new(value - 1)
                    .expect("source plane above zero must have a valid lower plane"),
            )
        }
    } else {
        CollisionPlane::Plane(source.index())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_plane_domains_preserve_source_and_separate_collision_projection() {
        for value in 0..=3 {
            let source = SourcePlane::new(value).expect("test source plane is valid");

            let normal = PlacementPlanes::initial(source, false);
            assert_eq!(normal.source, source);
            assert_eq!(normal.storage.index().get(), value);
            assert_eq!(normal.render_level.index().get(), value);
            assert_eq!(normal.collision, CollisionPlane::Plane(source.index()));

            let bridged = PlacementPlanes::initial(source, true);
            assert_eq!(bridged.source, source);
            assert_eq!(bridged.storage.index().get(), value);
            assert_eq!(bridged.render_level.index().get(), value);
            if value == 0 {
                assert_eq!(bridged.collision, CollisionPlane::None);
            } else {
                assert_eq!(
                    bridged.collision,
                    CollisionPlane::Plane(
                        PlaneIndex::new(value - 1).expect("lower test plane is valid")
                    )
                );
            }
        }
    }
}
