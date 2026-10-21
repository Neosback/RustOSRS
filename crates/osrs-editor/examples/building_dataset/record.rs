//! Exact placement records, sparse ownership and confidence-stamped relationship evidence.
use super::{areas::Area, context::Context};
use osrs_core::coords::{CollisionPlane, MapTile, SourcePlane};
use osrs_scene::building_census::{Building, ObjectTable, description::BuildingDescription};
use serde_json::{Value, json};
use std::collections::BTreeSet;
pub(super) fn record(
    context: &Context,
    description: &BuildingDescription,
    building: &Building,
    objects: &ObjectTable,
    areas: &[Area],
    source: &Value,
) -> Value {
    let floor: Vec<_> = description
        .floors
        .iter()
        .flat_map(|(&p, ts)| ts.iter().map(move |t| (p, *t)))
        .collect();
    let min = MapTile::new(
        floor.iter().map(|(_, t)| t.x).min().unwrap_or(0),
        floor.iter().map(|(_, t)| t.y).min().unwrap_or(0),
    );
    let max = MapTile::new(
        floor.iter().map(|(_, t)| t.x).max().unwrap_or(0),
        floor.iter().map(|(_, t)| t.y).max().unwrap_or(0),
    );
    let base = *description.floors.keys().next().unwrap_or(&0);
    let key = description.floors[&base]
        .iter()
        .next()
        .copied()
        .unwrap_or(min);
    let region = key.split_region().0;
    let group = format!(
        "geography:{}:{}",
        region.x.div_euclid(8),
        region.y.div_euclid(8)
    );
    let split = osrs_lab::LearningSplit::for_group(&group);
    let named: Vec<_> = areas
        .iter()
        .filter(|a| {
            a.boxes.iter().any(|b| {
                b.contains(min.x, min.y, i32::from(base))
                    && b.contains(max.x, max.y, i32::from(base))
            })
        })
        .map(|a| a.name.as_str())
        .collect();
    let mut tiles = BTreeSet::new();
    let mut vertices = BTreeSet::new();
    for &(p, t) in &floor {
        for dx in -2..=2 {
            for dy in -2..=2 {
                tiles.insert((p, MapTile::new(t.x + dx, t.y + dy)));
            }
        }
    }
    for m in &description.members {
        let p = m.placement;
        tiles.insert((p.plane, MapTile::new(p.tile.0, p.tile.1)));
    }
    for &(p, t) in &tiles {
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            vertices.insert((p, MapTile::new(t.x + dx, t.y + dy)));
        }
    }
    let terrain:Vec<_>=tiles.iter().filter_map(|&(p,t)|context.area.ground(p,(t.x,t.y)).filter(|g|g.height_known).map(|g|json!({"tile":[t.x-min.x,t.y-min.y],"plane":p,"underlay":g.underlay,"overlay":g.overlay,"shape":g.shape,"rotation":context.rotations[(usize::from(p)*192+(t.x-context.area.origin.0) as usize)*192+(t.y-context.area.origin.1) as usize],"flags":g.settings}))).collect();
    let heights: Vec<_> = vertices
        .iter()
        .filter_map(|&(p, t)| {
            context
                .area
                .ground(p, (t.x, t.y))
                .filter(|g| g.height_known)
                .map(|g| json!({"vertex":[t.x-min.x,t.y-min.y],"plane":p,"height":g.height}))
        })
        .collect();
    let edge_grids: std::collections::BTreeMap<_, _> = description
        .floors
        .keys()
        .map(|&p| {
            (
                p,
                osrs_scene::building_census::Edges::of_plane(&context.placements, p, objects),
            )
        })
        .collect();
    let sockets:Vec<_>=floor.iter().map(|&(p,t)|json!({"tile":[t.x-min.x,t.y-min.y],"plane":p,"order":"north,east,south,west","sockets":([osrs_scene::building_census::Dir::North,osrs_scene::building_census::Dir::East,osrs_scene::building_census::Dir::South,osrs_scene::building_census::Dir::West].map(|d|edge_grids[&p].between((t.x,t.y),d).map_or_else(||"Open".into(),|e|format!("{:?}",e.feature))))})).collect();
    let walkable:Vec<_>=floor.iter().map(|&(p,t)|{
        let bridge=context.area.ground(1,(t.x,t.y)).is_some_and(|g|g.settings & 2 != 0);
        let projected=SourcePlane::new(p).map(|s|osrs_scene::collision_plane(s,bridge));
        let plane=match projected {Some(CollisionPlane::Plane(p))=>Some(p),_=>None};
        let cell=plane.and_then(|p|context.collision.cell(p,t));
        json!({"tile":[t.x-min.x,t.y-min.y],"source_plane":p,"collision_plane":plane.map(|p|p.get()),"loaded":cell.is_some_and(|c|c.loaded),"flags":cell.map(|c|c.flags),"walkable":cell.filter(|c|c.loaded).map(|c|c.flags&(osrs_scene::collision::OBJECT|osrs_scene::collision::FLOOR|osrs_scene::collision::FLOOR_DECORATION)==0)})
    }).collect();
    let complete = floor.iter().all(|(_, t)| {
        (-1..=1).all(|dx| {
            (-1..=1).all(|dy| {
                context
                    .coverage
                    .contains(&MapTile::new(t.x + dx, t.y + dy).split_region().0)
            })
        })
    });
    let occurrences:Vec<_>=description.members.iter().map(|m|{let p=m.placement;json!({"object":p.id.get(),"tile":[p.tile.0-min.x,p.tile.1-min.y],"plane":p.plane,"kind":p.kind,"orientation":p.orientation,"role":format!("{:?}",m.role),"evidence":format!("{:?}",m.evidence),"confidence":if matches!(m.evidence,osrs_scene::building_census::description::Evidence::BoundaryIncident|osrs_scene::building_census::description::Evidence::SameAnchorBoundary){"structural"}else{"inferred"}})}).collect();
    let rooms:Vec<_>=building.rooms.iter().filter(|r|r.tiles.iter().any(|&(x,y)|description.floors.get(&r.plane).is_some_and(|f|f.contains(&MapTile::new(x,y))))).map(|r|json!({"plane":r.plane,"tiles":r.tiles.iter().map(|&(x,y)|[x-min.x,y-min.y]).collect::<Vec<_>>(),"tag":r.tag.word(),"label_source":"word_and_furniture_inference","confirmed":false})).collect();
    let walls:Vec<_>=description.boundaries.iter().map(|w|json!({"member":w.member,"tile":[w.tile.x-min.x,w.tile.y-min.y],"direction":format!("{:?}",w.side),"feature":format!("{:?}",w.feature),"exterior":w.exterior})).collect();
    let relations:Vec<_>=description.connections.iter().map(|r|json!({"from":r.from,"to":r.to,"kind":format!("{:?}",r.kind),"evidence":format!("{:?}",r.evidence),"source":"inference"})).collect();
    let portals:Vec<_>=description.portals.iter().map(|p|json!({"member":p.member,"approaches":p.approaches.map(|t|[t.x-min.x,t.y-min.y]),"exterior":p.exterior,"collision_state":"initial_cache_state"})).collect();
    let _ = objects;
    // The same numbers the lab's judge gives a generated building, so a model trained on real
    // buildings can score generated ones.
    let judgement = osrs_scene::building_census::judge(building);
    let judgement = json!({
        "usable": judgement.is_usable(),
        "problems": judgement.problems(),
        "rooms": judgement.rooms,
        "entrances": judgement.entrances,
        "rooms_reachable": judgement.rooms_reachable,
        "blocked_doorways": judgement.blocked_doorways,
        "features": osrs_scene::building_census::FEATURE_NAMES
            .iter()
            .zip(judgement.features)
            .map(|(name, value)| (name.to_string(), json!(value)))
            .collect::<serde_json::Map<String, Value>>(),
    });
    json!({"format":"rustosrs-building-dataset","version":2,"id":format!("b{base}_{}_{}",key.x,key.y),"source":source,"region":[region.x,region.y],"origin":[min.x,min.y],"size":[max.x-min.x+1,max.y-min.y+1],"group":group,"split":split,"reviewed":false,"coverage_complete":complete,"review":description.review,"area_names":named,"area_label_source":"117hd_weak","rooms":rooms,"floor_tiles":floor.iter().map(|(p,t)|json!({"tile":[t.x-min.x,t.y-min.y],"plane":p})).collect::<Vec<_>>(),"objects":occurrences,"walls":walls,"portals":portals,"relations":relations,"terrain":terrain,"height_vertices":heights,"walkability":walkable,"edge_socket_grid":sockets,"judgement":judgement})
}
