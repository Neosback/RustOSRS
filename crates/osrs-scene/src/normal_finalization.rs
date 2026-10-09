//! Scene-local ModelData ownership and exact NORMALS-003 reconciliation traversal.
//!
//! M6 owns semantic placement/storage. This module owns the mutable scene-local
//! `WorkingModel` copies that survive initial construction for normal
//! reconciliation. It mirrors the pinned `Scene.method5585`, `method5586`, and
//! `method5587` traversal without performing final ModelData -> Model lighting.

use crate::placement::Footprint;
use osrs_core::{
    coords::{LOCAL_UNITS_PER_TILE, SceneTile, StoragePlane},
    definitions::ModelTranslation,
    model::WorkingModel,
    normals::{NormalMergeOutcome, merge_model_normals},
};
use std::{error::Error, fmt};

const GAME_OBJECTS_PER_TILE: usize = 5;
const HALF_TILE: i32 = LOCAL_UNITS_PER_TILE / 2;

/// Arena identity for one scene-local mutable ModelData/WorkingModel instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SceneModelDataId(usize);

impl SceneModelDataId {
    pub const fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug)]
struct SceneModelDataEntry {
    model: WorkingModel,
    claimed: bool,
    pending: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundaryModelData {
    pub primary: SceneModelDataId,
    pub secondary: Option<SceneModelDataId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameObjectModelData {
    pub model: SceneModelDataId,
    pub start: SceneTile,
    pub end: SceneTile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FloorDecorationModelData {
    pub model: SceneModelDataId,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct SceneModelDataTile {
    boundary: Option<BoundaryModelData>,
    floor_decoration: Option<FloorDecorationModelData>,
    game_objects: Vec<GameObjectModelData>,
}

/// Exact aggregate result from one scene normal-reconciliation pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SceneNormalMergeReport {
    merge_calls: usize,
    matched_vertex_pairs: usize,
    hidden_faces: usize,
    closed_models: usize,
}

impl SceneNormalMergeReport {
    pub const fn merge_calls(self) -> usize {
        self.merge_calls
    }

    pub const fn matched_vertex_pairs(self) -> usize {
        self.matched_vertex_pairs
    }

    pub const fn hidden_faces(self) -> usize {
        self.hidden_faces
    }

    pub const fn closed_models(self) -> usize {
        self.closed_models
    }

    fn record_merge(&mut self, outcome: NormalMergeOutcome) {
        self.merge_calls += 1;
        self.matched_vertex_pairs += outcome.matched_vertex_pairs();
        self.hidden_faces += outcome.hidden_left_faces() + outcome.hidden_right_faces();
    }
}

/// Construction or traversal failure for the scene-local ModelData layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneNormalError {
    InvalidPlaneCount(u8),
    CapacityOverflow,
    CoordinateOverflow,
    OutOfBounds { plane: u8, x: u32, y: u32 },
    HeightCornerOutOfBounds { plane: u8, x: u32, y: u32 },
    UnknownModel(usize),
    ModelAlreadyClaimed(usize),
    DuplicateBoundaryArm(usize),
    OccupiedBoundary { plane: u8, x: u32, y: u32 },
    OccupiedFloorDecoration { plane: u8, x: u32, y: u32 },
    InvalidFootprint,
    GameObjectCapacity { plane: u8, x: u32, y: u32 },
}

impl fmt::Display for SceneNormalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPlaneCount(count) => {
                write!(
                    formatter,
                    "normal scene plane count {count} is outside 1..=4"
                )
            }
            Self::CapacityOverflow => formatter.write_str("normal scene capacity overflow"),
            Self::CoordinateOverflow => formatter.write_str("normal scene coordinate overflow"),
            Self::OutOfBounds { plane, x, y } => {
                write!(
                    formatter,
                    "normal scene tile ({x},{y}) on plane {plane} is out of bounds"
                )
            }
            Self::HeightCornerOutOfBounds { plane, x, y } => write!(
                formatter,
                "normal scene height corner ({x},{y}) on plane {plane} is out of bounds"
            ),
            Self::UnknownModel(index) => write!(formatter, "unknown scene ModelData id {index}"),
            Self::ModelAlreadyClaimed(index) => {
                write!(
                    formatter,
                    "scene ModelData id {index} is already owned by a renderable"
                )
            }
            Self::DuplicateBoundaryArm(index) => {
                write!(
                    formatter,
                    "boundary arms cannot share scene ModelData id {index}"
                )
            }
            Self::OccupiedBoundary { plane, x, y } => {
                write!(
                    formatter,
                    "boundary slot ({x},{y}) on plane {plane} is already occupied"
                )
            }
            Self::OccupiedFloorDecoration { plane, x, y } => write!(
                formatter,
                "floor-decoration slot ({x},{y}) on plane {plane} is already occupied"
            ),
            Self::InvalidFootprint => formatter.write_str("game-object footprint must be non-zero"),
            Self::GameObjectCapacity { plane, x, y } => write!(
                formatter,
                "game-object capacity reached at ({x},{y}) on plane {plane}"
            ),
        }
    }
}

impl Error for SceneNormalError {}

/// Scene-local mutable ModelData layer used only until normal reconciliation is closed.
///
/// The placement grid and this renderable layer remain separate on purpose: M6
/// placement is canonical semantic state, while these `WorkingModel` values are
/// per-build mutable copies that must never mutate the shared source-model cache.
#[derive(Debug)]
pub struct SceneModelDataGrid {
    width: u32,
    height: u32,
    plane_count: u8,
    plane_len: usize,
    height_plane_len: usize,
    tiles: Vec<SceneModelDataTile>,
    tile_heights: Vec<i32>,
    models: Vec<SceneModelDataEntry>,
}

impl SceneModelDataGrid {
    pub fn new(width: u32, height: u32, plane_count: u8) -> Result<Self, SceneNormalError> {
        if !(1..=4).contains(&plane_count) {
            return Err(SceneNormalError::InvalidPlaneCount(plane_count));
        }
        if width > i32::MAX as u32 || height > i32::MAX as u32 {
            return Err(SceneNormalError::CoordinateOverflow);
        }

        let width_usize = usize::try_from(width).map_err(|_| SceneNormalError::CapacityOverflow)?;
        let height_usize =
            usize::try_from(height).map_err(|_| SceneNormalError::CapacityOverflow)?;
        let plane_len = width_usize
            .checked_mul(height_usize)
            .ok_or(SceneNormalError::CapacityOverflow)?;
        let tile_len = plane_len
            .checked_mul(usize::from(plane_count))
            .ok_or(SceneNormalError::CapacityOverflow)?;

        let height_width = width_usize
            .checked_add(1)
            .ok_or(SceneNormalError::CapacityOverflow)?;
        let height_depth = height_usize
            .checked_add(1)
            .ok_or(SceneNormalError::CapacityOverflow)?;
        let height_plane_len = height_width
            .checked_mul(height_depth)
            .ok_or(SceneNormalError::CapacityOverflow)?;
        let height_len = height_plane_len
            .checked_mul(usize::from(plane_count))
            .ok_or(SceneNormalError::CapacityOverflow)?;

        Ok(Self {
            width,
            height,
            plane_count,
            plane_len,
            height_plane_len,
            tiles: vec![SceneModelDataTile::default(); tile_len],
            tile_heights: vec![0; height_len],
            models: Vec::new(),
        })
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub const fn plane_count(&self) -> u8 {
        self.plane_count
    }

    pub fn add_model(&mut self, model: WorkingModel) -> SceneModelDataId {
        let id = SceneModelDataId(self.models.len());
        self.models.push(SceneModelDataEntry {
            model,
            claimed: false,
            pending: true,
        });
        id
    }

    pub fn model(&self, id: SceneModelDataId) -> Option<&WorkingModel> {
        self.models.get(id.0).map(|entry| &entry.model)
    }

    pub fn is_pending_model_data(&self, id: SceneModelDataId) -> Option<bool> {
        self.models.get(id.0).map(|entry| entry.pending)
    }

    pub fn set_height_corner(
        &mut self,
        plane: StoragePlane,
        x: u32,
        y: u32,
        height: i32,
    ) -> Result<(), SceneNormalError> {
        let Some(index) = self.height_index(plane.index().get(), x, y) else {
            return Err(SceneNormalError::HeightCornerOutOfBounds {
                plane: plane.index().get(),
                x,
                y,
            });
        };
        self.tile_heights[index] = height;
        Ok(())
    }

    pub fn set_boundary(
        &mut self,
        plane: StoragePlane,
        tile: SceneTile,
        primary: SceneModelDataId,
        secondary: Option<SceneModelDataId>,
    ) -> Result<(), SceneNormalError> {
        let plane_value = plane.index().get();
        let tile_index = self.required_tile_index(plane_value, tile)?;
        if self.tiles[tile_index].boundary.is_some() {
            return Err(SceneNormalError::OccupiedBoundary {
                plane: plane_value,
                x: tile.x,
                y: tile.y,
            });
        }
        if secondary == Some(primary) {
            return Err(SceneNormalError::DuplicateBoundaryArm(primary.0));
        }
        self.ensure_unclaimed(primary)?;
        if let Some(secondary) = secondary {
            self.ensure_unclaimed(secondary)?;
        }

        self.models[primary.0].claimed = true;
        if let Some(secondary) = secondary {
            self.models[secondary.0].claimed = true;
        }
        self.tiles[tile_index].boundary = Some(BoundaryModelData { primary, secondary });
        Ok(())
    }

    pub fn set_floor_decoration(
        &mut self,
        plane: StoragePlane,
        tile: SceneTile,
        model: SceneModelDataId,
    ) -> Result<(), SceneNormalError> {
        let plane_value = plane.index().get();
        let tile_index = self.required_tile_index(plane_value, tile)?;
        if self.tiles[tile_index].floor_decoration.is_some() {
            return Err(SceneNormalError::OccupiedFloorDecoration {
                plane: plane_value,
                x: tile.x,
                y: tile.y,
            });
        }
        self.ensure_unclaimed(model)?;
        self.models[model.0].claimed = true;
        self.tiles[tile_index].floor_decoration = Some(FloorDecorationModelData { model });
        Ok(())
    }

    pub fn insert_game_object(
        &mut self,
        plane: StoragePlane,
        start: SceneTile,
        footprint: Footprint,
        model: SceneModelDataId,
    ) -> Result<(), SceneNormalError> {
        if footprint.width == 0 || footprint.depth == 0 {
            return Err(SceneNormalError::InvalidFootprint);
        }
        let end_x = start
            .x
            .checked_add(u32::from(footprint.width) - 1)
            .ok_or(SceneNormalError::CoordinateOverflow)?;
        let end_y = start
            .y
            .checked_add(u32::from(footprint.depth) - 1)
            .ok_or(SceneNormalError::CoordinateOverflow)?;
        let end = SceneTile::new(end_x, end_y);
        let plane_value = plane.index().get();

        self.required_tile_index(plane_value, start)?;
        self.required_tile_index(plane_value, end)?;
        self.ensure_unclaimed(model)?;

        for x in start.x..=end.x {
            for y in start.y..=end.y {
                let tile_index = self.required_tile_index(plane_value, SceneTile::new(x, y))?;
                if self.tiles[tile_index].game_objects.len() >= GAME_OBJECTS_PER_TILE {
                    return Err(SceneNormalError::GameObjectCapacity {
                        plane: plane_value,
                        x,
                        y,
                    });
                }
            }
        }

        self.models[model.0].claimed = true;
        let object = GameObjectModelData { model, start, end };
        for x in start.x..=end.x {
            for y in start.y..=end.y {
                let tile_index = self.required_tile_index(plane_value, SceneTile::new(x, y))?;
                self.tiles[tile_index].game_objects.push(object);
            }
        }
        Ok(())
    }

    /// Execute the pinned `Scene.method5585` normal-reconciliation ordering.
    ///
    /// Closing a model after its owner is processed mirrors the reference
    /// transition from ModelData to Model. The WorkingModel remains inspectable,
    /// but is no longer eligible as a later neighbor in this pass.
    pub fn reconcile_normals(&mut self) -> Result<SceneNormalMergeReport, SceneNormalError> {
        let mut report = SceneNormalMergeReport::default();

        for plane in 0..self.plane_count {
            for x in 0..self.width {
                for y in 0..self.height {
                    let tile = SceneTile::new(x, y);
                    let tile_index = self.required_tile_index(plane, tile)?;
                    let snapshot = self.tiles[tile_index].clone();

                    if let Some(boundary) = snapshot.boundary {
                        self.process_boundary(plane, tile, boundary, &mut report)?;
                    }

                    for game_object in snapshot.game_objects {
                        if self.model_pending(game_object.model)? {
                            let width = game_object.end.x - game_object.start.x + 1;
                            let depth = game_object.end.y - game_object.start.y + 1;
                            self.reconcile_neighbors(
                                game_object.model,
                                plane,
                                game_object.start,
                                width,
                                depth,
                                &mut report,
                            )?;
                            self.close_model(game_object.model, &mut report)?;
                        }
                    }

                    if let Some(floor_decoration) = snapshot.floor_decoration
                        && self.model_pending(floor_decoration.model)?
                    {
                        self.reconcile_floor_decoration(
                            floor_decoration.model,
                            plane,
                            tile,
                            &mut report,
                        )?;
                        self.close_model(floor_decoration.model, &mut report)?;
                    }
                }
            }
        }

        Ok(report)
    }

    fn process_boundary(
        &mut self,
        plane: u8,
        tile: SceneTile,
        boundary: BoundaryModelData,
        report: &mut SceneNormalMergeReport,
    ) -> Result<(), SceneNormalError> {
        if !self.model_pending(boundary.primary)? {
            return Ok(());
        }

        self.reconcile_neighbors(boundary.primary, plane, tile, 1, 1, report)?;
        if let Some(secondary) = boundary.secondary
            && self.model_pending(secondary)?
        {
            self.reconcile_neighbors(secondary, plane, tile, 1, 1, report)?;
            self.merge_pair(
                boundary.primary,
                secondary,
                ModelTranslation::ZERO,
                false,
                report,
            )?;
            self.close_model(secondary, report)?;
        }
        self.close_model(boundary.primary, report)?;
        Ok(())
    }

    fn reconcile_floor_decoration(
        &mut self,
        source: SceneModelDataId,
        plane: u8,
        source_tile: SceneTile,
        report: &mut SceneNormalMergeReport,
    ) -> Result<(), SceneNormalError> {
        let source_x =
            i32::try_from(source_tile.x).map_err(|_| SceneNormalError::CoordinateOverflow)?;
        let source_y =
            i32::try_from(source_tile.y).map_err(|_| SceneNormalError::CoordinateOverflow)?;
        let end_x = source_x
            .checked_add(1)
            .ok_or(SceneNormalError::CoordinateOverflow)?;
        let start_y = source_y
            .checked_sub(1)
            .ok_or(SceneNormalError::CoordinateOverflow)?;
        let end_y = source_y
            .checked_add(1)
            .ok_or(SceneNormalError::CoordinateOverflow)?;

        for x in source_x..=end_x {
            if x < 0 || x >= self.width as i32 {
                continue;
            }
            for y in start_y..=end_y {
                if y < 0 || y >= self.height as i32 || (x < end_x && y < end_y) {
                    continue;
                }
                let neighbor_tile = SceneTile::new(x as u32, y as u32);
                let tile_index = self.required_tile_index(plane, neighbor_tile)?;
                let Some(neighbor) = self.tiles[tile_index].floor_decoration else {
                    continue;
                };
                if !self.model_pending(neighbor.model)? {
                    continue;
                }

                let height_delta = self
                    .average_height(plane, x, y)?
                    .wrapping_sub(self.average_height(plane, source_x, source_y)?);
                let translation = ModelTranslation {
                    x: (x - source_x).wrapping_mul(LOCAL_UNITS_PER_TILE),
                    y: height_delta,
                    z: (y - source_y).wrapping_mul(LOCAL_UNITS_PER_TILE),
                };
                self.merge_pair(source, neighbor.model, translation, true, report)?;
            }
        }
        Ok(())
    }

    fn reconcile_neighbors(
        &mut self,
        source: SceneModelDataId,
        source_plane: u8,
        source_tile: SceneTile,
        source_width: u32,
        source_depth: u32,
        report: &mut SceneNormalMergeReport,
    ) -> Result<(), SceneNormalError> {
        let source_x =
            i32::try_from(source_tile.x).map_err(|_| SceneNormalError::CoordinateOverflow)?;
        let source_y =
            i32::try_from(source_tile.y).map_err(|_| SceneNormalError::CoordinateOverflow)?;
        let width =
            i32::try_from(source_width).map_err(|_| SceneNormalError::CoordinateOverflow)?;
        let depth =
            i32::try_from(source_depth).map_err(|_| SceneNormalError::CoordinateOverflow)?;
        let mut scan_start_x = source_x;
        let scan_end_x = source_x
            .checked_add(width)
            .ok_or(SceneNormalError::CoordinateOverflow)?;
        let scan_start_y = source_y
            .checked_sub(1)
            .ok_or(SceneNormalError::CoordinateOverflow)?;
        let scan_end_y = source_y
            .checked_add(depth)
            .ok_or(SceneNormalError::CoordinateOverflow)?;
        let mut hide_matched_faces = true;

        for neighbor_plane in source_plane..=source_plane.saturating_add(1) {
            if neighbor_plane >= self.plane_count {
                continue;
            }

            for x in scan_start_x..=scan_end_x {
                if x < 0 || x >= self.width as i32 {
                    continue;
                }
                for y in scan_start_y..=scan_end_y {
                    if y < 0 || y >= self.height as i32 {
                        continue;
                    }
                    if hide_matched_faces
                        && x < scan_end_x
                        && y < scan_end_y
                        && !(y < source_y && source_x != x)
                    {
                        continue;
                    }

                    let neighbor_tile = SceneTile::new(x as u32, y as u32);
                    let tile_index = self.required_tile_index(neighbor_plane, neighbor_tile)?;
                    let snapshot = self.tiles[tile_index].clone();
                    let height_delta = self
                        .average_height(neighbor_plane, x, y)?
                        .wrapping_sub(self.average_height(source_plane, source_x, source_y)?);

                    if let Some(boundary) = snapshot.boundary {
                        let translation = ModelTranslation {
                            x: (1 - width)
                                .wrapping_mul(HALF_TILE)
                                .wrapping_add((x - source_x).wrapping_mul(LOCAL_UNITS_PER_TILE)),
                            y: height_delta,
                            z: (y - source_y)
                                .wrapping_mul(LOCAL_UNITS_PER_TILE)
                                .wrapping_add((1 - depth).wrapping_mul(HALF_TILE)),
                        };
                        if self.model_pending(boundary.primary)? {
                            self.merge_pair(
                                source,
                                boundary.primary,
                                translation,
                                hide_matched_faces,
                                report,
                            )?;
                        }
                        if let Some(secondary) = boundary.secondary
                            && self.model_pending(secondary)?
                        {
                            self.merge_pair(
                                source,
                                secondary,
                                translation,
                                hide_matched_faces,
                                report,
                            )?;
                        }
                    }

                    for game_object in snapshot.game_objects {
                        if !self.model_pending(game_object.model)? {
                            continue;
                        }
                        let neighbor_width =
                            i32::try_from(game_object.end.x - game_object.start.x + 1)
                                .map_err(|_| SceneNormalError::CoordinateOverflow)?;
                        let neighbor_depth =
                            i32::try_from(game_object.end.y - game_object.start.y + 1)
                                .map_err(|_| SceneNormalError::CoordinateOverflow)?;
                        let neighbor_x = i32::try_from(game_object.start.x)
                            .map_err(|_| SceneNormalError::CoordinateOverflow)?;
                        let neighbor_y = i32::try_from(game_object.start.y)
                            .map_err(|_| SceneNormalError::CoordinateOverflow)?;
                        let translation = ModelTranslation {
                            x: (neighbor_width - width)
                                .wrapping_mul(HALF_TILE)
                                .wrapping_add(
                                    (neighbor_x - source_x).wrapping_mul(LOCAL_UNITS_PER_TILE),
                                ),
                            y: height_delta,
                            z: (neighbor_y - source_y)
                                .wrapping_mul(LOCAL_UNITS_PER_TILE)
                                .wrapping_add((neighbor_depth - depth).wrapping_mul(HALF_TILE)),
                        };
                        self.merge_pair(
                            source,
                            game_object.model,
                            translation,
                            hide_matched_faces,
                            report,
                        )?;
                    }
                }
            }

            scan_start_x = scan_start_x
                .checked_sub(1)
                .ok_or(SceneNormalError::CoordinateOverflow)?;
            hide_matched_faces = false;
        }
        Ok(())
    }

    fn merge_pair(
        &mut self,
        source: SceneModelDataId,
        neighbor: SceneModelDataId,
        translation: ModelTranslation,
        hide_matched_faces: bool,
        report: &mut SceneNormalMergeReport,
    ) -> Result<(), SceneNormalError> {
        if source == neighbor {
            return Ok(());
        }
        let (source_model, neighbor_model) = self.model_pair_mut(source, neighbor)?;
        let outcome = merge_model_normals(
            source_model,
            neighbor_model,
            translation,
            hide_matched_faces,
        );
        report.record_merge(outcome);
        Ok(())
    }

    fn close_model(
        &mut self,
        id: SceneModelDataId,
        report: &mut SceneNormalMergeReport,
    ) -> Result<(), SceneNormalError> {
        let Some(entry) = self.models.get_mut(id.0) else {
            return Err(SceneNormalError::UnknownModel(id.0));
        };
        if entry.pending {
            entry.pending = false;
            report.closed_models += 1;
        }
        Ok(())
    }

    fn model_pending(&self, id: SceneModelDataId) -> Result<bool, SceneNormalError> {
        self.models
            .get(id.0)
            .map(|entry| entry.pending)
            .ok_or(SceneNormalError::UnknownModel(id.0))
    }

    fn ensure_unclaimed(&self, id: SceneModelDataId) -> Result<(), SceneNormalError> {
        let Some(entry) = self.models.get(id.0) else {
            return Err(SceneNormalError::UnknownModel(id.0));
        };
        if entry.claimed {
            return Err(SceneNormalError::ModelAlreadyClaimed(id.0));
        }
        Ok(())
    }

    fn model_pair_mut(
        &mut self,
        first: SceneModelDataId,
        second: SceneModelDataId,
    ) -> Result<(&mut WorkingModel, &mut WorkingModel), SceneNormalError> {
        if first.0 >= self.models.len() {
            return Err(SceneNormalError::UnknownModel(first.0));
        }
        if second.0 >= self.models.len() {
            return Err(SceneNormalError::UnknownModel(second.0));
        }

        if first.0 < second.0 {
            let (left, right) = self.models.split_at_mut(second.0);
            Ok((&mut left[first.0].model, &mut right[0].model))
        } else {
            let (left, right) = self.models.split_at_mut(first.0);
            Ok((&mut right[0].model, &mut left[second.0].model))
        }
    }

    fn average_height(&self, plane: u8, x: i32, y: i32) -> Result<i32, SceneNormalError> {
        let x0 = u32::try_from(x).map_err(|_| SceneNormalError::CoordinateOverflow)?;
        let y0 = u32::try_from(y).map_err(|_| SceneNormalError::CoordinateOverflow)?;
        let x1 = x0
            .checked_add(1)
            .ok_or(SceneNormalError::CoordinateOverflow)?;
        let y1 = y0
            .checked_add(1)
            .ok_or(SceneNormalError::CoordinateOverflow)?;
        let a = self.height_at(plane, x1, y1)?;
        let b = self.height_at(plane, x0, y0)?;
        let c = self.height_at(plane, x0, y1)?;
        let d = self.height_at(plane, x1, y0)?;
        Ok(a.wrapping_add(b).wrapping_add(c).wrapping_add(d) / 4)
    }

    fn height_at(&self, plane: u8, x: u32, y: u32) -> Result<i32, SceneNormalError> {
        let Some(index) = self.height_index(plane, x, y) else {
            return Err(SceneNormalError::HeightCornerOutOfBounds { plane, x, y });
        };
        Ok(self.tile_heights[index])
    }

    fn required_tile_index(&self, plane: u8, tile: SceneTile) -> Result<usize, SceneNormalError> {
        self.tile_index(plane, tile)
            .ok_or(SceneNormalError::OutOfBounds {
                plane,
                x: tile.x,
                y: tile.y,
            })
    }

    fn tile_index(&self, plane: u8, tile: SceneTile) -> Option<usize> {
        if plane >= self.plane_count || tile.x >= self.width || tile.y >= self.height {
            return None;
        }
        let x = usize::try_from(tile.x).ok()?;
        let y = usize::try_from(tile.y).ok()?;
        usize::from(plane)
            .checked_mul(self.plane_len)?
            .checked_add(x.checked_mul(usize::try_from(self.height).ok()?)?)?
            .checked_add(y)
    }

    fn height_index(&self, plane: u8, x: u32, y: u32) -> Option<usize> {
        if plane >= self.plane_count || x > self.width || y > self.height {
            return None;
        }
        let x = usize::try_from(x).ok()?;
        let y = usize::try_from(y).ok()?;
        let height_depth = usize::try_from(self.height).ok()?.checked_add(1)?;
        usize::from(plane)
            .checked_mul(self.height_plane_len)?
            .checked_add(x.checked_mul(height_depth)?)?
            .checked_add(y)
    }
}
