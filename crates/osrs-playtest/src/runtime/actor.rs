use super::{MoveSpeed, RuntimeAssets};
use crate::navigation::{Navigation, flags, validated_step};
use osrs_core::{
    actor_snapshot::{ActorStatus, NpcActorSnapshot},
    coords::{LocalCoord, LocalXZ, MapTile},
    entity::{ActorAnimationState, NpcDefinition, NpcMovementSequences},
    scenario::{ActorBlocking, ActorLocation, MovementRestriction, NpcBehavior, NpcSpawn},
};
use std::collections::VecDeque;

pub(super) struct Actor {
    pub spawn: NpcSpawn,
    pub location: ActorLocation,
    pub size: u16,
    pub status: ActorStatus,
    pub destination: VecDeque<ActorLocation>,
    pub speed: MoveSpeed,
    pub manual: bool,
    pub sequences: ActorAnimationState,
    position: LocalXZ,
    orientation: u16,
    rotation: u16,
    rotation_speed: u16,
    movement: NpcMovementSequences,
    rng: u64,
    moved: bool,
    idle: u32,
    waypoint: usize,
    pause: u32,
}
impl Actor {
    pub fn new(spawn: &NpcSpawn, definition: Option<&NpcDefinition>, seed: u64) -> Self {
        let size = definition.map_or(1, |d| d.size);
        let mut sequences = ActorAnimationState::default();
        sequences.pose.sequence = definition.and_then(|d| d.movement_sequences.idle);
        sequences.pose.reset_frame();
        Self {
            spawn: spawn.clone(),
            location: spawn.location,
            size,
            status: if definition.is_some() {
                ActorStatus::Ready
            } else {
                ActorStatus::MissingDefinition
            },
            destination: VecDeque::new(),
            speed: MoveSpeed::Walk,
            manual: false,
            sequences,
            position: center(spawn.location, size),
            orientation: spawn.facing,
            rotation: spawn.facing,
            rotation_speed: definition.map_or(32, |d| d.render.rotation_speed),
            movement: definition
                .map_or_else(NpcMovementSequences::default, |d| d.movement_sequences),
            rng: seed ^ spawn.id.0.wrapping_mul(0x9e3779b97f4a7c15),
            moved: false,
            idle: 0,
            waypoint: 0,
            pause: 0,
        }
    }
    pub fn snapshot(&self) -> NpcActorSnapshot {
        NpcActorSnapshot {
            spawn: self.spawn.id,
            npc: self.spawn.npc,
            location: self.location,
            position: self.position,
            orientation: self.orientation,
            rotation: self.rotation,
            size: self.size,
            sequences: self.sequences,
            status: self.status.clone(),
        }
    }
    pub fn revalidate(&mut self, view: &dyn Navigation) {
        if self.status == ActorStatus::MissingDefinition {
            return;
        }
        for x in 0..i32::from(self.size) {
            for y in 0..i32::from(self.size) {
                let location = ActorLocation {
                    tile: MapTile::new(self.location.tile.x + x, self.location.tile.y + y),
                    plane: self.location.plane,
                };
                let Some(cell) = view.cell(location) else {
                    self.status = ActorStatus::Parked;
                    return;
                };
                let terrain = match self.spawn.restriction {
                    MovementRestriction::Blocked => {
                        cell.flags & flags::FLOOR != 0
                            && cell.flags & (flags::LOC | flags::FLOOR_DECORATION) == 0
                    }
                    MovementRestriction::BlockedNormal => cell.flags & flags::LOC_PROJ_BLOCKER == 0,
                    MovementRestriction::Indoors => {
                        cell.roof && cell.flags & flags::WALK_BLOCKED == 0
                    }
                    MovementRestriction::Outdoors => {
                        !cell.roof && cell.flags & flags::WALK_BLOCKED == 0
                    }
                    _ => cell.flags & flags::WALK_BLOCKED == 0,
                };
                if !terrain {
                    self.status = ActorStatus::InvalidPlacement;
                    return;
                }
            }
        }
        self.status = ActorStatus::Ready;
    }
    pub fn server_tick(&mut self, tick: u64, view: &dyn Navigation) {
        self.idle = if self.moved {
            0
        } else {
            self.idle.saturating_add(1)
        };
        self.moved = false;
        if !self.manual {
            match self.spawn.behavior.clone() {
                NpcBehavior::Stationary => {}
                NpcBehavior::Wander { radius } => {
                    if self.idle >= 500 {
                        self.teleport(self.spawn.location, view);
                    } else if radius == 0 {
                        if self.location != self.spawn.location {
                            self.set_destination(self.spawn.location);
                        }
                    } else if self.random(8) == 0 {
                        let width = u32::from(radius) * 2 + 1;
                        let dx = self.random(width) as i32 - i32::from(radius);
                        let dy = self.random(width) as i32 - i32::from(radius);
                        self.set_destination(ActorLocation {
                            tile: MapTile::new(
                                self.spawn.location.tile.x + dx,
                                self.spawn.location.tile.y + dy,
                            ),
                            plane: self.spawn.location.plane,
                        });
                    }
                }
                NpcBehavior::Patrol { waypoints } => {
                    let waypoint = &waypoints[self.waypoint % waypoints.len()];
                    if self.idle > 30 {
                        self.teleport(waypoint.destination, view);
                    }
                    if self.location == waypoint.destination {
                        if waypoint.pause_ticks > self.pause {
                            self.pause += 1;
                            self.idle = 0;
                        } else {
                            self.pause = 0;
                            self.waypoint = (self.waypoint + 1) % waypoints.len();
                            let destination = waypoints[self.waypoint].destination;
                            if destination.plane != self.location.plane {
                                self.teleport(destination, view);
                            } else {
                                self.set_destination(destination);
                            }
                        }
                    } else {
                        self.set_destination(waypoint.destination);
                    }
                }
            }
        }
        let before = self.location;
        let extra = if matches!(
            self.spawn.restriction,
            MovementRestriction::Blocked | MovementRestriction::PassThru
        ) {
            0
        } else {
            flags::NPC
        };
        for _ in 0..self.speed.steps(tick) {
            while self.destination.front() == Some(&self.location) {
                self.destination.pop_front();
            }
            let Some(&destination) = self.destination.front() else {
                break;
            };
            if let Some(next) = validated_step(
                view,
                self.location,
                destination,
                self.size,
                self.spawn.restriction,
                extra,
            ) {
                self.location = next;
            } else {
                self.status = ActorStatus::Blocked;
                break;
            }
        }
        self.moved = self.location != before;
        if self.moved {
            self.status = ActorStatus::Ready;
        }
    }
    pub fn client_cycle(&mut self, assets: &RuntimeAssets) {
        if matches!(
            self.status,
            ActorStatus::Parked | ActorStatus::MissingDefinition | ActorStatus::InvalidPlacement
        ) {
            return;
        }
        let target = center(self.location, self.size);
        let dx = target.x.units() - self.position.x.units();
        let dz = target.z.units() - self.position.z.units();
        let moving = dx != 0 || dz != 0;
        let mut pose = self.movement.idle;
        if moving {
            self.orientation = orientation(dx, dz);
            let angle = (i32::from(self.orientation) - i32::from(self.rotation) + 1024)
                .rem_euclid(2048)
                - 1024;
            pose = if (-256..=256).contains(&angle) {
                self.movement.walk
            } else if (256..768).contains(&angle) {
                self.movement.walk_right
            } else if (-768..-256).contains(&angle) {
                self.movement.walk_left
            } else {
                self.movement.walk_back
            };
            if pose.is_none() {
                pose = self.movement.walk;
            }
            let step = match self.speed {
                MoveSpeed::Crawl => 2,
                MoveSpeed::Walk => 4,
                MoveSpeed::Run => 8,
            };
            if matches!(self.speed, MoveSpeed::Run) && pose == self.movement.walk {
                pose = self.movement.run[0].or(pose);
            }
            if matches!(self.speed, MoveSpeed::Crawl) && pose == self.movement.walk {
                pose = self.movement.crawl[0].or(pose);
            }
            self.position = LocalXZ::new(
                LocalCoord::from_units(self.position.x.units() + dx.clamp(-step, step)),
                LocalCoord::from_units(self.position.z.units() + dz.clamp(-step, step)),
            );
        }
        let delta =
            (i32::from(self.orientation) - i32::from(self.rotation) + 1024).rem_euclid(2048) - 1024;
        let turn = i32::from(self.rotation_speed);
        self.rotation =
            (i32::from(self.rotation) + delta.clamp(-turn, turn)).rem_euclid(2048) as u16;
        if !moving && delta != 0 {
            pose = if delta < 0 {
                self.movement.turn_left
            } else {
                self.movement.turn_right
            }
            .or(self.movement.walk)
            .or(pose);
        }
        if self.sequences.pose.sequence != pose {
            self.sequences.pose.sequence = pose;
            self.sequences.pose.reset_frame();
        }
        super::sequence::advance(&mut self.sequences.pose, assets, true);
        super::sequence::advance(&mut self.sequences.action, assets, false);
    }
    fn set_destination(&mut self, destination: ActorLocation) {
        self.destination.clear();
        self.destination.push_back(destination);
    }
    fn teleport(&mut self, destination: ActorLocation, view: &dyn Navigation) {
        if view.cell(destination).is_none() {
            self.status = ActorStatus::Parked;
            return;
        }
        self.location = destination;
        self.position = center(destination, self.size);
        self.destination.clear();
    }
    fn random(&mut self, bound: u32) -> u32 {
        loop {
            self.rng = self.rng.wrapping_add(0x9e3779b97f4a7c15);
            let mut z = self.rng;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
            z ^= z >> 31;
            let value = z as u32;
            let threshold = bound.wrapping_neg() % bound;
            if value >= threshold {
                return value % bound;
            }
        }
    }
    pub fn occupies(&self) -> bool {
        self.spawn.blocking != ActorBlocking::None
            && !matches!(
                self.status,
                ActorStatus::Parked | ActorStatus::MissingDefinition
            )
    }
}
fn center(location: ActorLocation, size: u16) -> LocalXZ {
    LocalXZ::new(
        LocalCoord::from_units(location.tile.x * 128 + i32::from(size) * 64),
        LocalCoord::from_units(location.tile.y * 128 + i32::from(size) * 64),
    )
}
fn orientation(dx: i32, dz: i32) -> u16 {
    match (dx.signum(), dz.signum()) {
        (-1, -1) => 256,
        (-1, 0) => 512,
        (-1, 1) => 768,
        (0, 1) => 1024,
        (1, 1) => 1280,
        (1, 0) => 1536,
        (1, -1) => 1792,
        _ => 0,
    }
}
