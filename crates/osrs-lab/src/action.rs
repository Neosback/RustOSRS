//! Tool intents compile to the same pure planners as editor actions.
use crate::{
    LabTask,
    task::{placement, source_plane},
};
use osrs_core::{
    coords::MapTile,
    ids::{FloorOverlayId, FloorUnderlayId, ObjectId},
};
use osrs_edit::{
    height::{Falloff, HeightOp, HeightTool},
    *,
};
use serde::{Deserialize, Serialize};

fn default_edge_weight() -> f64 {
    1.5
}

fn default_true() -> bool {
    true
}

fn default_blend() -> f64 {
    1.5
}

fn default_strength() -> i32 {
    16
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PathSmoothSpec {
    pub passes: u32,
    #[serde(default = "default_blend")]
    pub blend: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tool", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    PaintRect {
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        plane: u8,
        overlay: Option<u32>,
        underlay: Option<u32>,
        shape: u8,
        rotation: u8,
        flags: Option<u8>,
    },
    HeightStroke {
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        plane: u8,
        value: i32,
    },
    HeightBrush {
        plane: u8,
        points: Vec<[u16; 2]>,
        radius: u8,
        #[serde(default)]
        circle: bool,
        #[serde(default)]
        op: HeightOp,
        #[serde(default = "default_strength")]
        strength: i32,
        #[serde(default)]
        falloff: Falloff,
        #[serde(default)]
        flatten_to: Option<i32>,
        #[serde(default)]
        seed: u64,
    },
    PathLay {
        plane: u8,
        points: Vec<[f64; 2]>,
        width: f64,
        #[serde(default = "default_edge_weight")]
        edge_weight: f64,
        #[serde(default = "default_true")]
        smooth_curve: bool,
        overlay_id: u32,
        #[serde(default)]
        companion_underlay: Option<u32>,
        #[serde(default)]
        smooth_heights: Option<PathSmoothSpec>,
    },
    Place {
        object: PrefabObject,
    },
    Remove {
        object: PrefabObject,
    },
    Move {
        object: PrefabObject,
        dx: i16,
        dy: i16,
    },
    Turn {
        object: PrefabObject,
        turns: u8,
    },
    WallLine {
        object: PrefabObject,
        length: u16,
        vertical: bool,
    },
    RoomRect {
        object: PrefabObject,
        width: u16,
        height: u16,
        #[serde(default)]
        corner_id: Option<u32>,
    },
    HipRoof {
        plane: u8,
        x0: u16,
        y0: u16,
        x1: u16,
        y1: u16,
        slope_id: u32,
        #[serde(default)]
        edge_id: Option<u32>,
        #[serde(default)]
        ridge_id: Option<u32>,
    },
    Undo,
    Redo,
}
impl Action {
    pub(crate) fn plan(&self, document: &MapDocument, task: &LabTask) -> Result<MapPatch, String> {
        let within = |x: u16, y: u16| {
            if x < task.width && y < task.height {
                Ok(())
            } else {
                Err("action outside task".to_owned())
            }
        };
        let object = |o: &PrefabObject| -> Result<ObjectPlacement, String> {
            within(o.x, o.y)?;
            let palette = task
                .palette
                .iter()
                .find(|p| p.id == o.id && p.kind == o.kind)
                .ok_or("object outside task palette")?;
            let (w, h) = if o.orientation & 1 == 1 {
                (palette.size[1], palette.size[0])
            } else {
                (palette.size[0], palette.size[1])
            };
            if matches!(o.kind, 10 | 11)
                && (u32::from(o.x) + u32::from(w) > u32::from(task.width)
                    || u32::from(o.y) + u32::from(h) > u32::from(task.height))
            {
                return Err("footprint outside task".into());
            }
            placement(o, task.planes)
        };
        match self {
            Self::PaintRect {
                x,
                y,
                width,
                height,
                plane,
                overlay,
                underlay,
                shape,
                rotation,
                flags,
            } => {
                if *width == 0
                    || *height == 0
                    || u32::from(*x) + u32::from(*width) > u32::from(task.width)
                    || u32::from(*y) + u32::from(*height) > u32::from(task.height)
                {
                    return Err("paint rectangle outside task".into());
                }
                if overlay.is_some_and(|v| !task.overlays.contains(&v))
                    || underlay.is_some_and(|v| !task.underlays.contains(&v))
                {
                    return Err("floor outside task palette".into());
                }
                let plane = source_plane(*plane, task.planes)?;
                let shape = OverlayShape::new(*shape).ok_or("shape")?;
                let rotation = OverlayRotation::new(*rotation).ok_or("rotation")?;
                let intents = (*x..*x + *width)
                    .flat_map(|x| {
                        (*y..*y + *height).map(move |y| TerrainPaintIntent {
                            tile: MapTile::new(i32::from(x), i32::from(y)),
                            underlay: Some(underlay.map(FloorUnderlayId::new)),
                            overlay: Some(overlay.map(FloorOverlayId::new)),
                            overlay_shape: Some((shape, rotation)),
                            flags: *flags,
                            height: None,
                        })
                    })
                    .collect::<Vec<_>>();
                build_terrain_paint_patch(document, plane, &intents).map_err(|e| format!("{e:?}"))
            }
            Self::HeightStroke {
                x,
                y,
                width,
                height,
                plane,
                value,
            } => {
                if *width == 0
                    || *height == 0
                    || u32::from(*x) + u32::from(*width) > u32::from(task.width) + 1
                    || u32::from(*y) + u32::from(*height) > u32::from(task.height) + 1
                    || !(-32768..=32768).contains(value)
                {
                    return Err("height stroke exceeds task bounds".into());
                }
                let intents = (*x..*x + *width)
                    .flat_map(|x| {
                        (*y..*y + *height).map(move |y| {
                            let mut i =
                                TerrainPaintIntent::new(MapTile::new(i32::from(x), i32::from(y)));
                            i.height = Some(*value);
                            i
                        })
                    })
                    .collect::<Vec<_>>();
                build_terrain_paint_patch(document, source_plane(*plane, task.planes)?, &intents)
                    .map_err(|e| format!("{e:?}"))
            }
            Self::HeightBrush {
                plane,
                points,
                radius,
                circle,
                op,
                strength,
                falloff,
                flatten_to,
                seed,
            } => {
                if points.is_empty() || *radius > 16 {
                    return Err("height brush requires bounded radius and points".into());
                }
                let source_plane = source_plane(*plane, task.planes)?;
                let mut centers = Vec::with_capacity(points.len());
                for &[px, py] in points {
                    if px > task.width || py > task.height {
                        return Err("height brush point outside task".into());
                    }
                    centers.push(MapTile::new(i32::from(px), i32::from(py)));
                }
                let anchor = document
                    .height_at(HeightVertexCoord::new(centers[0], source_plane))
                    .unwrap_or(0);
                let shape = if *circle {
                    BrushShape::Circle
                } else {
                    BrushShape::Square
                };
                let spec = HeightStrokeSpec::new(
                    source_plane,
                    HeightTool {
                        op: *op,
                        strength: *strength,
                        falloff: *falloff,
                        flatten_to: *flatten_to,
                    },
                    *radius,
                    shape,
                    anchor,
                    *seed,
                );
                let raw_patch = build_terrain_height_patch(document, &spec, &centers)
                    .map_err(|e| format!("{e:?}"))?;
                let ops: Vec<MapOp> = raw_patch
                    .ops()
                    .iter()
                    .copied()
                    .filter(|op| match op {
                        MapOp::SetHeight { coord, .. } => {
                            coord.corner.x >= 0
                                && coord.corner.y >= 0
                                && coord.corner.x <= i32::from(task.width)
                                && coord.corner.y <= i32::from(task.height)
                        }
                        _ => true,
                    })
                    .collect();
                Ok(MapPatch::new(ops))
            }
            Self::PathLay {
                plane,
                points,
                width,
                edge_weight,
                smooth_curve,
                overlay_id,
                companion_underlay,
                smooth_heights,
            } => {
                if points.len() < 2
                    || !width.is_finite()
                    || *width <= 0.0
                    || *width > 16.0
                    || !edge_weight.is_finite()
                    || *edge_weight < 0.0
                {
                    return Err("invalid path_lay parameters".into());
                }
                if !task.overlays.contains(overlay_id)
                    || companion_underlay.is_some_and(|u| !task.underlays.contains(&u))
                {
                    return Err("path floor outside task palette".into());
                }
                let source_plane = source_plane(*plane, task.planes)?;
                let pts: Vec<(f64, f64)> = points.iter().map(|&[x, y]| (x, y)).collect();
                if pts
                    .iter()
                    .any(|&(x, y)| !x.is_finite() || !y.is_finite() || x < 0.0 || y < 0.0)
                {
                    return Err("path_lay points must be finite and non-negative".into());
                }
                let line = if *smooth_curve {
                    osrs_scene::terrain_path::smooth(&pts)
                } else {
                    pts
                };
                let fitted = osrs_scene::terrain_path::fit_bounded(
                    &line,
                    *width,
                    *edge_weight,
                    (i32::from(task.width), i32::from(task.height)),
                );
                let ov_u16 = u16::try_from(*overlay_id).map_err(|_| "overlay id overflow")?;
                let comp_u16 = companion_underlay
                    .map(u16::try_from)
                    .transpose()
                    .map_err(|_| "underlay id overflow")?;
                let intents: Vec<TerrainPathIntent> = fitted
                    .into_iter()
                    .filter(|t| {
                        t.tile_x >= 0
                            && t.tile_y >= 0
                            && t.tile_x < i32::from(task.width)
                            && t.tile_y < i32::from(task.height)
                    })
                    .map(|t| TerrainPathIntent {
                        tile: MapTile::new(t.tile_x, t.tile_y),
                        overlay: Some(ov_u16),
                        shape: t.shape,
                        rotation: t.orientation,
                    })
                    .collect();
                if intents.is_empty() {
                    return Err("path_lay produced no in-bounds tiles".into());
                }
                let path_patch = build_terrain_path_patch_with_underlay(
                    document,
                    source_plane,
                    &intents,
                    comp_u16,
                )
                .map_err(|e| format!("{e:?}"))?;
                let mut ops = path_patch.ops().to_vec();
                if let Some(smooth_spec) = smooth_heights
                    && smooth_spec.passes > 0
                    && smooth_spec.passes <= 16
                    && smooth_spec.blend.is_finite()
                    && smooth_spec.blend >= 0.0
                {
                    let planned = plan_path_height_smoothing(
                        &line,
                        PathHeightSpec {
                            width: *width,
                            passes: smooth_spec.passes,
                            blend: smooth_spec.blend,
                        },
                        &|(vx, vy)| {
                            document.height_at(HeightVertexCoord::new(
                                MapTile::new(vx, vy),
                                source_plane,
                            ))
                        },
                    );
                    let raw_heights = build_path_height_patch(document, source_plane, &planned)
                        .map_err(|e| format!("{e:?}"))?;
                    for op in raw_heights.ops() {
                        if let MapOp::SetHeight { coord, .. } = op
                            && coord.corner.x >= 0
                            && coord.corner.y >= 0
                            && coord.corner.x <= i32::from(task.width)
                            && coord.corner.y <= i32::from(task.height)
                        {
                            ops.push(*op);
                        }
                    }
                }
                Ok(MapPatch::new(ops))
            }
            Self::Place { object: o } => {
                build_object_patch(document, &[], &[object(o)?]).map_err(|e| format!("{e:?}"))
            }
            Self::Remove { object: o } => {
                build_object_patch(document, &[object(o)?], &[]).map_err(|e| format!("{e:?}"))
            }
            Self::Move { object: o, dx, dy } => {
                let mut to = o.clone();
                to.x = u16::try_from(i32::from(o.x) + i32::from(*dx)).map_err(|_| "move x")?;
                to.y = u16::try_from(i32::from(o.y) + i32::from(*dy)).map_err(|_| "move y")?;
                build_object_patch(document, &[object(o)?], &[object(&to)?])
                    .map_err(|e| format!("{e:?}"))
            }
            Self::Turn { object: o, turns } => {
                let mut to = o.clone();
                to.orientation = (o.orientation + turns % 4) % 4;
                build_object_patch(document, &[object(o)?], &[object(&to)?])
                    .map_err(|e| format!("{e:?}"))
            }
            Self::WallLine {
                object: o,
                length,
                vertical,
            } => {
                if *length == 0 || *length > task.width.max(task.height) || !matches!(o.kind, 0 | 2)
                {
                    return Err("wall line requires a boundary piece and bounded length".into());
                }
                let mut additions = Vec::new();
                for n in 0..*length {
                    let mut p = o.clone();
                    if *vertical {
                        p.y = p.y.checked_add(n).ok_or("wall y")?
                    } else {
                        p.x = p.x.checked_add(n).ok_or("wall x")?
                    }
                    additions.push(object(&p)?)
                }
                build_object_patch(document, &[], &additions).map_err(|e| format!("{e:?}"))
            }
            Self::RoomRect {
                object: o,
                width,
                height,
                corner_id,
            } => {
                if *width < 2
                    || *height < 2
                    || u32::from(o.x) + u32::from(*width) > u32::from(task.width)
                    || u32::from(o.y) + u32::from(*height) > u32::from(task.height)
                    || o.kind != 0
                {
                    return Err("room rectangle requires a bounded straight-wall asset".into());
                }
                if let Some(cid) = corner_id {
                    let mut additions = Vec::new();
                    for x in 1..(*width - 1) {
                        for (y, r) in [(0, 3), (*height - 1, 1)] {
                            let mut p = o.clone();
                            p.x += x;
                            p.y += y;
                            p.orientation = r;
                            additions.push(object(&p)?);
                        }
                    }
                    for y in 1..(*height - 1) {
                        for (x, r) in [(0, 0), (*width - 1, 2)] {
                            let mut p = o.clone();
                            p.x += x;
                            p.y += y;
                            p.orientation = r;
                            additions.push(object(&p)?);
                        }
                    }
                    for (dx, dy, r) in [
                        (0, *height - 1, 0),
                        (*width - 1, *height - 1, 1),
                        (*width - 1, 0, 2),
                        (0, 0, 3),
                    ] {
                        let p = PrefabObject {
                            id: *cid,
                            x: o.x + dx,
                            y: o.y + dy,
                            plane: o.plane,
                            kind: 2,
                            orientation: r,
                        };
                        additions.push(object(&p)?);
                    }
                    return build_object_patch(document, &[], &additions)
                        .map_err(|e| format!("{e:?}"));
                }
                let mut additions = Vec::new();
                for x in 0..*width {
                    for (y, r) in [(0, 3), (*height - 1, 1)] {
                        let mut p = o.clone();
                        p.x += x;
                        p.y += y;
                        p.orientation = r;
                        additions.push(object(&p)?);
                    }
                }
                for y in 0..*height {
                    for (x, r) in [(0, 0), (*width - 1, 2)] {
                        let mut p = o.clone();
                        p.x += x;
                        p.y += y;
                        p.orientation = r;
                        additions.push(object(&p)?);
                    }
                }
                let patch =
                    build_object_patch(document, &[], &additions).map_err(|e| format!("{e:?}"))?;
                let (patch, unplaced) = resolve_wall_slots(&patch);
                if unplaced != 0 {
                    return Err("room corners need compatible corner assets".into());
                }
                Ok(patch)
            }
            Self::HipRoof {
                plane,
                x0,
                y0,
                x1,
                y1,
                slope_id,
                edge_id,
                ridge_id,
            } => {
                if *x0 > *x1 || *y0 > *y1 || *x1 >= task.width || *y1 >= task.height {
                    return Err("roof rectangle outside task".into());
                }
                let source_plane = source_plane(*plane, task.planes)?;
                let mut additions = Vec::new();
                if let Some(edge) = edge_id {
                    let mut footprint = Vec::new();
                    for x in *x0..=*x1 {
                        for y in *y0..=*y1 {
                            footprint.push(MapTile::new(i32::from(x), i32::from(y)));
                        }
                    }
                    for placement in hip_roof_placements(
                        &footprint,
                        HipRoofAssets {
                            slope: ObjectId::new(*slope_id),
                            edge: ObjectId::new(*edge),
                        },
                        source_plane,
                    ) {
                        let p = PrefabObject {
                            id: placement.object_id.get(),
                            x: placement.position.tile.x as u16,
                            y: placement.position.tile.y as u16,
                            plane: *plane,
                            kind: placement.kind.get(),
                            orientation: placement.orientation.get(),
                        };
                        additions.push(object(&p)?);
                    }
                } else {
                    let span_x = *x1 - *x0 + 1;
                    let span_y = *y1 - *y0 + 1;
                    let east_west_ridge = span_x >= span_y;
                    let mid_y = *y0 + (span_y - 1) / 2;
                    let mid_x = *x0 + (span_x - 1) / 2;
                    for x in *x0..=*x1 {
                        for y in *y0..=*y1 {
                            let (id, kind, orientation) = if east_west_ridge {
                                if span_y % 2 == 1 && y == mid_y {
                                    if let Some(rid) = ridge_id {
                                        (*rid, 13_u8, 0_u8)
                                    } else {
                                        (*slope_id, 12_u8, 0_u8)
                                    }
                                } else if y > mid_y {
                                    (*slope_id, 12_u8, 0_u8)
                                } else {
                                    (*slope_id, 12_u8, 2_u8)
                                }
                            } else if span_x % 2 == 1 && x == mid_x {
                                if let Some(rid) = ridge_id {
                                    (*rid, 13_u8, 1_u8)
                                } else {
                                    (*slope_id, 12_u8, 1_u8)
                                }
                            } else if x > mid_x {
                                (*slope_id, 12_u8, 1_u8)
                            } else {
                                (*slope_id, 12_u8, 3_u8)
                            };
                            additions.push(object(&PrefabObject {
                                id,
                                x,
                                y,
                                plane: *plane,
                                kind,
                                orientation,
                            })?);
                        }
                    }
                }
                build_object_patch(document, &[], &additions).map_err(|e| format!("{e:?}"))
            }
            Self::Undo | Self::Redo => Err("history actions are handled by the environment".into()),
        }
    }
}
