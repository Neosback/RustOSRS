//! Portable, source-stamped tasks; source snapshots are infrastructure input, never agent actions.
use osrs_core::{
    coords::{MapTile, SourcePlane},
    ids::{FloorOverlayId, FloorUnderlayId, ObjectId},
};
use osrs_edit::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub tiles: Vec<PrefabTile>,
    pub heights: Vec<PrefabHeight>,
    pub objects: Vec<PrefabObject>,
}
impl Snapshot {
    pub fn capture(document: &MapDocument) -> Self {
        Self {
            tiles: document
                .terrain_entries()
                .map(|(p, t)| PrefabTile {
                    x: p.tile.x as u16,
                    y: p.tile.y as u16,
                    plane: p.plane.index().get(),
                    underlay: t.underlay.map(FloorUnderlayId::get),
                    overlay: t.overlay.map(|o| PrefabOverlay {
                        id: o.id.get(),
                        shape: o.shape.get(),
                        rotation: o.rotation.get(),
                    }),
                    flags: t.flags,
                })
                .collect(),
            heights: document
                .height_entries()
                .map(|(c, value)| PrefabHeight {
                    x: c.corner.x as u16,
                    y: c.corner.y as u16,
                    plane: c.plane.index().get(),
                    value,
                })
                .collect(),
            objects: document
                .object_entries()
                .iter()
                .map(|o| PrefabObject {
                    id: o.object_id.get(),
                    x: o.position.tile.x as u16,
                    y: o.position.tile.y as u16,
                    plane: o.position.plane.index().get(),
                    kind: o.kind.get(),
                    orientation: o.orientation.get(),
                })
                .collect(),
        }
    }
    pub(crate) fn document(
        &self,
        width: u16,
        height: u16,
        planes: u8,
    ) -> Result<MapDocument, String> {
        let mut terrain = Vec::new();
        let mut heights = Vec::new();
        let mut objects = Vec::new();
        for t in &self.tiles {
            if t.x >= width || t.y >= height {
                return Err("terrain outside task".into());
            }
            let plane = source_plane(t.plane, planes)?;
            let overlay = t
                .overlay
                .as_ref()
                .map(|o| {
                    Ok::<_, String>(TerrainOverlay {
                        id: FloorOverlayId::new(o.id),
                        shape: OverlayShape::new(o.shape).ok_or("overlay shape")?,
                        rotation: OverlayRotation::new(o.rotation).ok_or("overlay rotation")?,
                    })
                })
                .transpose()?;
            terrain.push((
                MapPosition::new(MapTile::new(i32::from(t.x), i32::from(t.y)), plane),
                TerrainTile {
                    underlay: t.underlay.map(FloorUnderlayId::new),
                    overlay,
                    flags: t.flags,
                },
            ));
        }
        for h in &self.heights {
            if h.x > width || h.y > height {
                return Err("height outside task".into());
            }
            heights.push((
                HeightVertexCoord::new(
                    MapTile::new(i32::from(h.x), i32::from(h.y)),
                    source_plane(h.plane, planes)?,
                ),
                h.value,
            ));
        }
        for o in &self.objects {
            if o.x >= width || o.y >= height {
                return Err("object outside task".into());
            }
            objects.push(placement(o, planes)?);
        }
        Ok(MapDocument::from_parts(terrain, heights, objects))
    }
}
pub(crate) fn source_plane(p: u8, planes: u8) -> Result<SourcePlane, String> {
    if p >= planes {
        return Err("plane outside task".into());
    }
    SourcePlane::new(p).ok_or_else(|| "invalid plane".into())
}
pub(crate) fn placement(o: &PrefabObject, planes: u8) -> Result<ObjectPlacement, String> {
    Ok(ObjectPlacement {
        object_id: ObjectId::new(o.id),
        position: MapPosition::new(
            MapTile::new(i32::from(o.x), i32::from(o.y)),
            source_plane(o.plane, planes)?,
        ),
        kind: LocKind::new(o.kind).ok_or("object kind")?,
        orientation: ObjectOrientation::new(o.orientation).ok_or("object orientation")?,
    })
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaletteEntry {
    pub id: u32,
    pub kind: u8,
    pub size: [u8; 2],
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabTask {
    pub format: String,
    pub version: u32,
    pub name: String,
    pub source: String,
    pub group: String,
    pub width: u16,
    pub height: u16,
    pub planes: u8,
    pub budget: u32,
    pub initial: Snapshot,
    pub target: Snapshot,
    pub palette: Vec<PaletteEntry>,
    pub overlays: Vec<u32>,
    pub underlays: Vec<u32>,
}

/// Cache-free first curriculum task using the same BuildingKit planner as the editor.
pub fn generated_tavern(kit: &KitFile, seed: u64) -> Result<LabTask, String> {
    let (width, height, planes) = (24, 24, 2);
    let plane = source_plane(0, planes)?;
    let heights = (0..planes)
        .flat_map(|p| {
            (0..=width).flat_map(move |x| {
                (0..=height).map(move |y| PrefabHeight {
                    x,
                    y,
                    plane: p,
                    value: -i32::from(p) * 240,
                })
            })
        })
        .collect();
    let initial = Snapshot {
        heights,
        ..Snapshot::default()
    };
    let mut document = initial.document(width, height, planes)?;
    let request = BuildingRequest {
        site: RectSite::new(MapTile::new(2, 2), 20, 20).ok_or("site")?,
        archetype: BuildingArchetype::Tavern,
        style: BuildingStyle::Lumbridge,
        seed,
        base_plane: plane,
        floors: 1,
        constraints: BuildingConstraints::default(),
    };
    let snapshot = capture_generation_snapshot(&document, request, []);
    let patch = kit
        .compile()
        .map_err(|e| e.to_string())?
        .generate(&snapshot, 0)
        .map_err(|e| format!("{e:?}"))?;
    document.apply_patch(&patch).map_err(|e| format!("{e:?}"))?;
    let target = Snapshot::capture(&document);
    let palette = target
        .objects
        .iter()
        .map(|o| {
            (
                (o.id, o.kind),
                PaletteEntry {
                    id: o.id,
                    kind: o.kind,
                    size: kit
                        .finish
                        .as_ref()
                        .and_then(|f| f.furniture.values().flatten().find(|f| f.id == o.id))
                        .map_or([1, 1], |f| f.size),
                },
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>()
        .into_values()
        .collect();
    let overlays = target
        .tiles
        .iter()
        .filter_map(|t| t.overlay.as_ref().map(|o| o.id))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let underlays = target
        .tiles
        .iter()
        .filter_map(|t| t.underlay)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    Ok(LabTask {
        format: "rustosrs-lab-task".into(),
        version: 1,
        name: "Reconstruct a tavern".into(),
        source: format!("BuildingKit {}", kit.name),
        group: format!("generated:{}", kit.name),
        width,
        height,
        planes,
        budget: 1024,
        initial,
        target,
        palette,
        overlays,
        underlays,
    })
}
