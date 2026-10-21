//! Explicit reviewed occurrence groups reuse ordinary prefab capture and placement semantics.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

impl Prefab {
    /// Declare selected occurrences one reusable assembly. Attachment/roof/adjacency evidence is
    /// deliberately not promoted into this stronger intent. Invalid selection changes nothing.
    pub fn link_assembly(&mut self, members: &[usize]) -> Result<(), PrefabError> {
        self.validate()?;
        let members: BTreeSet<_> = members.iter().copied().collect();
        if members.len() < 2 || members.iter().any(|&i| i >= self.objects.len()) {
            return Err(PrefabError::Building(BuildingPrefabError::InvalidRelation));
        }
        let building = self
            .building
            .as_mut()
            .ok_or(PrefabError::Building(BuildingPrefabError::Unreviewed))?;
        let first = *members.first().ok_or(PrefabError::Nothing)?;
        for &other in members.iter().skip(1) {
            if !building.relations.iter().any(|r| {
                r.human
                    && r.kind == "assembly"
                    && ((r.from == first && r.to == other) || (r.from == other && r.to == first))
            }) {
                building.relations.push(PrefabRelation {
                    from: first,
                    to: other,
                    kind: "assembly".into(),
                    evidence: "human".into(),
                    human: true,
                });
            }
        }
        Ok(())
    }

    /// Connected groups of explicit human assembly links, sorted by source occurrence index.
    /// This is a disposable projection of existing metadata, never a second membership store.
    pub fn reviewed_assemblies(&self) -> Result<Vec<Vec<usize>>, PrefabError> {
        self.validate()?;
        let Some(building) = &self.building else {
            return Ok(vec![]);
        };
        if !building.reviewed {
            return Err(PrefabError::Building(BuildingPrefabError::Unreviewed));
        }
        let mut adjacent = BTreeMap::<usize, BTreeSet<usize>>::new();
        for link in building
            .relations
            .iter()
            .filter(|r| r.human && r.kind == "assembly")
        {
            adjacent.entry(link.from).or_default().insert(link.to);
            adjacent.entry(link.to).or_default().insert(link.from);
        }
        let mut seen = BTreeSet::new();
        let mut groups = Vec::new();
        for &first in adjacent.keys() {
            if !seen.insert(first) {
                continue;
            }
            let mut pending = vec![first];
            let mut group = BTreeSet::from([first]);
            while let Some(member) = pending.pop() {
                for &other in &adjacent[&member] {
                    if seen.insert(other) {
                        group.insert(other);
                        pending.push(other);
                    }
                }
            }
            groups.push(group.into_iter().collect());
        }
        Ok(groups)
    }

    /// Extract an entire reviewed group as an object-only ordinary prefab. Bounds include actual
    /// rotated definition footprints; sparse terrain, support, unrelated members and rooms stay
    /// in the source. Runtime placement still uses the canonical prefab planner and edit gateway.
    pub fn extract_assembly(
        &self,
        member: usize,
        name: impl Into<String>,
    ) -> Result<Self, PrefabError> {
        let members = self
            .reviewed_assemblies()?
            .into_iter()
            .find(|g| g.contains(&member))
            .ok_or(PrefabError::NoReviewedAssembly { member })?;
        let building = self
            .building
            .as_ref()
            .ok_or(PrefabError::NoReviewedAssembly { member })?;
        let mut objects: Vec<_> = members.iter().map(|&i| self.objects[i].clone()).collect();
        let min_x = objects
            .iter()
            .map(|o| o.x)
            .min()
            .ok_or(PrefabError::Nothing)?;
        let min_y = objects
            .iter()
            .map(|o| o.y)
            .min()
            .ok_or(PrefabError::Nothing)?;
        let min_plane = objects
            .iter()
            .map(|o| o.plane)
            .min()
            .ok_or(PrefabError::Nothing)?;
        let mut max_x = 0;
        let mut max_y = 0;
        let mut max_plane = 0;
        for object in &mut objects {
            let kind = LocKind::new(object.kind).ok_or(PrefabError::BadValue {
                what: "object kind",
                value: u32::from(object.kind),
            })?;
            let (w, h) = try_object_footprint(
                ObjectId::new(object.id),
                kind,
                object.orientation,
                &|id, kind| {
                    building
                        .object_sizes
                        .get(&id.get())
                        .and_then(|&(w, h)| Some((u8::try_from(w).ok()?, u8::try_from(h).ok()?)))
                        .ok_or(PrefabError::MissingFootprint { object: id, kind })
                },
            )?;
            object.x -= min_x;
            object.y -= min_y;
            object.plane -= min_plane;
            max_x = max_x.max(i32::from(object.x) + w);
            max_y = max_y.max(i32::from(object.y) + h);
            max_plane = max_plane.max(object.plane + 1);
        }
        let overflow = || PrefabError::Building(BuildingPrefabError::CoordinateOverflow);
        let source = match &self.source {
            PrefabSource::Captured {
                tile_x,
                tile_y,
                plane,
            } => PrefabSource::Captured {
                tile_x: tile_x.checked_add(i32::from(min_x)).ok_or_else(overflow)?,
                tile_y: tile_y.checked_add(i32::from(min_y)).ok_or_else(overflow)?,
                plane: plane.checked_add(min_plane).ok_or_else(overflow)?,
            },
            other => other.clone(),
        };
        let mut tags = self.tags.clone();
        if !tags.iter().any(|t| t == "assembly") {
            tags.push("assembly".into());
        }
        let extracted = Self {
            format: PREFAB_FORMAT.into(),
            version: PREFAB_VERSION,
            name: name.into(),
            tags,
            notes: format!(
                "{}\nReviewed assembly from {} · {}",
                self.notes, self.name, building.provenance
            ),
            source,
            width: u16::try_from(max_x).map_err(|_| overflow())?,
            height: u16::try_from(max_y).map_err(|_| overflow())?,
            planes: max_plane,
            tiles: vec![],
            heights: vec![],
            objects,
            building: None,
        };
        extracted.validate()?;
        Ok(extracted)
    }
}

#[cfg(test)]
mod tests;
