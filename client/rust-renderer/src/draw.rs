/// Draw-range layout used by the existing TypeScript renderer:
/// (index byte offset, element count, instance count).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrawRange {
    pub offset_bytes: u32,
    pub elements: u32,
    pub instances: u32,
}

impl DrawRange {
    pub const fn new(offset_bytes: u32, elements: u32, instances: u32) -> Self {
        Self {
            offset_bytes,
            elements,
            instances,
        }
    }

    pub const fn is_empty(self) -> bool {
        self.elements == 0 || self.instances == 0
    }

    pub const fn submitted_indices(self) -> u64 {
        self.elements as u64 * self.instances as u64
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DrawStats {
    pub draw_calls: u32,
    pub submitted_indices: u64,
}

pub fn draw_range_is_visible(range: DrawRange, plane: Option<u8>, roof_plane_limit: u8) -> bool {
    if range.is_empty() {
        return false;
    }
    if roof_plane_limit >= 3 {
        return true;
    }

    // Match TypeScript drawWithRoofPlaneFilter: missing metadata defaults
    // visible on plane 0 rather than accidentally dropping geometry.
    plane.unwrap_or(0) <= roof_plane_limit
}

pub fn filter_draw_ranges(
    ranges: &[DrawRange],
    range_planes: Option<&[u8]>,
    roof_plane_limit: u8,
) -> Vec<DrawRange> {
    ranges
        .iter()
        .copied()
        .enumerate()
        .filter(|(index, range)| {
            draw_range_is_visible(
                *range,
                range_planes.and_then(|planes| planes.get(*index).copied()),
                roof_plane_limit,
            )
        })
        .map(|(_, range)| range)
        .collect()
}

pub fn parse_draw_ranges(flat: &[u32]) -> Result<Vec<DrawRange>, &'static str> {
    if !flat.len().is_multiple_of(3) {
        return Err("draw range packet must contain triples");
    }

    let (triples, remainder) = flat.as_chunks::<3>();
    debug_assert!(remainder.is_empty());

    Ok(triples
        .iter()
        .map(|chunk| DrawRange::new(chunk[0], chunk[1], chunk[2]))
        .collect())
}

pub fn parse_draw_range_patches(
    flat: &[u32],
    range_count: usize,
) -> Result<Vec<(usize, DrawRange)>, String> {
    if !flat.len().is_multiple_of(4) {
        return Err("draw-range patch packet must contain quadruples".to_string());
    }

    let mut patches = Vec::with_capacity(flat.len() / 4);
    for chunk in flat.chunks_exact(4) {
        let range_index = chunk[0] as usize;
        if range_index >= range_count {
            return Err(format!(
                "draw-range patch index {range_index} is outside {range_count} resident ranges"
            ));
        }

        patches.push((range_index, DrawRange::new(chunk[1], chunk[2], chunk[3])));
    }
    Ok(patches)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roof_filter_keeps_missing_plane_metadata_visible() {
        let ranges = [
            DrawRange::new(0, 3, 1),
            DrawRange::new(12, 6, 1),
            DrawRange::new(36, 9, 1),
        ];

        assert_eq!(
            filter_draw_ranges(&ranges, Some(&[0, 2]), 0),
            vec![ranges[0], ranges[2]]
        );
    }

    #[test]
    fn roof_visibility_preserves_original_range_semantics() {
        let visible = DrawRange::new(0, 3, 1);
        let empty = DrawRange::new(12, 0, 1);

        assert!(draw_range_is_visible(visible, Some(0), 0));
        assert!(!draw_range_is_visible(visible, Some(2), 0));
        assert!(draw_range_is_visible(visible, None, 0));
        assert!(draw_range_is_visible(visible, Some(2), 3));
        assert!(!draw_range_is_visible(empty, Some(0), 3));
    }

    #[test]
    fn parses_typescript_draw_range_packet() {
        assert_eq!(
            parse_draw_ranges(&[0, 6, 1, 24, 12, 2]).unwrap(),
            vec![DrawRange::new(0, 6, 1), DrawRange::new(24, 12, 2)]
        );
    }

    #[test]
    fn parses_animation_draw_range_patch_packet() {
        assert_eq!(
            parse_draw_range_patches(&[2, 48, 6, 1, 0, 0, 3, 2], 3).unwrap(),
            vec![
                (2, DrawRange::new(48, 6, 1)),
                (0, DrawRange::new(0, 3, 2)),
            ]
        );
    }

    #[test]
    fn rejects_malformed_animation_draw_range_patches() {
        assert_eq!(
            parse_draw_range_patches(&[0, 12, 3], 1).unwrap_err(),
            "draw-range patch packet must contain quadruples"
        );
        assert_eq!(
            parse_draw_range_patches(&[1, 0, 3, 1], 1).unwrap_err(),
            "draw-range patch index 1 is outside 1 resident ranges"
        );
    }
}
