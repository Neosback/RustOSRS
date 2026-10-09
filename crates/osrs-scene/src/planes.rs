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
    pub fn initial(source: SourcePlane, bridge_bit_set: bool) -> Option<Self> {
        let value = source.index().get();
        Some(Self {
            source,
            collision: collision_plane(source, bridge_bit_set),
            storage: StoragePlane::new(value)?,
            render_level: RenderLevel::new(value)?,
        })
    }
}

/// Resolve only the collision-plane bridge rule from `PLANES-002`.
pub fn collision_plane(source: SourcePlane, bridge_bit_set: bool) -> CollisionPlane {
    let value = source.index().get();
    if !bridge_bit_set {
        return CollisionPlane::Plane(source.index());
    }
    if value == 0 {
        return CollisionPlane::None;
    }
    match PlaneIndex::new(value - 1) {
        Some(lower) => CollisionPlane::Plane(lower),
        None => CollisionPlane::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_plane_domains_preserve_source_and_separate_collision_projection() {
        for source in (0..=3).filter_map(SourcePlane::new) {
            let value = source.index().get();
            let normal = PlacementPlanes::initial(source, false);
            assert_eq!(normal.map(|planes| planes.source), Some(source));
            assert_eq!(
                normal.map(|planes| planes.storage.index().get()),
                Some(value)
            );
            assert_eq!(
                normal.map(|planes| planes.render_level.index().get()),
                Some(value)
            );
            assert_eq!(
                normal.map(|planes| planes.collision),
                Some(CollisionPlane::Plane(source.index()))
            );

            let bridged = PlacementPlanes::initial(source, true);
            assert_eq!(bridged.map(|planes| planes.source), Some(source));
            assert_eq!(
                bridged.map(|planes| planes.storage.index().get()),
                Some(value)
            );
            assert_eq!(
                bridged.map(|planes| planes.render_level.index().get()),
                Some(value)
            );
            if value == 0 {
                assert_eq!(
                    bridged.map(|planes| planes.collision),
                    Some(CollisionPlane::None)
                );
            } else {
                assert_eq!(
                    bridged.map(|planes| planes.collision),
                    CollisionPlane::from_index(value - 1)
                );
            }
        }
    }
}
