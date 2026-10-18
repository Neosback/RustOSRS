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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dont_care_tiles: Vec<PrefabCell>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dont_care_heights: Vec<PrefabCell>,
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
        dont_care_tiles: vec![],
        dont_care_heights: vec![],
    })
}

/// Generate a procedural road/path network and sloped terrain task with a masked building plot.
pub fn generated_paths(seed: u64) -> Result<LabTask, String> {
    let (width, height, planes) = (20_u16, 20_u16, 1_u8);
    let plane0 = source_plane(0, planes)?;
    let base_underlay = 2_u32;
    let road_underlay = 10_u32;
    let road_overlay = 4_u32;
    let spur_overlay = 12_u32;

    let initial = Snapshot {
        tiles: (0..width)
            .flat_map(|x| {
                (0..height).map(move |y| PrefabTile {
                    x,
                    y,
                    plane: 0,
                    underlay: Some(base_underlay),
                    overlay: None,
                    flags: 0,
                })
            })
            .collect(),
        objects: vec![],
        heights: (0..=width)
            .flat_map(|x| {
                (0..=height).map(move |y| {
                    let ridge = ((i32::from(x) - 10).abs() <= 4 && (i32::from(y) - 10).abs() <= 4)
                        as i32
                        * -64;
                    PrefabHeight {
                        x,
                        y,
                        plane: 0,
                        value: ridge,
                    }
                })
            })
            .collect(),
    };
    let mut document = initial.document(width, height, planes)?;

    let offset = (seed % 3) as f64;
    let main_waypoints = [(2.5, 3.5 + offset), (9.5, 9.5), (17.5, 15.5 - offset)];
    let main_line = osrs_scene::terrain_path::smooth(&main_waypoints);
    let fitted_main = osrs_scene::terrain_path::fit_bounded(
        &main_line,
        3.0,
        1.5,
        (i32::from(width), i32::from(height)),
    );
    let main_intents: Vec<TerrainPathIntent> = fitted_main
        .into_iter()
        .map(|t| TerrainPathIntent {
            tile: MapTile::new(t.tile_x, t.tile_y),
            overlay: Some(road_overlay as u16),
            shape: t.shape,
            rotation: t.orientation,
        })
        .collect();
    let path_patch = build_terrain_path_patch_with_underlay(
        &document,
        plane0,
        &main_intents,
        Some(road_underlay as u16),
    )
    .map_err(|e| format!("{e:?}"))?;
    document
        .apply_patch(&path_patch)
        .map_err(|e| format!("{e:?}"))?;
    let planned_heights = plan_path_height_smoothing(
        &main_line,
        PathHeightSpec {
            width: 3.0,
            passes: 2,
            blend: 1.5,
        },
        &|(vx, vy)| document.height_at(HeightVertexCoord::new(MapTile::new(vx, vy), plane0)),
    );
    let smooth_patch = build_path_height_patch(&document, plane0, &planned_heights)
        .map_err(|e| format!("{e:?}"))?;
    document
        .apply_patch(&smooth_patch)
        .map_err(|e| format!("{e:?}"))?;

    let mut dont_care_tiles = Vec::new();
    let mut dont_care_heights = Vec::new();
    for x in 13..=16 {
        for y in 2..=5 {
            dont_care_tiles.push(PrefabCell { x, y, plane: 0 });
            dont_care_heights.push(PrefabCell { x, y, plane: 0 });
        }
    }

    let target = Snapshot::capture(&document);
    Ok(LabTask {
        format: "rustosrs-lab-task".into(),
        version: 1,
        name: format!("Paths seed {seed}"),
        source: "procedural-paths".into(),
        group: format!("paths:{seed}"),
        width,
        height,
        planes,
        budget: 512,
        initial,
        target,
        palette: vec![],
        overlays: vec![road_overlay, spur_overlay],
        underlays: vec![base_underlay, road_underlay],
        dont_care_tiles,
        dont_care_heights,
    })
}

/// Convert a captured building/region prefab into a terrain & path reconstruction task by
/// stripping objects and masking building interior floors as `dont_care_tiles`.
pub fn prefab_paths_task(prefab: &Prefab, _seed: u64) -> Result<LabTask, String> {
    prefab.validate().map_err(|e| e.to_string())?;
    let datum = prefab.building.as_ref().map_or(0, |b| b.source_datum);
    let dominant_underlay = prefab
        .tiles
        .iter()
        .filter(|t| t.plane == 0)
        .filter_map(|t| t.underlay)
        .fold(
            std::collections::BTreeMap::<u32, u32>::new(),
            |mut acc, u| {
                *acc.entry(u).or_insert(0) += 1;
                acc
            },
        )
        .into_iter()
        .max_by_key(|&(_, count)| count)
        .map(|(u, _)| u);

    let dont_care_tiles: Vec<PrefabCell> = prefab
        .building
        .as_ref()
        .map(|b| b.floors.iter().filter(|c| c.plane == 0).cloned().collect())
        .unwrap_or_default();
    let dont_care_set: std::collections::HashSet<(u16, u16, u8)> = dont_care_tiles
        .iter()
        .map(|c| (c.x, c.y, c.plane))
        .collect();
    let dont_care_heights = dont_care_tiles.clone();

    let initial = Snapshot {
        tiles: (0..prefab.width)
            .flat_map(|x| {
                (0..prefab.height).filter_map(move |y| {
                    dominant_underlay.map(|u| PrefabTile {
                        x,
                        y,
                        plane: 0,
                        underlay: Some(u),
                        overlay: None,
                        flags: 0,
                    })
                })
            })
            .collect(),
        objects: vec![],
        heights: (0..=prefab.width)
            .flat_map(|x| {
                (0..=prefab.height).map(move |y| PrefabHeight {
                    x,
                    y,
                    plane: 0,
                    value: datum,
                })
            })
            .collect(),
    };
    let mut target = initial.clone();
    target.tiles = prefab
        .tiles
        .iter()
        .filter(|t| t.plane == 0 && !dont_care_set.contains(&(t.x, t.y, 0)))
        .map(|t| {
            let mut clean = t.clone();
            clean.flags = 0;
            clean
        })
        .collect();
    for h in prefab.heights.iter().filter(|h| h.plane == 0) {
        if let Some(existing) = target
            .heights
            .iter_mut()
            .find(|v| (v.x, v.y, v.plane) == (h.x, h.y, 0))
        {
            *existing = h.clone();
        }
    }

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
        .chain(dominant_underlay)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();

    Ok(LabTask {
        format: "rustosrs-lab-task".into(),
        version: 1,
        name: format!("Paths {}", prefab.name),
        source: prefab.name.clone(),
        group: format!("paths:{}", prefab.name),
        width: prefab.width,
        height: prefab.height,
        planes: 1,
        budget: 1024,
        initial,
        target,
        palette: vec![],
        overlays,
        underlays,
        dont_care_tiles,
        dont_care_heights,
    })
}
