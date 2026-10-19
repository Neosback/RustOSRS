//! Explicit building capture: terrain ownership and placement anchors are independent.
mod semantics;
use super::*;
use crate::MapQuery;
pub use semantics::{PrefabPortal, PrefabRoom, PrefabSemantics};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PrefabCell {
    pub x: u16,
    pub y: u16,
    pub plane: u8,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrefabRelation {
    /// Indices into the deterministically ordered prefab object occurrences.
    pub from: usize,
    pub to: usize,
    pub kind: String,
    pub evidence: String,
    pub human: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrefabBuilding {
    #[serde(default)]
    pub semantics: PrefabSemantics,
    pub floors: Vec<PrefabCell>,
    pub support: Vec<PrefabCell>,
    pub source_datum: i32,
    #[serde(default)]
    pub object_sizes: std::collections::BTreeMap<u32, (u16, u16)>,
    pub provenance: String,
    pub relations: Vec<PrefabRelation>,
    pub reviewed: bool,
    #[serde(default)]
    pub labels: std::collections::BTreeMap<String, String>,
}
/// A reviewed capture intent in canonical source coordinates. No picker references are stored.
#[derive(Debug, Clone)]
pub struct BuildingCaptureSpec {
    pub floors: BTreeSet<MapPosition>,
    pub support: BTreeSet<HeightVertexCoord>,
    pub objects: Vec<ObjectPlacement>,
    pub object_sizes: std::collections::BTreeMap<ObjectId, (u16, u16)>,
    pub provenance: String,
    pub relations: Vec<PrefabRelation>,
    pub reviewed: bool,
    pub labels: std::collections::BTreeMap<String, String>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildingPrefabError {
    Unreviewed,
    EmptyFloor,
    InvalidMask,
    InvalidRelation,
    MissingHeight(HeightVertexCoord),
    MissingObject(ObjectPlacement),
    IncompatibleGround(MapPosition),
    Occupied(ObjectPlacement),
    MissingDefinition(ObjectId),
    SteppedFoundation,
    CoordinateOverflow,
}
impl fmt::Display for BuildingPrefabError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreviewed => f.write_str("review and confirm building membership first"),
            Self::EmptyFloor => f.write_str("the building has no owned floor tiles"),
            Self::MissingHeight(c) => write!(
                f,
                "height coverage is missing at {c:?}; load the surrounding map"
            ),
            Self::MissingObject(p) => write!(f, "placement {p:?} changed; refresh the capture"),
            Self::IncompatibleGround(c) => write!(
                f,
                "bridge or unverified destination overlay at {c:?}; clear or review the site material first"
            ),
            Self::Occupied(p) => write!(f, "placement conflicts with {p:?}; review replacement"),
            Self::MissingDefinition(id) => write!(
                f,
                "object {} needs its definition before placement",
                id.get()
            ),
            Self::SteppedFoundation => f.write_str(
                "stepped foundations need reviewed support groups before automatic fitting",
            ),
            _ => write!(f, "invalid building metadata: {self:?}"),
        }
    }
}
impl std::error::Error for BuildingPrefabError {}
impl PrefabBuilding {
    pub(super) fn validate(&self, p: &Prefab) -> Result<(), BuildingPrefabError> {
        if !(2..=PREFAB_VERSION).contains(&p.version) || self.floors.is_empty() {
            return Err(BuildingPrefabError::InvalidMask);
        }
        self.semantics.validate(p, &self.floors)?;
        for (cells, extra) in [(&self.floors, 0), (&self.support, 1)] {
            let mut unique = BTreeSet::new();
            for c in cells {
                if c.x >= p.width + extra
                    || c.y >= p.height + extra
                    || c.plane >= p.planes
                    || !unique.insert(*c)
                {
                    return Err(BuildingPrefabError::InvalidMask);
                }
            }
        }
        let heights: BTreeSet<_> = p.heights.iter().map(|h| (h.x, h.y, h.plane)).collect();
        let support: BTreeSet<_> = self.support.iter().map(|h| (h.x, h.y, h.plane)).collect();
        for c in &self.support {
            if !heights.contains(&(c.x, c.y, c.plane)) {
                return Err(BuildingPrefabError::InvalidMask);
            }
        }
        for f in &self.floors {
            if !p
                .tiles
                .iter()
                .any(|t| (t.x, t.y, t.plane) == (f.x, f.y, f.plane))
            {
                return Err(BuildingPrefabError::InvalidMask);
            }
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                if !support.contains(&(f.x + dx, f.y + dy, f.plane)) {
                    return Err(BuildingPrefabError::InvalidMask);
                }
            }
        }
        for object in &p.objects {
            if self
                .object_sizes
                .get(&object.id)
                .is_none_or(|&(w, h)| w == 0 || h == 0 || w > 255 || h > 255)
            {
                return Err(BuildingPrefabError::MissingDefinition(ObjectId::new(
                    object.id,
                )));
            }
        }
        if self
            .relations
            .iter()
            .any(|r| r.from >= p.objects.len() || r.to >= p.objects.len() || r.from == r.to)
        {
            return Err(BuildingPrefabError::InvalidRelation);
        }
        Ok(())
    }
}
fn building_error(e: BuildingPrefabError) -> PrefabError {
    PrefabError::Building(e)
}

pub fn capture_building_prefab(
    query: &impl MapQuery,
    spec: &BuildingCaptureSpec,
    name: impl Into<String>,
) -> Result<Prefab, PrefabError> {
    if !spec.reviewed {
        return Err(building_error(BuildingPrefabError::Unreviewed));
    }
    let base = spec
        .floors
        .iter()
        .map(|c| c.plane.index().get())
        .min()
        .ok_or_else(|| building_error(BuildingPrefabError::EmptyFloor))?;
    for object in &spec.objects {
        if object.position.plane.index().get() < base {
            return Err(building_error(BuildingPrefabError::CoordinateOverflow));
        }
        if spec
            .object_sizes
            .get(&object.object_id)
            .is_none_or(|&(w, h)| w == 0 || h == 0 || w > 255 || h > 255)
        {
            return Err(building_error(BuildingPrefabError::MissingDefinition(
                object.object_id,
            )));
        }
    }
    if spec.support.iter().any(|c| c.plane.index().get() < base) {
        return Err(building_error(BuildingPrefabError::CoordinateOverflow));
    }
    let mut coords: Vec<MapTile> = spec
        .floors
        .iter()
        .map(|c| c.tile)
        .chain(spec.objects.iter().map(|o| o.position.tile))
        .collect();
    coords.extend(spec.support.iter().map(|c| c.corner));
    let min = MapTile::new(
        coords
            .iter()
            .map(|c| c.x)
            .min()
            .ok_or(PrefabError::Nothing)?,
        coords
            .iter()
            .map(|c| c.y)
            .min()
            .ok_or(PrefabError::Nothing)?,
    );
    // Tile anchors occupy a cell; support coordinates name vertices on its far boundary.
    let extent = |axis: fn(MapTile) -> i32| {
        spec.floors
            .iter()
            .map(|c| axis(c.tile) + 1)
            .chain(spec.objects.iter().map(|o| axis(o.position.tile) + 1))
            .chain(spec.support.iter().map(|c| axis(c.corner)))
            .max()
            .unwrap_or_else(|| axis(min) + 1)
            - axis(min)
    };
    let width = u16::try_from(extent(|t| t.x))
        .map_err(|_| building_error(BuildingPrefabError::CoordinateOverflow))?;
    let height = u16::try_from(extent(|t| t.y))
        .map_err(|_| building_error(BuildingPrefabError::CoordinateOverflow))?;
    let top = spec
        .floors
        .iter()
        .map(|c| c.plane.index().get())
        .chain(spec.support.iter().map(|c| c.plane.index().get()))
        .chain(spec.objects.iter().map(|o| o.position.plane.index().get()))
        .max()
        .unwrap_or(base);
    let local = |t: MapTile, p: SourcePlane| PrefabCell {
        x: (t.x - min.x) as u16,
        y: (t.y - min.y) as u16,
        plane: p.index().get() - base,
    };
    let mut p = Prefab {
        format: PREFAB_FORMAT.into(),
        version: PREFAB_VERSION,
        name: name.into(),
        tags: vec![],
        notes: String::new(),
        source: PrefabSource::Captured {
            tile_x: min.x,
            tile_y: min.y,
            plane: base,
        },
        width,
        height,
        planes: top - base + 1,
        tiles: vec![],
        heights: vec![],
        objects: vec![],
        building: None,
    };
    for &coord in &spec.floors {
        let c = local(coord.tile, coord.plane);
        let t = query.terrain_at(coord);
        // Explicit default tiles are retained: an owned floor may intentionally clear a channel.
        p.tiles.push(PrefabTile {
            x: c.x,
            y: c.y,
            plane: c.plane,
            underlay: t.underlay.map(FloorUnderlayId::get),
            overlay: t.overlay.map(|o| PrefabOverlay {
                id: o.id.get(),
                shape: o.shape.get(),
                rotation: o.rotation.get(),
            }),
            flags: t.flags,
        });
    }
    for &coord in &spec.support {
        let c = local(coord.corner, coord.plane);
        let value = query
            .height_at(coord)
            .ok_or_else(|| building_error(BuildingPrefabError::MissingHeight(coord)))?;
        p.heights.push(PrefabHeight {
            x: c.x,
            y: c.y,
            plane: c.plane,
            value,
        });
    }
    let mut objects = spec.objects.clone();
    objects.sort_by_key(|o| {
        (
            o.position.plane.index().get(),
            o.position.tile.y,
            o.position.tile.x,
            o.kind.get(),
            o.object_id.get(),
            o.orientation.get(),
        )
    });
    objects.dedup();
    for o in objects {
        if !query.contains_object(o) {
            return Err(building_error(BuildingPrefabError::MissingObject(o)));
        }
        let c = local(o.position.tile, o.position.plane);
        p.objects.push(PrefabObject {
            id: o.object_id.get(),
            x: c.x,
            y: c.y,
            plane: c.plane,
            kind: o.kind.get(),
            orientation: o.orientation.get(),
        });
    }
    let mut floor_vertices = BTreeSet::new();
    for floor in spec.floors.iter().filter(|c| c.plane.index().get() == base) {
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            let c = HeightVertexCoord::new(
                MapTile::new(floor.tile.x + dx, floor.tile.y + dy),
                floor.plane,
            );
            if !spec.support.contains(&c) {
                return Err(building_error(BuildingPrefabError::MissingHeight(c)));
            }
            floor_vertices.insert(c);
        }
    }
    let mut datum: Vec<_> = floor_vertices
        .iter()
        .filter_map(|&c| query.height_at(c))
        .collect();
    datum.sort_unstable();
    let source_datum = datum
        .get(datum.len() / 2)
        .copied()
        .ok_or_else(|| building_error(BuildingPrefabError::EmptyFloor))?;
    p.building = Some(PrefabBuilding {
        semantics: PrefabSemantics::default(),
        floors: spec.floors.iter().map(|c| local(c.tile, c.plane)).collect(),
        support: spec
            .support
            .iter()
            .map(|c| local(c.corner, c.plane))
            .collect(),
        source_datum,
        object_sizes: spec
            .object_sizes
            .iter()
            .map(|(id, &size)| (id.get(), size))
            .collect(),
        provenance: spec.provenance.clone(),
        relations: spec.relations.clone(),
        reviewed: true,
        labels: spec.labels.clone(),
    });
    p.tiles.sort_by_key(|c| (c.plane, c.y, c.x));
    p.heights.sort_by_key(|c| (c.plane, c.y, c.x));
    p.validate()?;
    Ok(p)
}

mod studio;
pub use studio::capture_prefab_over;

#[cfg(test)]
mod tests;
