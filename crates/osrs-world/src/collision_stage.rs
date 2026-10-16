//! Compose loaded-terrain coverage and source-plane floor clipping before loc placement.

use crate::LoadedWindow;
use osrs_core::coords::{MapTile, SourcePlane};
use osrs_scene::collision::CollisionSnapshotBuilder;

pub(crate) fn terrain_collision(loaded: &LoadedWindow) -> CollisionSnapshotBuilder {
    let origin = MapTile::new(loaded.window.base_x, loaded.window.base_y);
    let mut collision = CollisionSnapshotBuilder::reference(origin);
    for (region, _) in &loaded.locations {
        collision.load_region(*region);
    }
    for plane in (0..4).filter_map(SourcePlane::new) {
        for x in 0..loaded.grid.size_x() {
            for y in 0..loaded.grid.size_y() {
                let Some((wx, wy)) = origin
                    .x
                    .checked_add(x as i32)
                    .zip(origin.y.checked_add(y as i32))
                else {
                    continue;
                };
                collision.terrain(
                    plane,
                    MapTile::new(wx, wy),
                    loaded.grid.settings(usize::from(plane.index().get()), x, y),
                    loaded.grid.settings(1, x, y) & 2 != 0,
                );
            }
        }
    }
    collision
}

#[cfg(test)]
mod tests {
    use super::*;
    use osrs_cache::decode::DecodedLocations;
    use osrs_core::coords::RegionCoord;
    use osrs_scene::{
        collision::FLOOR,
        terrain_load::{RegionTerrain, RegionTerrainTile, RegionTileHeight, TerrainLoadGrid},
    };

    #[test]
    fn loaded_window_keeps_missing_land_unknown_and_projects_source_floor_flags()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut tiles = vec![
            RegionTerrainTile {
                height: RegionTileHeight::Default,
                overlay: None,
                settings: None,
                underlay: 0
            };
            4 * 64 * 64
        ];
        tiles[32 * 64 + 32].settings = Some(1);
        tiles[(64 + 32) * 64 + 32].settings = Some(2);
        tiles[(2 * 64 + 32) * 64 + 32].settings = Some(1);
        let terrain = RegionTerrain::new(tiles)?;
        let mut grid = TerrainLoadGrid::reference();
        grid.load_region(&terrain, (0, 0), (0, 0), 0);
        grid.fill_missing_region(64, 64, 64, 64);
        let region = RegionCoord::new(0, 0);
        let loaded = LoadedWindow {
            window: crate::SceneWindow::new(0, 0).ok_or("window")?,
            grid,
            locations: vec![(region, DecodedLocations::with_locations(region, Vec::new()))],
        };
        let snapshot = terrain_collision(&loaded).build();
        let zero = SourcePlane::new(0).ok_or("plane")?.index();
        let one = SourcePlane::new(1).ok_or("plane")?.index();
        assert_eq!(
            snapshot
                .cell(zero, MapTile::new(32, 32))
                .ok_or("cell")?
                .flags,
            0
        );
        let bridged = snapshot.cell(one, MapTile::new(32, 32)).ok_or("cell")?;
        assert_eq!(bridged.flags, FLOOR);
        assert_eq!(bridged.terrain_source_planes, 1 << 2);
        assert!(bridged.loaded);
        assert!(
            !snapshot
                .cell(zero, MapTile::new(80, 80))
                .ok_or("missing cell")?
                .loaded
        );
        Ok(())
    }
}
