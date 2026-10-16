//! A path gesture's fixed source-editor map: region membership, local origin and dimensions.
use super::InteractionRuntime;
use osrs_core::coords::{MapTile, RegionCoord};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::app) struct PathExtent {
    pub origin: MapTile,
    pub size: (i32, i32),
    pub generation: u64,
    regions: BTreeSet<RegionCoord>,
}

impl PathExtent {
    pub(in crate::app) fn capture(
        regions: impl IntoIterator<Item = RegionCoord>,
        generation: u64,
    ) -> Option<Self> {
        let regions: BTreeSet<_> = regions.into_iter().collect();
        let min_x = regions.iter().map(|r| r.x).min()?.checked_mul(64)?;
        let min_y = regions.iter().map(|r| r.y).min()?.checked_mul(64)?;
        let max_x = regions
            .iter()
            .map(|r| r.x)
            .max()?
            .checked_add(1)?
            .checked_mul(64)?;
        let max_y = regions
            .iter()
            .map(|r| r.y)
            .max()?
            .checked_add(1)?
            .checked_mul(64)?;
        let size = (max_x.checked_sub(min_x)?, max_y.checked_sub(min_y)?);
        if size.0 > 65536 || size.1 > 65536 {
            return None;
        }
        Some(Self {
            origin: MapTile::new(min_x, min_y),
            size,
            generation,
            regions,
        })
    }

    pub(in crate::app) fn contains(&self, tile: MapTile) -> bool {
        self.regions.contains(&tile.split_region().0)
    }

    pub(in crate::app) fn in_bounds(&self, tile: MapTile) -> bool {
        tile.x >= self.origin.x
            && tile.y >= self.origin.y
            && tile.x < self.origin.x + self.size.0
            && tile.y < self.origin.y + self.size.1
    }

    pub(in crate::app) fn local(&self, point: (f64, f64)) -> (f64, f64) {
        (
            point.0 - f64::from(self.origin.x),
            point.1 - f64::from(self.origin.y),
        )
    }
}

impl InteractionRuntime {
    /// Capture a prospective map while no points are placed. The first click freezes it. A new
    /// source generation discards the entire old gesture and its in-flight worker.
    pub(in crate::app) fn prepare_spline_extent(
        &mut self,
        regions: impl IntoIterator<Item = RegionCoord>,
        generation: u64,
    ) {
        if self
            .spline
            .extent
            .as_ref()
            .is_some_and(|e| e.generation != generation)
        {
            self.clear_spline_points();
        }
        if self.spline.points.is_empty() {
            self.spline.extent = PathExtent::capture(regions, generation);
        }
    }

    pub(in crate::app) fn spline_extent(&self) -> Option<&PathExtent> {
        self.spline.extent.as_ref()
    }
}
