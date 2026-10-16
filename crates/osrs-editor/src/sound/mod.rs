//! Area sounds of the loaded map: the objects that play a sound on their own (waterfalls, fires,
//! machinery) and where each can be heard. Pure data and arithmetic over the picker's region
//! infos; no audio and no semantic state. How the client treats them (from the RuneLite client's
//! `ObjectSound`): the sound is positioned on the object's footprint rectangle, plays only on the
//! listener's plane, is silent farther than `distance` tiles and at full volume within `retain - 1`
//! tiles, and an object with random sounds picks one at random every `change_ticks`.

use crate::pick::Infos;
use osrs_core::definitions::ObjectSound;

/// Sound effect ids RuneLite names (most area sounds have no known name).
const NAMES: &str = include_str!("names.txt");

/// What the panel lists: the camera's map square, or everything loaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Region((i32, i32)),
    Loaded,
}

/// One object that makes a sound.
#[derive(Debug, Clone, PartialEq)]
pub struct AreaSound {
    pub object: u32,
    pub name: String,
    /// World tile of the placement anchor and its source plane.
    pub tile: (i32, i32),
    pub plane: u8,
    /// Footprint in tiles after rotation.
    pub size: (u8, u8),
    pub region: (i32, i32),
    /// Index of the loc in its region (a stable handle while the region stays loaded).
    pub index: usize,
    pub sound: ObjectSound,
}

/// The id the row is listed under: the looping sound, else the first random one.
impl AreaSound {
    pub fn main_sound(&self) -> Option<u32> {
        self.sound.sounds().next().map(|id| id.get())
    }

    /// The footprint rectangle in tile coordinates: `(min, max)` corners.
    pub fn rect(&self) -> ((f32, f32), (f32, f32)) {
        let min = (self.tile.0 as f32, self.tile.1 as f32);
        (
            min,
            (
                min.0 + f32::from(self.size.0),
                min.1 + f32::from(self.size.1),
            ),
        )
    }

    /// Whether `needle` (lower case) occurs in the object's or a sound's name or id.
    pub fn matches(&self, needle: &str) -> bool {
        needle.is_empty()
            || self.name.to_lowercase().contains(needle)
            || self.object.to_string() == needle
            || self.sound.sounds().any(|id| {
                id.get().to_string() == needle || name(id.get()).is_some_and(|n| n.contains(needle))
            })
    }
}

/// RuneLite's name for a sound effect id, when it has one.
pub fn name(id: u32) -> Option<&'static str> {
    NAMES
        .lines()
        .filter(|l| !l.starts_with('#'))
        .find_map(|line| {
            let (number, name) = line.split_once(' ')?;
            (number.parse::<u32>().ok()? == id).then_some(name)
        })
}

/// Footprint of an object placed with `orientation` (quarter turns): odd turns swap the sides.
pub fn footprint(size_x: u16, size_y: u16, orientation: u8) -> (u8, u8) {
    let (x, y) = if orientation & 1 == 1 {
        (size_y, size_x)
    } else {
        (size_x, size_y)
    };
    (x.min(255) as u8, y.min(255) as u8)
}

/// The distance in tiles beyond a footprint at which the sound is full volume and silent.
/// The client measures from the footprint rectangle to the listener's position and takes off half
/// a tile before comparing, so each radius reaches half a tile further than its setting.
pub fn reach(sound: &ObjectSound) -> (f32, f32) {
    let inner = f32::from(sound.retain.saturating_sub(1)) + 0.5;
    let outer = f32::from(sound.distance) + 0.5;
    (inner.min(outer), outer)
}

/// Points, in tile coordinates, on the outline `distance` tiles outside a rectangle: straight
/// along the sides, quarter circles at the corners. Counter-clockwise, closed implicitly.
pub fn ring(min: (f32, f32), max: (f32, f32), distance: f32) -> Vec<(f32, f32)> {
    const ARC: usize = 8;
    let corners = [
        ((max.0, max.1), 0.0_f32),
        ((min.0, max.1), 90.0),
        ((min.0, min.1), 180.0),
        ((max.0, min.1), 270.0),
    ];
    let mut points = Vec::with_capacity(4 * (ARC + 1));
    for (corner, start) in corners {
        for step in 0..=ARC {
            let angle = (start + 90.0 * step as f32 / ARC as f32).to_radians();
            points.push((
                corner.0 + distance * angle.cos(),
                corner.1 + distance * angle.sin(),
            ));
        }
    }
    points
}

/// The objects in `scope` that play a sound, on `plane` when given, sorted by (sound, position).
/// The second value counts objects that change with a variable and have no sound of their own:
/// the sound of the object they turn into is not listed.
pub fn collect(infos: &Infos, scope: Scope, plane: Option<u8>) -> (Vec<AreaSound>, usize) {
    let mut sounds = Vec::new();
    let mut unresolved = 0;
    for (region, info) in infos {
        if let Scope::Region(only) = scope
            && only != *region
        {
            continue;
        }
        for (index, loc) in info.locs.iter().enumerate() {
            if plane.is_some_and(|plane| loc.plane != plane) {
                continue;
            }
            let definition = &loc.definition;
            let Some(sound) = &definition.sound else {
                unresolved += usize::from(definition.morphs.is_some());
                continue;
            };
            if sound.sounds().next().is_none() {
                continue;
            }
            sounds.push(AreaSound {
                object: definition.identity.id.get(),
                name: definition.name.clone().unwrap_or_default(),
                tile: loc.tile,
                plane: loc.plane,
                size: footprint(definition.size_x, definition.size_y, loc.orientation),
                region: *region,
                index,
                sound: sound.clone(),
            });
        }
    }
    sounds.sort_by_key(|s| (s.main_sound(), s.tile, s.plane, s.object));
    (sounds, unresolved)
}

/// The objects that play each sound, most used first (ties by sound ID). An object with several
/// sounds appears under each of them.
pub fn by_sound<'a>(sounds: &[&'a AreaSound]) -> Vec<(u32, Vec<&'a AreaSound>)> {
    let mut groups: std::collections::BTreeMap<u32, Vec<&'a AreaSound>> =
        std::collections::BTreeMap::new();
    for row in sounds {
        for id in row.sound.sounds() {
            groups.entry(id.get()).or_default().push(row);
        }
    }
    let mut groups: Vec<_> = groups.into_iter().collect();
    groups.sort_by_key(|(id, objects)| (std::cmp::Reverse(objects.len()), *id));
    groups
}

#[cfg(test)]
mod tests;
