use super::actor::Actor;
use crate::navigation::{Navigation, NavigationCell, flags};
use osrs_core::{coords::MapTile, scenario::ActorLocation};
use std::collections::BTreeMap;

/// Counts preserve shared occupancy when overlapping actors move or despawn.
pub(super) struct Occupancy {
    cells: BTreeMap<ActorLocation, u32>,
}
impl Occupancy {
    pub fn new<'a>(actors: impl Iterator<Item = &'a Actor>) -> Self {
        let mut value = Self {
            cells: BTreeMap::new(),
        };
        for actor in actors {
            value.add(actor);
        }
        value
    }
    pub fn add(&mut self, actor: &Actor) {
        if actor.occupies() {
            visit(actor, |location| {
                *self.cells.entry(location).or_default() += 1;
            });
        }
    }
    pub fn remove(&mut self, actor: &Actor) {
        if actor.occupies() {
            visit(actor, |location| {
                if let Some(count) = self.cells.get_mut(&location) {
                    *count -= 1;
                    if *count == 0 {
                        self.cells.remove(&location);
                    }
                }
            });
        }
    }
    pub fn view<'a>(&'a self, base: &'a dyn Navigation) -> impl Navigation + 'a {
        View {
            base,
            cells: &self.cells,
        }
    }
}
fn visit(actor: &Actor, mut f: impl FnMut(ActorLocation)) {
    for x in 0..i32::from(actor.size) {
        for y in 0..i32::from(actor.size) {
            f(ActorLocation {
                tile: MapTile::new(actor.location.tile.x + x, actor.location.tile.y + y),
                plane: actor.location.plane,
            });
        }
    }
}
struct View<'a> {
    base: &'a dyn Navigation,
    cells: &'a BTreeMap<ActorLocation, u32>,
}
impl Navigation for View<'_> {
    fn cell(&self, location: ActorLocation) -> Option<NavigationCell> {
        let mut cell = self.base.cell(location)?;
        if self.cells.contains_key(&location) {
            cell.flags |= flags::NPC;
        }
        Some(cell)
    }
}
