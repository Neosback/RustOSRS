//! Exact reference ground-contouring semantics for M8.
//!
//! The audited `Model.contourGround(...)` path operates on already-lit model
//! instances in exact 128-unit tile space. This module preserves its fast
//! paths, Java `int` arithmetic, copy-vs-mutate ownership, bilinear sampling,
//! and nonzero clip behavior without introducing cache, scene, or renderer
//! dependencies.

use crate::lighting::ReferenceLitModel;
use std::{borrow::Cow, error::Error, fmt};

const TILE_SHIFT: u32 = 7;
const TILE_MASK: i32 = 127;
const TILE_SIZE: i32 = 128;
const FIXED_POINT_SHIFT: u32 = 16;

/// Inputs supplied to the reference `Model.contourGround(...)` algorithm.
#[derive(Debug, Clone, Copy)]
pub struct ContourGroundInput<'a> {
    /// Height samples indexed as `[x][z]`.
    pub heights: &'a [Vec<i32>],
    /// Model scene-space X origin in exact 128-unit tile coordinates.
    pub origin_x: i32,
    /// Reference base height subtracted from sampled ground.
    pub base_height: i32,
    /// Model scene-space Z origin in exact 128-unit tile coordinates.
    pub origin_z: i32,
    /// Reference fixed-point clip threshold. Zero contours every vertex.
    pub clip: i32,
}

/// Explicit malformed-input failures that the Java path would otherwise panic on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContourGroundError {
    EmptyHeightGrid,
    NonRectangularHeightGrid {
        row: usize,
        expected: usize,
        actual: usize,
    },
    HeightGridTooLarge,
    SampleOutOfBounds {
        x: i32,
        z: i32,
    },
    ZeroModelHeight,
}

impl fmt::Display for ContourGroundError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyHeightGrid => formatter.write_str("contour height grid must be non-empty"),
            Self::NonRectangularHeightGrid {
                row,
                expected,
                actual,
            } => write!(
                formatter,
                "contour height-grid row {row} has length {actual}, expected {expected}"
            ),
            Self::HeightGridTooLarge => {
                formatter.write_str("contour height-grid dimensions exceed reference int range")
            }
            Self::SampleOutOfBounds { x, z } => {
                write!(formatter, "contour sample ({x},{z}) is outside the height grid")
            }
            Self::ZeroModelHeight => formatter.write_str(
                "nonzero contour clip cannot divide by zero reference model height",
            ),
        }
    }
}

impl Error for ContourGroundError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ModelCylinderBounds {
    height: i32,
    xz_radius: i32,
}

/// Reference copying contour path.
///
/// Fast-path cases return the original borrowed model exactly as the client
/// returns `this`. Otherwise an owned clone is contoured, so the shared cached
/// base model remains immutable.
pub fn contour_ground_copy<'a>(
    model: &'a ReferenceLitModel,
    input: ContourGroundInput<'_>,
) -> Result<Cow<'a, ReferenceLitModel>, ContourGroundError> {
    let dimensions = validate_height_grid(input.heights)?;
    let bounds = calculate_bounds_cylinder(model);
    if contour_fast_path(model, input, dimensions, bounds)? {
        return Ok(Cow::Borrowed(model));
    }

    let mut contoured = model.clone();
    apply_contour(&mut contoured, input, dimensions, bounds.height)?;
    Ok(Cow::Owned(contoured))
}

/// Reference in-place contour path used only after the caller already owns a
/// safe dynamic working model.
///
/// Returns `false` for the audited out-of-range/equal-corner fast paths and
/// `true` when the contour path executes. `ReferenceLitModel` has no cached
/// bounds, so geometry mutation cannot leave stale bound state behind.
pub fn contour_ground_in_place(
    model: &mut ReferenceLitModel,
    input: ContourGroundInput<'_>,
) -> Result<bool, ContourGroundError> {
    let dimensions = validate_height_grid(input.heights)?;
    let bounds = calculate_bounds_cylinder(model);
    if contour_fast_path(model, input, dimensions, bounds)? {
        return Ok(false);
    }

    apply_contour(model, input, dimensions, bounds.height)?;
    Ok(true)
}

fn validate_height_grid(heights: &[Vec<i32>]) -> Result<(i32, i32), ContourGroundError> {
    let Some(first) = heights.first() else {
        return Err(ContourGroundError::EmptyHeightGrid);
    };
    if first.is_empty() {
        return Err(ContourGroundError::EmptyHeightGrid);
    }

    let expected = first.len();
    for (row, values) in heights.iter().enumerate().skip(1) {
        if values.len() != expected {
            return Err(ContourGroundError::NonRectangularHeightGrid {
                row,
                expected,
                actual: values.len(),
            });
        }
    }

    let width = i32::try_from(heights.len()).map_err(|_| ContourGroundError::HeightGridTooLarge)?;
    let depth = i32::try_from(expected).map_err(|_| ContourGroundError::HeightGridTooLarge)?;
    Ok((width, depth))
}

fn calculate_bounds_cylinder(model: &ReferenceLitModel) -> ModelCylinderBounds {
    let mut height = 0_i32;
    let mut radius_squared = 0_i32;

    for vertex in &model.vertices {
        let upward_extent = vertex.y.wrapping_neg();
        if upward_extent > height {
            height = upward_extent;
        }

        let squared = vertex
            .x
            .wrapping_mul(vertex.x)
            .wrapping_add(vertex.z.wrapping_mul(vertex.z));
        if squared > radius_squared {
            radius_squared = squared;
        }
    }

    let xz_radius = (f64::from(radius_squared).sqrt() + 0.99_f64) as i32;
    ModelCylinderBounds { height, xz_radius }
}

fn contour_fast_path(
    model: &ReferenceLitModel,
    input: ContourGroundInput<'_>,
    dimensions: (i32, i32),
    bounds: ModelCylinderBounds,
) -> Result<bool, ContourGroundError> {
    let (width, depth) = dimensions;
    let min_x = input.origin_x.wrapping_sub(bounds.xz_radius);
    let max_x = input.origin_x.wrapping_add(bounds.xz_radius);
    let min_z = input.origin_z.wrapping_sub(bounds.xz_radius);
    let max_z = input.origin_z.wrapping_add(bounds.xz_radius);

    if min_x < 0
        || max_x.wrapping_add(TILE_SIZE) >> TILE_SHIFT >= width
        || min_z < 0
        || max_z.wrapping_add(TILE_SIZE) >> TILE_SHIFT >= depth
    {
        return Ok(true);
    }

    let min_tile_x = min_x >> TILE_SHIFT;
    let max_tile_x = max_x.wrapping_add(TILE_MASK) >> TILE_SHIFT;
    let min_tile_z = min_z >> TILE_SHIFT;
    let max_tile_z = max_z.wrapping_add(TILE_MASK) >> TILE_SHIFT;

    let southwest = height_at(input.heights, min_tile_x, min_tile_z)?;
    let southeast = height_at(input.heights, max_tile_x, min_tile_z)?;
    let northwest = height_at(input.heights, min_tile_x, max_tile_z)?;
    let northeast = height_at(input.heights, max_tile_x, max_tile_z)?;

    let _ = model;
    Ok(input.base_height == southwest
        && input.base_height == southeast
        && input.base_height == northwest
        && input.base_height == northeast)
}

fn apply_contour(
    model: &mut ReferenceLitModel,
    input: ContourGroundInput<'_>,
    dimensions: (i32, i32),
    model_height: i32,
) -> Result<(), ContourGroundError> {
    if input.clip != 0 && model_height == 0 && !model.vertices.is_empty() {
        return Err(ContourGroundError::ZeroModelHeight);
    }

    for vertex in &mut model.vertices {
        let original_y = vertex.y;
        if input.clip == 0 {
            let sampled = sample_ground(input.heights, dimensions, input, vertex.x, vertex.z)?;
            vertex.y = sampled.wrapping_add(original_y).wrapping_sub(input.base_height);
            continue;
        }

        let fixed_height = original_y
            .wrapping_neg()
            .wrapping_shl(FIXED_POINT_SHIFT);
        let ratio = java_int_div(fixed_height, model_height);
        if ratio < input.clip {
            let sampled = sample_ground(input.heights, dimensions, input, vertex.x, vertex.z)?;
            let adjustment = input
                .clip
                .wrapping_sub(ratio)
                .wrapping_mul(sampled.wrapping_sub(input.base_height));
            vertex.y = java_int_div(adjustment, input.clip).wrapping_add(original_y);
        }
    }

    Ok(())
}

fn sample_ground(
    heights: &[Vec<i32>],
    dimensions: (i32, i32),
    input: ContourGroundInput<'_>,
    vertex_x: i32,
    vertex_z: i32,
) -> Result<i32, ContourGroundError> {
    let world_x = input.origin_x.wrapping_add(vertex_x);
    let world_z = input.origin_z.wrapping_add(vertex_z);
    let offset_x = world_x & TILE_MASK;
    let offset_z = world_z & TILE_MASK;
    let tile_x = world_x >> TILE_SHIFT;
    let tile_z = world_z >> TILE_SHIFT;
    let (width, depth) = dimensions;

    if tile_x < 0
        || tile_z < 0
        || tile_x.wrapping_add(1) >= width
        || tile_z.wrapping_add(1) >= depth
    {
        return Err(ContourGroundError::SampleOutOfBounds {
            x: tile_x,
            z: tile_z,
        });
    }

    let h00 = height_at(heights, tile_x, tile_z)?;
    let h10 = height_at(heights, tile_x + 1, tile_z)?;
    let h01 = height_at(heights, tile_x, tile_z + 1)?;
    let h11 = height_at(heights, tile_x + 1, tile_z + 1)?;

    let south = h00
        .wrapping_mul(TILE_SIZE - offset_x)
        .wrapping_add(h10.wrapping_mul(offset_x))
        >> TILE_SHIFT;
    let north = h01
        .wrapping_mul(TILE_SIZE - offset_x)
        .wrapping_add(h11.wrapping_mul(offset_x))
        >> TILE_SHIFT;
    Ok(south
        .wrapping_mul(TILE_SIZE - offset_z)
        .wrapping_add(north.wrapping_mul(offset_z))
        >> TILE_SHIFT)
}

fn height_at(heights: &[Vec<i32>], x: i32, z: i32) -> Result<i32, ContourGroundError> {
    let x_index = usize::try_from(x).map_err(|_| ContourGroundError::SampleOutOfBounds { x, z })?;
    let z_index = usize::try_from(z).map_err(|_| ContourGroundError::SampleOutOfBounds { x, z })?;
    heights
        .get(x_index)
        .and_then(|row| row.get(z_index))
        .copied()
        .ok_or(ContourGroundError::SampleOutOfBounds { x, z })
}

fn java_int_div(dividend: i32, divisor: i32) -> i32 {
    if dividend == i32::MIN && divisor == -1 {
        i32::MIN
    } else {
        dividend / divisor
    }
}
