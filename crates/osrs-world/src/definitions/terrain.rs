//! Semantic terrain projection into worker-owned disposable mirrors.
use super::WorldDefinitions;
impl WorldDefinitions {
    /// Record editor changes; later builds of the affected regions include them.
    /// Install absolute semantic height overrides after loading the source grid.
    pub(crate) fn project_heights(
        &self,
        grid: &mut osrs_scene::terrain_load::TerrainLoadGrid,
        origin: osrs_core::coords::MapTile,
    ) {
        self.edits.project_heights(grid, origin);
    }

    pub fn apply_edits(&mut self, edits: &[crate::TileEdit]) {
        self.edits.apply(edits);
    }
}
