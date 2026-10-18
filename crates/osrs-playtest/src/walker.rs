//! The player's walking, headless: a route of waypoints followed one validated step per server
//! tick (two when running), the same step order and collision rules the NPC runtime uses. It owns
//! no rendering and no map; callers hand it a [`Navigation`] view each tick and read back where
//! the player stands, where it stood a tick ago (for smooth drawing) and which way it faces.

use crate::{
    navigation::{Navigation, Route, RouteTarget, find_path, validated_step},
    runtime::actor::orientation,
};
use osrs_core::{
    coords::MapTile,
    scenario::{ActorLocation, MovementRestriction},
};
use std::collections::VecDeque;

/// What asking the player to walk somewhere came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalkOutcome {
    /// A route was found (to the tile, or as near as it can get).
    Walking { exact: bool },
    AlreadyThere,
    /// Nothing reachable: collision is missing, or the tile is walled off.
    Unreachable,
}

#[derive(Debug, Clone)]
pub struct PlayerWalker {
    location: ActorLocation,
    /// Where the player stood when the last tick began.
    previous: MapTile,
    waypoints: VecDeque<ActorLocation>,
    orientation: u16,
}

impl PlayerWalker {
    pub fn new(location: ActorLocation) -> Self {
        Self {
            location,
            previous: location.tile,
            waypoints: VecDeque::new(),
            orientation: 0,
        }
    }

    pub fn location(&self) -> ActorLocation {
        self.location
    }

    /// The tile the player stood on at the start of the last tick.
    pub fn previous(&self) -> MapTile {
        self.previous
    }

    /// Facing in client units (0 south, 512 west, 1024 north, 1536 east).
    pub fn orientation(&self) -> u16 {
        self.orientation
    }

    pub fn is_moving(&self) -> bool {
        !self.waypoints.is_empty()
    }

    /// Stand somewhere without walking: no route, no interpolation.
    pub fn teleport(&mut self, location: ActorLocation) {
        *self = Self {
            orientation: self.orientation,
            ..Self::new(location)
        };
    }

    pub fn stop(&mut self) {
        self.waypoints.clear();
    }

    /// Route to `target`, replacing any route being walked. When the tile itself cannot be
    /// reached the player goes as near as it can, as the client does.
    pub fn walk_to(&mut self, view: &dyn Navigation, target: ActorLocation) -> WalkOutcome {
        let route = find_path(
            view,
            self.location,
            RouteTarget {
                location: target,
                width: 1,
                height: 1,
            },
            1,
            MovementRestriction::Normal,
            true,
        );
        match route {
            Route::Found { waypoints, exact } => {
                self.waypoints = waypoints.into();
                WalkOutcome::Walking { exact }
            }
            Route::AlreadyThere => {
                self.waypoints.clear();
                WalkOutcome::AlreadyThere
            }
            Route::Unreachable => {
                self.waypoints.clear();
                WalkOutcome::Unreachable
            }
        }
    }

    /// One server tick: `steps` tiles (1 walking, 2 running) along the route. A step collision
    /// refuses ends the route there.
    pub fn tick(&mut self, view: &dyn Navigation, steps: u8) {
        self.previous = self.location.tile;
        for _ in 0..steps {
            while self
                .waypoints
                .front()
                .is_some_and(|waypoint| *waypoint == self.location)
            {
                self.waypoints.pop_front();
            }
            let Some(&waypoint) = self.waypoints.front() else {
                return;
            };
            let Some(next) = validated_step(
                view,
                self.location,
                waypoint,
                1,
                MovementRestriction::Normal,
                0,
            ) else {
                self.waypoints.clear();
                return;
            };
            self.orientation = orientation(
                next.tile.x - self.location.tile.x,
                next.tile.y - self.location.tile.y,
            );
            self.location = next;
        }
        while self
            .waypoints
            .front()
            .is_some_and(|waypoint| *waypoint == self.location)
        {
            self.waypoints.pop_front();
        }
    }
}

#[cfg(test)]
mod tests;
