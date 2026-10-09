//! Exact initial-placement terrain-height sampling for M6.
//!
//! The pinned client samples the four height-grid points around the rotated
//! definition footprint midpoint. At the scene edge it falls back to the
//! anchor and anchor+1 indices instead of reading beyond the scene height grid.

use crate::placement::Footprint;
use osrs_core::coords::SceneTile;
use std::{error::Error, fmt};

/// Inputs required by the audited initial-placement height sampler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlacementHeightInput {
    pub tile: SceneTile,
    pub size_x: u16,
    pub size_y: u16,
    pub orientation: u8,
}

/// Explicit failures for malformed height-grid or placement inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementHeightError {
    InvalidOrientation(u8),
    ZeroFootprint,
    TileOutOfBounds {
        x: u32,
        y: u32,
        scene_width: u32,
        scene_height: u32,
    },
    HeightGridTooSmall,
    CoordinateOverflow,
}

impl fmt::Display for PlacementHeightError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidOrientation(value) => {
                write!(formatter, "loc orientation {value} is outside 0..=3")
            }
            Self::ZeroFootprint => formatter.write_str("loc definition footprint must be non-zero"),
            Self::TileOutOfBounds {
                x,
                y,
                scene_width,
                scene_height,
            } => write!(
                formatter,
                "placement tile ({x},{y}) is outside scene {scene_width}x{scene_height}",
            ),
            Self::HeightGridTooSmall => formatter
                .write_str("height grid must contain scene_width+1 by scene_height+1 samples"),
            Self::CoordinateOverflow => {
                formatter.write_str("placement height-sample index overflow")
            }
        }
    }
}

impl Error for PlacementHeightError {}

/// Sample the exact semantic terrain height used by the pinned initial loc path.
///
/// `heights` is indexed as `[x][y]` and must expose the one-sample border used
/// by scene terrain heights. Addition uses Java `int` wrapping semantics before
/// the final arithmetic right shift by two.
pub fn sample_placement_height(
    heights: &[Vec<i32>],
    scene_width: u32,
    scene_height: u32,
    input: PlacementHeightInput,
) -> Result<i32, PlacementHeightError> {
    if input.orientation > 3 {
        return Err(PlacementHeightError::InvalidOrientation(input.orientation));
    }
    if input.size_x == 0 || input.size_y == 0 {
        return Err(PlacementHeightError::ZeroFootprint);
    }
    if input.tile.x >= scene_width || input.tile.y >= scene_height {
        return Err(PlacementHeightError::TileOutOfBounds {
            x: input.tile.x,
            y: input.tile.y,
            scene_width,
            scene_height,
        });
    }

    let required_x = usize::try_from(scene_width)
        .ok()
        .and_then(|value| value.checked_add(1))
        .ok_or(PlacementHeightError::CoordinateOverflow)?;
    let required_y = usize::try_from(scene_height)
        .ok()
        .and_then(|value| value.checked_add(1))
        .ok_or(PlacementHeightError::CoordinateOverflow)?;
    if heights.len() < required_x
        || heights
            .iter()
            .take(required_x)
            .any(|row| row.len() < required_y)
    {
        return Err(PlacementHeightError::HeightGridTooSmall);
    }

    let footprint = Footprint::rotated(input.size_x, input.size_y, input.orientation);
    let width = u32::from(footprint.width);
    let depth = u32::from(footprint.depth);

    let (sample_x0, sample_x1) = midpoint_indices(input.tile.x, width, scene_width)?;
    let (sample_y0, sample_y1) = midpoint_indices(input.tile.y, depth, scene_height)?;

    let x0 = usize::try_from(sample_x0).map_err(|_| PlacementHeightError::CoordinateOverflow)?;
    let x1 = usize::try_from(sample_x1).map_err(|_| PlacementHeightError::CoordinateOverflow)?;
    let y0 = usize::try_from(sample_y0).map_err(|_| PlacementHeightError::CoordinateOverflow)?;
    let y1 = usize::try_from(sample_y1).map_err(|_| PlacementHeightError::CoordinateOverflow)?;

    let sum = heights[x1][y0]
        .wrapping_add(heights[x0][y0])
        .wrapping_add(heights[x0][y1])
        .wrapping_add(heights[x1][y1]);
    Ok(sum >> 2)
}

fn midpoint_indices(
    anchor: u32,
    extent: u32,
    scene_extent: u32,
) -> Result<(u32, u32), PlacementHeightError> {
    let fits = anchor
        .checked_add(extent)
        .ok_or(PlacementHeightError::CoordinateOverflow)?
        <= scene_extent;
    if fits {
        let lower = anchor
            .checked_add(extent >> 1)
            .ok_or(PlacementHeightError::CoordinateOverflow)?;
        let upper = anchor
            .checked_add((extent + 1) >> 1)
            .ok_or(PlacementHeightError::CoordinateOverflow)?;
        Ok((lower, upper))
    } else {
        Ok((
            anchor,
            anchor
                .checked_add(1)
                .ok_or(PlacementHeightError::CoordinateOverflow)?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(width: u32, height: u32) -> Vec<Vec<i32>> {
        (0..=width)
            .map(|x| {
                (0..=height)
                    .map(|y| (x as i32) * 100 + (y as i32) * 10)
                    .collect()
            })
            .collect()
    }

    #[test]
    fn rotated_non_square_footprint_samples_exact_midpoint_pair() {
        let heights = grid(4, 4);
        let sampled = sample_placement_height(
            &heights,
            4,
            4,
            PlacementHeightInput {
                tile: SceneTile::new(0, 0),
                size_x: 2,
                size_y: 3,
                orientation: 1,
            },
        );
        assert_eq!(sampled, Ok(160));
    }

    #[test]
    fn scene_edge_uses_anchor_and_anchor_plus_one_fallback() {
        let heights = grid(4, 4);
        let sampled = sample_placement_height(
            &heights,
            4,
            4,
            PlacementHeightInput {
                tile: SceneTile::new(3, 3),
                size_x: 2,
                size_y: 3,
                orientation: 0,
            },
        );
        assert_eq!(sampled, Ok(385));
    }

    #[test]
    fn invalid_inputs_fail_without_guessing() {
        let heights = grid(2, 2);
        assert_eq!(
            sample_placement_height(
                &heights,
                2,
                2,
                PlacementHeightInput {
                    tile: SceneTile::new(0, 0),
                    size_x: 1,
                    size_y: 1,
                    orientation: 4,
                },
            ),
            Err(PlacementHeightError::InvalidOrientation(4))
        );
        assert_eq!(
            sample_placement_height(
                &heights,
                2,
                2,
                PlacementHeightInput {
                    tile: SceneTile::new(2, 0),
                    size_x: 1,
                    size_y: 1,
                    orientation: 0,
                },
            ),
            Err(PlacementHeightError::TileOutOfBounds {
                x: 2,
                y: 0,
                scene_width: 2,
                scene_height: 2,
            })
        );
    }
}
