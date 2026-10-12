//! The editor's object edits: placements removed from, or added to, the map as the cache has it.
//!
//! Like [`crate::TerrainEdits`] this is an edit layer over a read-only cache: the world worker keeps
//! a log per region and replays it over the decoded locations every time the region is built, so a
//! rebuilt region shows the edits and the cache is never written. An undo is the inverse edit; an add
//! that cancels an earlier remove (or the other way round) drops both from the log.

use osrs_cache::decode::DecodedLocation;
use osrs_core::coords::{RegionCoord, RegionTile, SourcePlane};
use osrs_core::ids::ObjectId;
use std::collections::HashMap;

/// One placement: which object, where, and how it stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocSpec {
    pub object_id: u32,
    /// World tile of the placement anchor.
    pub tile: (i32, i32),
    /// Source (map file) plane.
    pub plane: u8,
    pub loc_type: u8,
    pub orientation: u8,
}

impl LocSpec {
    pub fn region(&self) -> (i32, i32) {
        (self.tile.0.div_euclid(64), self.tile.1.div_euclid(64))
    }

    fn matches(&self, location: &DecodedLocation, region: (i32, i32)) -> bool {
        location.object_id.get() == self.object_id
            && location.loc_type == self.loc_type
            && location.orientation == self.orientation
            && location.source_plane.index().get() == self.plane
            && i32::from(location.tile.x()) + region.0 * 64 == self.tile.0
            && i32::from(location.tile.y()) + region.1 * 64 == self.tile.1
    }

    fn to_location(self) -> Option<DecodedLocation> {
        let (rx, ry) = self.region();
        Some(DecodedLocation {
            object_id: ObjectId::new(self.object_id),
            tile: RegionTile::new(
                u8::try_from(self.tile.0 - rx * 64).ok()?,
                u8::try_from(self.tile.1 - ry * 64).ok()?,
            )?,
            source_plane: SourcePlane::new(self.plane)?,
            loc_type: self.loc_type,
            orientation: self.orientation,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocEdit {
    Remove(LocSpec),
    Add(LocSpec),
}

impl LocEdit {
    fn spec(&self) -> LocSpec {
        match self {
            Self::Remove(spec) | Self::Add(spec) => *spec,
        }
    }

    /// The edit that undoes this one.
    pub fn inverse(self) -> Self {
        match self {
            Self::Remove(spec) => Self::Add(spec),
            Self::Add(spec) => Self::Remove(spec),
        }
    }
}

/// Every object edit, by region, in the order they were made.
#[derive(Debug, Default)]
pub struct LocEdits {
    by_region: HashMap<(i32, i32), Vec<LocEdit>>,
}

impl LocEdits {
    pub fn apply(&mut self, edits: &[LocEdit]) {
        for edit in edits {
            let log = self.by_region.entry(edit.spec().region()).or_default();
            // An edit that undoes a logged one cancels it instead of piling up.
            if let Some(position) = log.iter().rposition(|logged| *logged == edit.inverse()) {
                log.remove(position);
            } else {
                log.push(*edit);
            }
        }
    }

    /// Whether any edit touches `region`.
    pub fn touches(&self, region: (i32, i32)) -> bool {
        self.by_region
            .get(&region)
            .is_some_and(|log| !log.is_empty())
    }

    /// `locations` of `region` with the logged edits replayed over them.
    pub fn replay(
        &self,
        region: RegionCoord,
        locations: &[DecodedLocation],
    ) -> Vec<DecodedLocation> {
        let key = (region.x, region.y);
        let mut list = locations.to_vec();
        let Some(log) = self.by_region.get(&key) else {
            return list;
        };
        for edit in log {
            match edit {
                LocEdit::Remove(spec) => {
                    if let Some(position) = list.iter().position(|l| spec.matches(l, key)) {
                        list.remove(position);
                    }
                }
                LocEdit::Add(spec) => {
                    if let Some(location) = spec.to_location() {
                        list.push(location);
                    }
                }
            }
        }
        list
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(x: i32, y: i32) -> LocSpec {
        LocSpec {
            object_id: 1124,
            tile: (x, y),
            plane: 0,
            loc_type: 10,
            orientation: 1,
        }
    }

    fn at(spec: LocSpec) -> DecodedLocation {
        spec.to_location()
            .unwrap_or_else(|| panic!("{spec:?} is not a valid placement"))
    }

    #[test]
    fn removing_and_adding_change_the_replayed_list_and_inverses_cancel() {
        let region = RegionCoord::new(50, 50);
        let (x, y) = (50 * 64 + 5, 50 * 64 + 7);
        let base = vec![at(spec(x, y)), at(spec(x + 1, y))];
        let mut edits = LocEdits::default();
        edits.apply(&[LocEdit::Remove(spec(x, y))]);
        assert_eq!(edits.replay(region, &base), vec![at(spec(x + 1, y))]);
        edits.apply(&[LocEdit::Add(spec(x + 3, y + 3))]);
        assert_eq!(edits.replay(region, &base).len(), 2);
        // Undoing both leaves the log empty and the list as the cache has it.
        edits.apply(&[
            LocEdit::Add(spec(x, y)),
            LocEdit::Remove(spec(x + 3, y + 3)),
        ]);
        assert!(!edits.touches((50, 50)));
        assert_eq!(edits.replay(region, &base), base);
    }

    #[test]
    fn a_remove_that_matches_nothing_changes_nothing() {
        let region = RegionCoord::new(0, 0);
        let base = vec![at(spec(3, 3))];
        let mut edits = LocEdits::default();
        edits.apply(&[LocEdit::Remove(spec(9, 9))]);
        assert_eq!(edits.replay(region, &base), base);
    }
}
