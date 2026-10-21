//! Private wire records. Canonical coordinates, IDs and value constructors remain semantic authority.
use super::SemanticTransportError as Error;
use crate::{
    HeightVertexCoord, LocKind, MapOp, MapPosition, ObjectOrientation, ObjectPlacement,
    OverlayRotation, OverlayShape, TerrainOverlay, TerrainTile,
};
use osrs_core::{
    coords::{MapTile, SourcePlane},
    ids::{FloorOverlayId, FloorUnderlayId, ObjectId},
};
use serde::{Deserialize, Serialize};

fn invalid(field: &'static str, value: u8) -> Error {
    Error::InvalidValue { field, value }
}
// Optional channels must be present (null means absent); a truncated record is not a complete tile.
fn required_option<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(d)
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PositionRecord {
    x: i32,
    y: i32,
    plane: u8,
}
impl PositionRecord {
    fn decode(self) -> Result<MapPosition, Error> {
        Ok(MapPosition::new(
            MapTile::new(self.x, self.y),
            SourcePlane::new(self.plane).ok_or_else(|| invalid("source plane", self.plane))?,
        ))
    }
}
impl From<MapPosition> for PositionRecord {
    fn from(p: MapPosition) -> Self {
        Self {
            x: p.tile.x,
            y: p.tile.y,
            plane: p.plane.index().get(),
        }
    }
}
impl From<HeightVertexCoord> for PositionRecord {
    fn from(c: HeightVertexCoord) -> Self {
        Self {
            x: c.corner.x,
            y: c.corner.y,
            plane: c.plane.index().get(),
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OverlayRecord {
    id: FloorOverlayId,
    shape: u8,
    rotation: u8,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TileRecord {
    #[serde(deserialize_with = "required_option")]
    underlay: Option<FloorUnderlayId>,
    #[serde(deserialize_with = "required_option")]
    overlay: Option<OverlayRecord>,
    flags: u8,
}
impl TileRecord {
    fn decode(self) -> Result<TerrainTile, Error> {
        let overlay = self
            .overlay
            .map(|o| {
                Ok::<_, Error>(TerrainOverlay {
                    id: o.id,
                    shape: OverlayShape::new(o.shape)
                        .ok_or_else(|| invalid("overlay shape", o.shape))?,
                    rotation: OverlayRotation::new(o.rotation)
                        .ok_or_else(|| invalid("overlay rotation", o.rotation))?,
                })
            })
            .transpose()?;
        Ok(TerrainTile {
            underlay: self.underlay,
            overlay,
            flags: self.flags,
        })
    }
}
impl From<TerrainTile> for TileRecord {
    fn from(t: TerrainTile) -> Self {
        Self {
            underlay: t.underlay,
            overlay: t.overlay.map(|o| OverlayRecord {
                id: o.id,
                shape: o.shape.get(),
                rotation: o.rotation.get(),
            }),
            flags: t.flags,
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TerrainRecord {
    position: PositionRecord,
    value: TileRecord,
}
impl From<(MapPosition, TerrainTile)> for TerrainRecord {
    fn from((position, value): (MapPosition, TerrainTile)) -> Self {
        Self {
            position: position.into(),
            value: value.into(),
        }
    }
}
impl TerrainRecord {
    pub(super) fn decode(self) -> Result<(MapPosition, TerrainTile), Error> {
        Ok((self.position.decode()?, self.value.decode()?))
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct HeightRecord {
    vertex: PositionRecord,
    value: i32,
}
impl From<(HeightVertexCoord, i32)> for HeightRecord {
    fn from((vertex, value): (HeightVertexCoord, i32)) -> Self {
        Self {
            vertex: vertex.into(),
            value,
        }
    }
}
impl HeightRecord {
    pub(super) fn decode(self) -> Result<(HeightVertexCoord, i32), Error> {
        let p = self.vertex.decode()?;
        Ok((HeightVertexCoord::new(p.tile, p.plane), self.value))
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ObjectRecord {
    id: ObjectId,
    position: PositionRecord,
    kind: u8,
    orientation: u8,
}
impl From<ObjectPlacement> for ObjectRecord {
    fn from(p: ObjectPlacement) -> Self {
        Self {
            id: p.object_id,
            position: p.position.into(),
            kind: p.kind.get(),
            orientation: p.orientation.get(),
        }
    }
}
impl ObjectRecord {
    pub(super) fn decode(self) -> Result<ObjectPlacement, Error> {
        Ok(ObjectPlacement {
            object_id: self.id,
            position: self.position.decode()?,
            kind: LocKind::new(self.kind).ok_or_else(|| invalid("loc kind", self.kind))?,
            orientation: ObjectOrientation::new(self.orientation)
                .ok_or_else(|| invalid("object orientation", self.orientation))?,
        })
    }
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum OperationRecord {
    SetTerrain {
        position: PositionRecord,
        before: TileRecord,
        after: TileRecord,
    },
    SetHeight {
        vertex: PositionRecord,
        before: i32,
        after: i32,
    },
    AddObject {
        placement: ObjectRecord,
    },
    RemoveObject {
        placement: ObjectRecord,
    },
}
impl From<MapOp> for OperationRecord {
    fn from(op: MapOp) -> Self {
        match op {
            MapOp::SetTerrain {
                coord,
                before,
                after,
            } => Self::SetTerrain {
                position: coord.into(),
                before: before.into(),
                after: after.into(),
            },
            MapOp::SetHeight {
                coord,
                before,
                after,
            } => Self::SetHeight {
                vertex: coord.into(),
                before,
                after,
            },
            MapOp::AddObject { placement } => Self::AddObject {
                placement: placement.into(),
            },
            MapOp::RemoveObject { placement } => Self::RemoveObject {
                placement: placement.into(),
            },
        }
    }
}
impl OperationRecord {
    pub(super) fn decode(self) -> Result<MapOp, Error> {
        Ok(match self {
            Self::SetTerrain {
                position,
                before,
                after,
            } => MapOp::SetTerrain {
                coord: position.decode()?,
                before: before.decode()?,
                after: after.decode()?,
            },
            Self::SetHeight {
                vertex,
                before,
                after,
            } => {
                let p = vertex.decode()?;
                MapOp::SetHeight {
                    coord: HeightVertexCoord::new(p.tile, p.plane),
                    before,
                    after,
                }
            }
            Self::AddObject { placement } => MapOp::AddObject {
                placement: placement.decode()?,
            },
            Self::RemoveObject { placement } => MapOp::RemoveObject {
                placement: placement.decode()?,
            },
        })
    }
}
