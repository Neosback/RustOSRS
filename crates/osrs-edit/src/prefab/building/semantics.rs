//! Versioned author-reviewed room, portal and placement-anchor metadata.
use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrefabSemantics {
    #[serde(default)]
    pub rooms: Vec<PrefabRoom>,
    #[serde(default)]
    pub portals: Vec<PrefabPortal>,
    #[serde(default)]
    pub anchor: Option<PrefabCell>,
    #[serde(default)]
    pub locked_parts: Vec<usize>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrefabRoom {
    pub id: u16,
    pub role: String,
    pub cells: Vec<PrefabCell>,
    /// False retains the distinction between census inference and human labels.
    pub human: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrefabPortal {
    pub object: Option<usize>,
    /// None names the outside. Both sides cannot be outside.
    pub rooms: [Option<u16>; 2],
    pub approaches: [PrefabCell; 2],
    pub human: bool,
}
impl PrefabSemantics {
    pub(super) fn validate(
        &self,
        prefab: &Prefab,
        floors: &[PrefabCell],
    ) -> Result<(), BuildingPrefabError> {
        let floor: BTreeSet<_> = floors.iter().copied().collect();
        let mut ids = BTreeSet::new();
        let mut assigned = BTreeSet::new();
        for room in &self.rooms {
            if room.role.trim().is_empty()
                || room.cells.is_empty()
                || !ids.insert(room.id)
                || room
                    .cells
                    .iter()
                    .any(|c| !floor.contains(c) || !assigned.insert(*c))
            {
                return Err(BuildingPrefabError::InvalidMask);
            }
        }
        if self.anchor.is_some_and(|a| {
            a.x >= prefab.width || a.y >= prefab.height || a.plane >= prefab.planes
        }) || self.locked_parts.iter().any(|&i| i >= prefab.objects.len())
        {
            return Err(BuildingPrefabError::InvalidMask);
        }
        for portal in &self.portals {
            if portal.rooms == [None, None]
                || portal.rooms[0] == portal.rooms[1]
                || portal.rooms.iter().flatten().any(|id| !ids.contains(id))
                || portal.object.is_some_and(|i| i >= prefab.objects.len())
                || portal.approaches.iter().any(|c| {
                    c.x >= prefab.width || c.y >= prefab.height || c.plane >= prefab.planes
                })
                || portal.rooms.iter().zip(&portal.approaches).any(|(id, c)| {
                    id.is_some_and(|id| {
                        !self.rooms.iter().any(|r| r.id == id && r.cells.contains(c))
                    })
                })
            {
                return Err(BuildingPrefabError::InvalidRelation);
            }
        }
        Ok(())
    }

    /// Split a room using a selected subset. No floor ownership is added or removed.
    pub fn split(
        &mut self,
        id: u16,
        selected: &BTreeSet<PrefabCell>,
    ) -> Result<u16, BuildingPrefabError> {
        let next = self
            .rooms
            .iter()
            .map(|r| r.id)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(BuildingPrefabError::CoordinateOverflow)?;
        let room = self
            .rooms
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or(BuildingPrefabError::InvalidMask)?;
        let cells: Vec<_> = room
            .cells
            .iter()
            .filter(|c| selected.contains(c))
            .copied()
            .collect();
        if cells.is_empty() || cells.len() == room.cells.len() {
            return Err(BuildingPrefabError::InvalidMask);
        }
        room.cells.retain(|c| !selected.contains(c));
        room.human = true;
        let role = room.role.clone();
        self.rooms.push(PrefabRoom {
            id: next,
            role,
            cells,
            human: true,
        });
        self.portals.clear(); // Room incidence must be reviewed after a partition change.
        Ok(next)
    }

    pub fn merge(&mut self, first: u16, second: u16) -> Result<(), BuildingPrefabError> {
        if first == second || !self.rooms.iter().any(|r| r.id == first) {
            return Err(BuildingPrefabError::InvalidMask);
        }
        let index = self
            .rooms
            .iter()
            .position(|r| r.id == second)
            .ok_or(BuildingPrefabError::InvalidMask)?;
        let removed = self.rooms.remove(index);
        if let Some(room) = self.rooms.iter_mut().find(|r| r.id == first) {
            room.cells.extend(removed.cells);
            room.cells.sort();
            room.cells.dedup();
            room.human = true;
        }
        self.portals.clear();
        Ok(())
    }
}
