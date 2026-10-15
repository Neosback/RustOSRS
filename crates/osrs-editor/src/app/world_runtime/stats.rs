use super::{RegionBuildScope, ZoneKey};
use osrs_render::SceneGeometry;
use std::collections::BTreeMap;

/// Status-bar accounting for one resident region, kept per zone so partial refreshes remain exact.
#[derive(Clone, Default)]
pub(super) struct RegionStats {
    zone_vertices: BTreeMap<ZoneKey, usize>,
}

impl RegionStats {
    pub(super) fn from_geometry(geometry: &SceneGeometry) -> Self {
        let mut stats = Self::default();
        stats.replace_all(geometry);
        stats
    }

    pub(super) fn refresh(&mut self, scope: &RegionBuildScope, geometry: &SceneGeometry) {
        match scope {
            RegionBuildScope::Full => self.replace_all(geometry),
            RegionBuildScope::Zones(requested) => {
                for zone in requested {
                    self.zone_vertices.remove(zone);
                }
                for zone in &geometry.zones {
                    let key = (zone.zone_x, zone.zone_z);
                    if requested.contains(&key) {
                        let vertices = zone.vertex_count();
                        if vertices != 0 {
                            self.zone_vertices.insert(key, vertices);
                        }
                    }
                }
            }
        }
    }

    pub(super) fn vertices(&self) -> usize {
        self.zone_vertices.values().sum()
    }

    fn replace_all(&mut self, geometry: &SceneGeometry) {
        self.zone_vertices.clear();
        self.zone_vertices.extend(geometry.zones.iter().filter_map(|zone| {
            let vertices = zone.vertex_count();
            (vertices != 0).then_some(((zone.zone_x, zone.zone_z), vertices))
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use osrs_render::PackedVertex;
    use std::collections::BTreeSet;

    fn vertex() -> PackedVertex {
        PackedVertex {
            position: [0; 4],
            abhsl: 0,
            texture: 0,
            uv: [0; 2],
            pad: 0,
        }
    }

    fn geometry(zones: &[(ZoneKey, usize)]) -> SceneGeometry {
        let mut geometry = SceneGeometry::default();
        for &(zone, count) in zones {
            geometry
                .zone_mut(zone.0, zone.1)
                .group_mut(0, 0)
                .opaque
                .extend(std::iter::repeat_n(vertex(), count));
        }
        geometry
    }

    #[test]
    fn full_refresh_replaces_all_zone_counts() {
        let mut stats = RegionStats::from_geometry(&geometry(&[((400, 400), 3), ((401, 400), 6)]));
        assert_eq!(stats.vertices(), 9);

        stats.refresh(
            &RegionBuildScope::Full,
            &geometry(&[((400, 400), 12)]),
        );
        assert_eq!(stats.vertices(), 12);
    }

    #[test]
    fn partial_refresh_replaces_only_requested_zone_counts() {
        let mut stats = RegionStats::from_geometry(&geometry(&[
            ((400, 400), 3),
            ((401, 400), 6),
            ((402, 400), 4),
        ]));
        let requested = BTreeSet::from([(400, 400), (402, 400)]);

        stats.refresh(
            &RegionBuildScope::Zones(requested),
            &geometry(&[((400, 400), 9)]),
        );

        assert_eq!(stats.vertices(), 15);
    }
}
