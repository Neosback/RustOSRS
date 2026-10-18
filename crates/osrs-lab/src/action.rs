//! Tool intents compile to the same pure planners as editor actions.
use crate::{
    LabTask,
    task::{placement, source_plane},
};
use osrs_core::{
    coords::MapTile,
    ids::{FloorOverlayId, FloorUnderlayId},
};
use osrs_edit::*;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
            } => {
                if *width < 2
                    || *height < 2
                    || u32::from(o.x) + u32::from(*width) > u32::from(task.width)
                    || u32::from(o.y) + u32::from(*height) > u32::from(task.height)
                    || o.kind != 0
                {
                    return Err("room rectangle requires a bounded straight-wall asset".into());
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
            Self::Undo | Self::Redo => Err("history actions are handled by the environment".into()),
        }
    }
}
