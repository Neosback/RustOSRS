//! Neighboring map context, resolved terrain and semantic clipping, including invisible locs.
use osrs_core::{
    coords::{MapTile, RegionCoord, SceneTile, SourcePlane},
    definitions::{LocType, ObjectDefinition},
    ids::ObjectId,
};
use osrs_scene::{
    PlacementInput,
    building_census::{CensusArea, Ground, ObjectInfo, ObjectTable},
    collision::{CollisionObject, CollisionSnapshot, CollisionSnapshotBuilder},
    object_semantics::Placed,
    plan_definition_side_effects, plan_placement,
    terrain_load::TerrainLoadGrid,
};
use osrs_world::WorldDefinitions;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
pub(super) struct Context {
    pub area: CensusArea,
    pub placements: Vec<Placed>,
    pub coverage: BTreeSet<RegionCoord>,
    pub collision: CollisionSnapshot,
    pub rotations: Vec<u8>,
}
pub(super) fn load(
    definitions: &mut WorldDefinitions,
    region: RegionCoord,
    table: &mut ObjectTable,
    symbols: &std::collections::HashMap<u32, String>,
) -> Result<Context, Box<dyn std::error::Error>> {
    let origin = ((region.x - 1) * 64, (region.y - 1) * 64);
    let mut grid = TerrainLoadGrid::new(192, 192)?;
    let mut placements = Vec::new();
    let mut coverage = BTreeSet::new();
    let mut defs = BTreeMap::<ObjectId, Arc<ObjectDefinition>>::new();
    for rx in region.x - 1..=region.x + 1 {
        for ry in region.y - 1..=region.y + 1 {
            let key = RegionCoord::new(rx, ry);
            let Some(map) = definitions.region(key)? else {
                continue;
            };
            coverage.insert(key);
            grid.load_region(
                &map.terrain,
                ((rx - region.x + 1) * 64, (ry - region.y + 1) * 64),
                origin,
                0,
            );
            for p in map.locations.locations() {
                let Some(d) = definitions.object(p.object_id)? else {
                    return Err(format!(
                        "Missing definition {} in {},{}",
                        p.object_id.get(),
                        rx,
                        ry
                    )
                    .into());
                };
                table.entry(p.object_id).or_insert_with(|| ObjectInfo {
                    name: d.name.clone().unwrap_or_default().to_lowercase(),
                    symbol: symbols.get(&p.object_id.get()).cloned().unwrap_or_default(),
                    actions: d
                        .actions
                        .iter()
                        .flatten()
                        .map(|s| s.to_lowercase())
                        .collect(),
                    size: (d.size_x.min(255) as u8, d.size_y.min(255) as u8),
                });
                defs.insert(p.object_id, d);
                placements.push(Placed {
                    id: p.object_id,
                    kind: p.loc_type,
                    orientation: p.orientation,
                    tile: (
                        rx * 64 + i32::from(p.tile.x()),
                        ry * 64 + i32::from(p.tile.y()),
                    ),
                    plane: p.source_plane.index().get(),
                });
            }
        }
    }
    placements.sort_by_key(|p| {
        (
            p.plane,
            p.tile.1,
            p.tile.0,
            p.kind,
            p.id.get(),
            p.orientation,
        )
    });
    let mut area = CensusArea::new(origin, 192, 192);
    let mut rotations = Vec::with_capacity(4 * 192 * 192);
    let mut collision =
        CollisionSnapshotBuilder::reference(MapTile::new(region.x * 64 - 20, region.y * 64 - 20));
    for &key in &coverage {
        collision.load_region(key);
    }
    for p in 0..4u8 {
        let plane = SourcePlane::new(p).ok_or("plane")?;
        for x in 0..192 {
            for y in 0..192 {
                let tile = MapTile::new(origin.0 + x as i32, origin.1 + y as i32);
                let known = coverage.contains(&tile.split_region().0);
                let settings = grid.settings(usize::from(p), x, y);
                rotations.push(grid.rotation(usize::from(p), x, y));
                area.set(
                    p,
                    (tile.x, tile.y),
                    Ground {
                        underlay: grid.underlay(usize::from(p), x, y),
                        overlay: grid.overlay_raw(usize::from(p), x, y) & 0x7fff,
                        shape: grid.shape(usize::from(p), x, y),
                        settings,
                        height: grid.height(usize::from(p), x, y),
                        height_known: known,
                    },
                );
                if known {
                    collision.terrain(plane, tile, settings, grid.settings(1, x, y) & 2 != 0);
                }
            }
        }
    }
    for p in &placements {
        let definition = &defs[&p.id];
        let tile = SceneTile::new((p.tile.0 - origin.0) as u32, (p.tile.1 - origin.1) as u32);
        let plan = plan_placement(PlacementInput {
            loc_type: LocType::new(p.kind),
            orientation: p.orientation,
            tile,
            size_x: definition.size_x,
            size_y: definition.size_y,
            sampled_height: 0,
            existing_wall_displacement: None,
        })?;
        let source_plane = SourcePlane::new(p.plane).ok_or("plane")?;
        let bridge = area.ground(1, p.tile).is_some_and(|g| g.settings & 2 != 0);
        collision.object(
            CollisionObject {
                object_id: p.id,
                tile: MapTile::new(p.tile.0, p.tile.1),
                source_plane,
                loc_type: LocType::new(p.kind),
                orientation: p.orientation,
                effect: plan_definition_side_effects(definition, plan).collision,
            },
            bridge,
        )?;
    }
    Ok(Context {
        area,
        placements,
        coverage,
        collision: collision.build(),
        rotations,
    })
}
