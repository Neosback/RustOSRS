//! Read-only adapter from accepted world collision snapshots to the playtest `Navigation` view.
use super::super::world_runtime::{CollisionInspection, WorldRuntime};
use osrs_core::{coords::SourcePlane, scenario::ActorLocation};
use osrs_edit::{MapDocument, MapPosition};
use osrs_playtest::navigation::{Navigation, NavigationCell};

/// Terrain setting bit marking a roofed tile (`Indoors` / `Outdoors` restrictions read it).
/// This is the tile's own plane setting, not a full client roof-visibility resolution.
const ROOF_FLAG: u8 = 4;

pub(in crate::app) struct WorldNavigation<'a> {
    pub world: &'a WorldRuntime,
    pub map: &'a MapDocument,
}

impl Navigation for WorldNavigation<'_> {
    /// `None` for any tile whose collision is absent, stale or still rebuilding, so actors park
    /// instead of walking through unverified cells.
    fn cell(&self, location: ActorLocation) -> Option<NavigationCell> {
        let CollisionInspection::Ready(snapshot) = self.world.collision_inspection(location.tile)
        else {
            return None;
        };
        let cell = snapshot.cell(location.plane, location.tile)?;
        let plane = SourcePlane::new(location.plane.get())?;
        let roof = self
            .map
            .terrain_at(MapPosition::new(location.tile, plane))
            .flags
            & ROOF_FLAG
            != 0;
        NavigationCell::from_client(cell.flags, cell.loaded, roof)
    }
}
