use super::*;
use crate::{BuildingArchetype,BuildingConstraints,BuildingStyle,PrefabRoom,PrefabPortal,RectSite,derive_building_architecture};
fn request() -> BuildingRequest {
    BuildingRequest {site:RectSite::new(MapTile::new(0,0),10,10).unwrap(),seed:17,base_plane:osrs_core::coords::SourcePlane::new(0).unwrap(),floors:1,archetype:BuildingArchetype::House,style:BuildingStyle::Generic,constraints:BuildingConstraints::default()}
}
fn cell(x:u16,y:u16)->PrefabCell {PrefabCell {x,y,plane:0}}
#[test]
fn sparse_l_room_keeps_the_drawn_footprint_and_opens_its_decomposition_seams() {
    let floors:Vec<_>=(1..=8).flat_map(|x|(1..=8).filter_map(move|y| (x<=3 || y<=3).then_some(cell(x,y)))).collect();
    let sketch=BuildingSketch {origin:MapTile::new(0,0),semantics:PrefabSemantics {rooms:vec![PrefabRoom {id:1,role:"living".into(),cells:floors.clone(),human:true}],..Default::default()},floors};
    let result=plan_authored_rooms(request(),&sketch).unwrap();
    let actual:BTreeSet<_>=result.plan.rooms.iter().flat_map(|r|rect_tiles(r.rect).unwrap()).collect();
    let expected:BTreeSet<_>=sketch.floors.iter().map(|c|MapTile::new(i32::from(c.x),i32::from(c.y))).collect();
    assert_eq!(actual,expected);
    assert!(result.plan.connections.iter().filter(|c|c.from.is_some()).all(|c|c.kind==PortalKind::Open));
    assert!(derive_building_architecture(&result.plan).is_ok());
    assert_eq!(result,plan_authored_rooms(request(),&sketch).unwrap());
}
#[test]
fn explicit_portal_connects_requested_room_roles_and_disconnected_programs_fail() {
    let left:Vec<_>=(1..=4).flat_map(|x|(1..=8).map(move|y|cell(x,y))).collect();
    let right:Vec<_>=(5..=8).flat_map(|x|(1..=8).map(move|y|cell(x,y))).collect();
    let mut sketch=BuildingSketch {origin:MapTile::new(0,0),floors:left.iter().chain(&right).copied().collect(),semantics:PrefabSemantics {rooms:vec![PrefabRoom {id:1,role:"living".into(),cells:left,human:true},PrefabRoom {id:2,role:"bedroom".into(),cells:right,human:true}],portals:vec![PrefabPortal {object:None,rooms:[Some(1),Some(2)],approaches:[cell(4,3),cell(5,3)],human:true}],..Default::default()}};
    let result=plan_authored_rooms(request(),&sketch).unwrap();
    assert!(result.plan.connections.iter().any(|c|c.from.is_some() && c.doorway==MapTile::new(4,3)));
    assert!(derive_building_architecture(&result.plan).is_ok());
    sketch.semantics.rooms[1].cells.retain(|c|c.x>=6);
    sketch.floors.retain(|c|c.x!=5);
    assert!(plan_authored_rooms(request(),&sketch).is_err());
}
#[test]
fn courtyard_hole_is_not_filled_by_room_partitioning() {
    let floors:Vec<_>=(1..=8).flat_map(|x|(1..=8).filter_map(move|y| (!(4..=5).contains(&x) || !(4..=5).contains(&y)).then_some(cell(x,y)))).collect();
    let sketch=BuildingSketch {origin:MapTile::new(0,0),floors,semantics:Default::default()};
    let result=plan_authored_rooms(request(),&sketch).unwrap();
    assert!(!result.plan.rooms.iter().any(|r|r.rect.contains(MapTile::new(4,4))));
    assert!(derive_building_architecture(&result.plan).is_ok());
}
