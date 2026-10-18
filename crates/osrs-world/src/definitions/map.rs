//! Source maps for normal worlds and disposable, cache-independent prefab stages.
use super::*;
impl WorldDefinitions {
    /// Configure a disposable worker before its first build. Definitions and models still come
    /// from the selected cache; terrain and placements never do. Edited semantics are replayed
    /// by the normal projection path over this immutable, history-free source.
    pub(crate) fn use_virtual_map(&mut self, underlay: u16) {
        self.virtual_underlay = Some(underlay);
    }
    /// Read and decode one region's terrain and location files, or `None` when the cache has no
    /// map square for it.
    pub fn region(&self, region: RegionCoord) -> Result<Option<RegionMap>, WorldError> {
        let (mut tiles, locations) = if let Some(underlay) = self.virtual_underlay {
            virtual_region(region, underlay)
        } else {
            let Some(source) = self.cache_region(region)? else {
                return Ok(None);
            };
            source
        };
        for edit in self.edits.region((region.x, region.y)) {
            let (_, (x, y)) = edit.locate();
            let index = (usize::from(edit.plane) * 64 + usize::from(x)) * 64 + usize::from(y);
            if let Some(tile) = tiles.get_mut(index) {
                edit.apply_to(tile);
            }
        }
        let locations = if self.loc_edits.touches((region.x, region.y)) {
            DecodedLocations::with_locations(
                region,
                self.loc_edits.replay(region, locations.locations()),
            )
        } else {
            locations
        };
        Ok(Some(RegionMap {
            terrain: RegionTerrain::new(tiles)?,
            locations,
        }))
    }

    fn cache_region(
        &self,
        region: RegionCoord,
    ) -> Result<Option<(Vec<RegionTerrainTile>, DecodedLocations)>, WorldError> {
        let square = match self.cache().read_map_square(region) {
            Ok(square) => square,
            Err(error)
                if matches!(
                    error.kind(),
                    CacheErrorKind::MapResolution { .. }
                        | CacheErrorKind::MissingGroup
                        | CacheErrorKind::MissingMetadata
                        | CacheErrorKind::MissingFile
                ) =>
            {
                return Ok(None);
            }
            Err(error) => return Err(error.into()),
        };
        let terrain = decode_terrain(
            &square.terrain.bytes,
            &self.context,
            &square.terrain.provenance,
            region,
        )?;
        let locations = decode_locations(
            &square.locations.bytes,
            &self.context,
            &square.locations.provenance,
            region,
        )?;
        Ok(Some((region_tiles_from_decoded(&terrain), locations)))
    }
}

fn virtual_region(
    region: RegionCoord,
    underlay: u16,
) -> (Vec<RegionTerrainTile>, DecodedLocations) {
    let tiles = (0..4)
        .flat_map(|plane| {
            std::iter::repeat_n(
                RegionTerrainTile {
                    // Zero on the base plane; -240 relative to the plane below on upper planes.
                    height: RegionTileHeight::Explicit(if plane == 0 { 0 } else { 30 }),
                    overlay: None,
                    settings: None,
                    underlay: if plane == 0 { underlay } else { 0 },
                },
                64 * 64,
            )
        })
        .collect();
    (tiles, DecodedLocations::with_locations(region, vec![]))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn virtual_regions_have_flat_support_on_all_planes_and_no_cache_content() {
        for region in [
            RegionCoord::new(50, 33),
            RegionCoord::new(49, 32),
            RegionCoord::new(51, 34),
        ] {
            let (tiles, locs) = virtual_region(region, 50);
            assert!(locs.locations().is_empty());
            let terrain = RegionTerrain::new(tiles).unwrap_or_else(|e| panic!("{e:?}"));
            for p in 0..4 {
                for x in 0..64 {
                    for y in 0..64 {
                        let t = terrain.tile(p, x, y);
                        assert_eq!(
                            t.height,
                            RegionTileHeight::Explicit(if p == 0 { 0 } else { 30 })
                        );
                        assert_eq!(t.underlay, if p == 0 { 50 } else { 0 });
                        assert_eq!(t.overlay, None);
                        assert_eq!(t.settings, None);
                    }
                }
            }
        }
    }
}
