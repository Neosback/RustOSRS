use crate::MapPosition;
use osrs_core::ids::ObjectId;

/// Valid location/object placement kind used by the client (`0..=22`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocKind(u8);

impl LocKind {
    pub const MAX: u8 = 22;

    pub const fn new(value: u8) -> Option<Self> {
        if value <= Self::MAX {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn get(self) -> u8 {
        self.0
    }

    /// Only scenery kinds 10/11 use the definition footprint; other stored loc kinds occupy one
    /// tile even when their definition advertises a larger model footprint.
    pub const fn uses_definition_footprint(self) -> bool {
        matches!(self.0, 10 | 11)
    }
}

/// Valid cardinal object orientation (`0..=3`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectOrientation(u8);

impl ObjectOrientation {
    pub const MAX: u8 = 3;

    pub const fn new(value: u8) -> Option<Self> {
        if value <= Self::MAX {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

/// One semantic object placement in the editable map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectPlacement {
    pub object_id: ObjectId,
    pub position: MapPosition,
    pub kind: LocKind,
    pub orientation: ObjectOrientation,
}

impl ObjectPlacement {
    /// Existing canonical document order, shared by deterministic spatial query projections.
    pub(crate) fn document_key(&self) -> (MapPosition, u8, u32, u8) {
        (
            self.position,
            self.kind.get(),
            self.object_id.get(),
            self.orientation.get(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loc_kind_and_orientation_validate_at_construction() {
        assert_eq!(LocKind::new(22).map(LocKind::get), Some(22));
        assert_eq!(LocKind::new(23), None);
        assert_eq!(
            ObjectOrientation::new(3).map(ObjectOrientation::get),
            Some(3)
        );
        assert_eq!(ObjectOrientation::new(4), None);
    }
}
