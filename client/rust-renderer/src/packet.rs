use crate::draw::DrawRange;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RendererPacketError {
    VertexWordCount(usize),
    IndexOutOfBounds {
        index_position: usize,
        vertex_index: u32,
        vertex_count: usize,
    },
    DrawOffsetAlignment {
        draw_index: usize,
        offset_bytes: u32,
    },
    DrawRangeOutOfBounds {
        draw_index: usize,
        end_index: u64,
        index_count: usize,
    },
    DrawRangePlaneCount {
        range_count: usize,
        plane_count: usize,
    },
}

impl Display for RendererPacketError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::VertexWordCount(words) => {
                write!(
                    formatter,
                    "packed vertex packet must contain u32 triplets, got {words} words"
                )
            }
            Self::IndexOutOfBounds {
                index_position,
                vertex_index,
                vertex_count,
            } => write!(
                formatter,
                "index {index_position} references vertex {vertex_index}, but only {vertex_count} vertices exist"
            ),
            Self::DrawOffsetAlignment {
                draw_index,
                offset_bytes,
            } => write!(
                formatter,
                "draw range {draw_index} has non-u32-aligned byte offset {offset_bytes}"
            ),
            Self::DrawRangeOutOfBounds {
                draw_index,
                end_index,
                index_count,
            } => write!(
                formatter,
                "draw range {draw_index} ends at index {end_index}, but index buffer has {index_count} entries"
            ),
            Self::DrawRangePlaneCount {
                range_count,
                plane_count,
            } => write!(
                formatter,
                "draw-range plane packet has {plane_count} entries, expected {range_count}"
            ),
        }
    }
}

impl std::error::Error for RendererPacketError {}

pub fn validate_geometry(
    packed_vertices: &[u32],
    indices: &[u32],
) -> Result<(), RendererPacketError> {
    if !packed_vertices.len().is_multiple_of(3) {
        return Err(RendererPacketError::VertexWordCount(packed_vertices.len()));
    }

    let vertex_count = packed_vertices.len() / 3;
    for (index_position, &vertex_index) in indices.iter().enumerate() {
        if vertex_index as usize >= vertex_count {
            return Err(RendererPacketError::IndexOutOfBounds {
                index_position,
                vertex_index,
                vertex_count,
            });
        }
    }

    Ok(())
}

pub fn validate_draw_ranges(
    ranges: &[DrawRange],
    index_count: usize,
) -> Result<(), RendererPacketError> {
    for (draw_index, range) in ranges.iter().enumerate() {
        if range.offset_bytes % 4 != 0 {
            return Err(RendererPacketError::DrawOffsetAlignment {
                draw_index,
                offset_bytes: range.offset_bytes,
            });
        }

        let start_index = u64::from(range.offset_bytes / 4);
        let end_index = start_index + u64::from(range.elements);
        if end_index > index_count as u64 {
            return Err(RendererPacketError::DrawRangeOutOfBounds {
                draw_index,
                end_index,
                index_count,
            });
        }
    }

    Ok(())
}

pub fn validate_draw_range_planes(
    ranges: &[DrawRange],
    range_planes: &[u8],
) -> Result<(), RendererPacketError> {
    if ranges.len() != range_planes.len() {
        return Err(RendererPacketError::DrawRangePlaneCount {
            range_count: ranges.len(),
            plane_count: range_planes.len(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_rejects_indices_outside_vertex_packet() {
        let error = validate_geometry(&[1, 2, 3], &[0, 1]).unwrap_err();
        assert_eq!(
            error,
            RendererPacketError::IndexOutOfBounds {
                index_position: 1,
                vertex_index: 1,
                vertex_count: 1,
            }
        );
    }

    #[test]
    fn draw_range_planes_require_one_entry_per_range() {
        assert!(
            validate_draw_range_planes(
                &[DrawRange::new(0, 3, 1), DrawRange::new(12, 6, 1)],
                &[0, 2],
            )
            .is_ok()
        );
        assert_eq!(
            validate_draw_range_planes(
                &[DrawRange::new(0, 3, 1), DrawRange::new(12, 6, 1)],
                &[0],
            )
            .unwrap_err(),
            RendererPacketError::DrawRangePlaneCount {
                range_count: 2,
                plane_count: 1,
            }
        );
    }

    #[test]
    fn draw_ranges_validate_byte_offsets_and_bounds() {
        assert!(validate_draw_ranges(&[DrawRange::new(4, 2, 1)], 3).is_ok());

        assert_eq!(
            validate_draw_ranges(&[DrawRange::new(2, 1, 1)], 3).unwrap_err(),
            RendererPacketError::DrawOffsetAlignment {
                draw_index: 0,
                offset_bytes: 2,
            }
        );

        assert_eq!(
            validate_draw_ranges(&[DrawRange::new(8, 2, 1)], 3).unwrap_err(),
            RendererPacketError::DrawRangeOutOfBounds {
                draw_index: 0,
                end_index: 4,
                index_count: 3,
            }
        );
    }
}
