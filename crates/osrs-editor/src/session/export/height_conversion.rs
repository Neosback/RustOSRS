//! Explicit lossy target-height conversion. Projects and the live document retain exact heights.
use super::{ExportError, baseline_document};
use osrs_cache::decode::{DecodedLocations, DecodedTerrain};
use osrs_core::coords::{MapTile, RegionTile, SourcePlane};
use osrs_edit::{HeightVertexCoord, MapDocument, MapOp, MapPatch, MapPosition};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HeightExportPolicy {
    Exact,
    /// Source editor: truncate authored per-plane deltas / 8; byte 1 reloads as zero steps.
    EikenTruncate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HeightConversion {
    pub position: MapPosition,
    pub authored: i32,
    pub reloaded: i32,
}

pub(super) fn convert(
    terrain: &DecodedTerrain,
    locations: &DecodedLocations,
    document: &MapDocument,
    policy: HeightExportPolicy,
) -> Result<(MapDocument, Vec<HeightConversion>), ExportError> {
    if policy == HeightExportPolicy::Exact {
        return Ok((document.clone(), Vec::new()));
    }
    let baseline = baseline_document(terrain, locations)?;
    let mut changes = Vec::new();
    let mut ops = Vec::new();
    let origin = MapTile::from_region(
        terrain.region(),
        RegionTile::new(0, 0).unwrap_or_else(|| unreachable!("region origin")),
    )
    .unwrap_or_else(|| unreachable!("loaded region coordinates"));
    for x in 0..64 {
        for y in 0..64 {
            let tile = MapTile::new(origin.x + x, origin.y + y);
            let coords = [0, 1, 2, 3].map(|z| {
                HeightVertexCoord::new(
                    tile,
                    SourcePlane::new(z).unwrap_or_else(|| unreachable!("plane")),
                )
            });
            if coords
                .iter()
                .all(|v| document.height_at(*v) == baseline.height_at(*v))
            {
                continue;
            }
            let mut authored_base = 0;
            let mut reloaded_base = 0;
            for coord in coords {
                let target = document.height_at(coord).ok_or_else(|| bad(coord, 0))?;
                // Compute in i64 so malformed project values cannot overflow validation.
                let delta = i64::from(authored_base) - i64::from(target);
                let byte = delta / 8;
                if delta < 0 || byte > 255 {
                    return Err(bad(coord, target));
                }
                let reloaded = reloaded_base - if byte == 1 { 0 } else { byte as i32 * 8 };
                if reloaded != target {
                    changes.push(HeightConversion {
                        position: MapPosition::new(tile, coord.plane),
                        authored: target,
                        reloaded,
                    });
                    ops.push(MapOp::SetHeight {
                        coord,
                        before: target,
                        after: reloaded,
                    });
                }
                authored_base = target;
                reloaded_base = reloaded;
            }
        }
    }
    let mut expected = document.clone();
    expected
        .apply_patch(&MapPatch::new(ops))
        .unwrap_or_else(|_| unreachable!("conversion patch uses its snapshot's before values"));
    Ok((expected, changes))
}

fn bad(coord: HeightVertexCoord, height: i32) -> ExportError {
    ExportError::HeightNotStorable {
        position: MapPosition::new(coord.corner, coord.plane),
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::export::tests::{blank_terrain, reimport, source_locations};

    #[test]
    fn conversion_is_explicit_and_preserves_the_authored_document() {
        let terrain = blank_terrain();
        let locations = source_locations();
        let mut document = baseline_document(&terrain, &locations)
            .unwrap_or_else(|_| unreachable!("valid fixture"));
        let tile = MapTile::new(3200, 3200);
        let mut patch = Vec::new();
        // A ten-unit inter-plane gap encodes as byte 1, which reloads as no gap.
        for (z, after) in [(0, -53), (1, -63), (2, -253), (3, -494)] {
            let coord = HeightVertexCoord::new(
                tile,
                SourcePlane::new(z).unwrap_or_else(|| unreachable!("valid fixture")),
            );
            patch.push(MapOp::SetHeight {
                coord,
                before: document
                    .height_at(coord)
                    .unwrap_or_else(|| unreachable!("valid fixture")),
                after,
            });
        }
        document
            .apply_patch(&MapPatch::new(patch))
            .unwrap_or_else(|_| unreachable!("valid fixture"));
        let original = document.clone();
        let (converted, changes) = convert(
            &terrain,
            &locations,
            &document,
            HeightExportPolicy::EikenTruncate,
        )
        .unwrap_or_else(|_| unreachable!("valid fixture"));
        let heights: Vec<_> = (0..4)
            .map(|z| {
                converted
                    .height_at(HeightVertexCoord::new(
                        tile,
                        SourcePlane::new(z).unwrap_or_else(|| unreachable!("valid fixture")),
                    ))
                    .unwrap_or_else(|| unreachable!("valid fixture"))
            })
            .collect();
        assert_eq!(heights, [-48, -48, -232, -472]);
        assert_eq!(changes.len(), 4);
        assert_eq!(document, original);
        let prepared = super::super::export_region_with_policy(
            &terrain,
            &locations,
            &document,
            HeightExportPolicy::EikenTruncate,
        )
        .unwrap_or_else(|_| unreachable!("valid fixture"));
        assert_eq!(
            reimport(
                prepared
                    .region
                    .as_ref()
                    .unwrap_or_else(|| unreachable!("valid fixture"))
            ),
            converted
        );
        assert!(super::super::export_region(&terrain, &locations, &document).is_err());
        assert_eq!(
            convert(&terrain, &locations, &document, HeightExportPolicy::Exact)
                .unwrap_or_else(|_| unreachable!("valid fixture")),
            (document, Vec::new())
        );
    }
}
