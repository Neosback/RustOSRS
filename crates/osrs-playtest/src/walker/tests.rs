use super::*;
use crate::navigation::{NavigationCell, flags};
use osrs_core::coords::PlaneIndex;

struct Field {
    blocked: Vec<(i32, i32)>,
}

impl Navigation for Field {
    fn cell(&self, location: ActorLocation) -> Option<NavigationCell> {
        let (x, y) = (location.tile.x, location.tile.y);
        if !(0..100).contains(&x) || !(0..100).contains(&y) {
            return None;
        }
        Some(NavigationCell {
            flags: if self.blocked.contains(&(x, y)) {
                flags::LOC
            } else {
                flags::OPEN
            },
            roof: false,
        })
    }
}

fn at(x: i32, y: i32) -> ActorLocation {
    ActorLocation {
        tile: MapTile::new(x, y),
        plane: PlaneIndex::new(0).unwrap_or_else(|| unreachable!("plane 0")),
    }
}

fn walk_until_still(walker: &mut PlayerWalker, field: &Field, steps: u8) -> Vec<(i32, i32)> {
    let mut stood = Vec::new();
    for _ in 0..200 {
        if !walker.is_moving() {
            break;
        }
        walker.tick(field, steps);
        stood.push((walker.location().tile.x, walker.location().tile.y));
    }
    stood
}

#[test]
fn walking_takes_one_tile_a_tick_and_faces_the_way_it_goes() {
    let field = Field { blocked: vec![] };
    let mut walker = PlayerWalker::new(at(10, 10));
    assert_eq!(
        walker.walk_to(&field, at(14, 10)),
        WalkOutcome::Walking { exact: true }
    );
    walker.tick(&field, 1);
    assert_eq!(walker.location(), at(11, 10));
    assert_eq!(walker.previous(), MapTile::new(10, 10));
    assert_eq!(walker.orientation(), 1536, "east");
    let rest = walk_until_still(&mut walker, &field, 1);
    assert_eq!(rest.len(), 3);
    assert_eq!(walker.location(), at(14, 10));
    assert!(!walker.is_moving());
}

#[test]
fn running_takes_two_tiles_a_tick() {
    let field = Field { blocked: vec![] };
    let mut walker = PlayerWalker::new(at(10, 10));
    walker.walk_to(&field, at(10, 16));
    walker.tick(&field, 2);
    assert_eq!(walker.location(), at(10, 12));
    assert_eq!(walker.orientation(), 1024, "north");
    walk_until_still(&mut walker, &field, 2);
    assert_eq!(walker.location(), at(10, 16));
}

#[test]
fn a_wall_is_walked_around_never_through() {
    let wall: Vec<(i32, i32)> = (4..=16).map(|y| (12, y)).collect();
    let field = Field {
        blocked: wall.clone(),
    };
    let mut walker = PlayerWalker::new(at(10, 10));
    assert_eq!(
        walker.walk_to(&field, at(14, 10)),
        WalkOutcome::Walking { exact: true }
    );
    let stood = walk_until_still(&mut walker, &field, 1);
    assert_eq!(walker.location(), at(14, 10));
    assert!(stood.iter().all(|tile| !wall.contains(tile)));
    assert!(stood.len() > 4, "the detour is longer than the direct line");
}

#[test]
fn an_enclosed_tile_is_approached_not_entered() {
    let ring: Vec<(i32, i32)> = (13..=15)
        .flat_map(|x| (13..=15).map(move |y| (x, y)))
        .filter(|tile| *tile != (14, 14))
        .collect();
    let field = Field { blocked: ring };
    let mut walker = PlayerWalker::new(at(10, 10));
    assert_eq!(
        walker.walk_to(&field, at(14, 14)),
        WalkOutcome::Walking { exact: false }
    );
    walk_until_still(&mut walker, &field, 1);
    let end = walker.location().tile;
    assert_ne!((end.x, end.y), (14, 14));
    assert!((end.x - 14).abs() <= 2 && (end.y - 14).abs() <= 2);
}

#[test]
fn standing_still_and_teleporting() {
    let field = Field { blocked: vec![] };
    let mut walker = PlayerWalker::new(at(10, 10));
    assert_eq!(
        walker.walk_to(&field, at(10, 10)),
        WalkOutcome::AlreadyThere
    );
    walker.walk_to(&field, at(20, 10));
    walker.tick(&field, 1);
    walker.teleport(at(30, 30));
    assert!(!walker.is_moving());
    assert_eq!(walker.location(), at(30, 30));
    assert_eq!(walker.previous(), MapTile::new(30, 30));
}

#[test]
fn a_tile_with_no_collision_loaded_is_unreachable() {
    let field = Field { blocked: vec![] };
    let mut walker = PlayerWalker::new(at(10, 10));
    assert_eq!(
        walker.walk_to(&field, at(500, 500)),
        WalkOutcome::Unreachable
    );
    assert!(!walker.is_moving());
}
