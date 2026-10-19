//! A path lesson: one rectangle of real map (ground, heights, objects) with the paths that are in
//! it. It holds everything needed to hide the paths (a blank canvas) and to check what a learner
//! draws against the real thing, and nothing else: it is a self-contained file.

use crate::Snapshot;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const LESSON_FORMAT: &str = "rustosrs-path-lesson";
pub const LESSON_VERSION: u32 = 1;
/// Largest side of a lesson, in tiles.
pub const MAX_SIDE: u16 = 128;

/// What the lesson needs to know about an object definition.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectEntry {
    /// Lowercase cache name (empty when none).
    pub name: String,
    #[serde(default)]
    pub symbol: String,
    #[serde(default)]
    pub actions: Vec<String>,
    pub size: [u8; 2],
    /// Whether it blocks walking, when the cache says; otherwise the learner guesses by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocks: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PathLesson {
    pub format: String,
    pub version: u32,
    pub name: String,
    /// Which part of the world this was taken from: the south-west tile.
    pub origin: [i32; 2],
    /// Lessons of one group are never split between training and testing.
    #[serde(default)]
    pub group: String,
    pub width: u16,
    pub height: u16,
    /// Per tile, row by row from the south: `index = y * width + x`.
    pub overlay: Vec<u16>,
    pub shape: Vec<u8>,
    /// Overlay rotation per tile; empty when unknown (a lesson used only for learning).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rotation: Vec<u8>,
    /// Floor underlay per tile; empty when unknown.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub underlay: Vec<u16>,
    pub flags: Vec<u8>,
    /// The height of each tile's south-west corner, in client units.
    pub heights: Vec<i32>,
    /// `[object id, kind, orientation, x, y]` on the ground plane.
    pub objects: Vec<[u32; 5]>,
    pub names: BTreeMap<u32, ObjectEntry>,
    /// The floor overlays that are paths. Empty means "work it out" (see [`Self::guess_paths`]).
    #[serde(default)]
    pub path_overlays: Vec<u16>,
}

/// An overlay that looks like a path, and how sure that is.
#[derive(Debug, Clone, PartialEq)]
pub struct PathGuess {
    pub overlay: u16,
    pub tiles: u32,
    pub pieces: u32,
    /// Typical width, in tiles.
    pub width: f32,
}

impl PathLesson {
    pub fn validate(&self) -> Result<(), String> {
        if self.format != LESSON_FORMAT || self.version != LESSON_VERSION {
            return Err("not a path lesson (or from a newer editor)".into());
        }
        if self.width == 0 || self.height == 0 || self.width > MAX_SIDE || self.height > MAX_SIDE {
            return Err(format!(
                "a lesson is between 1 and {MAX_SIDE} tiles on a side"
            ));
        }
        let tiles = usize::from(self.width) * usize::from(self.height);
        if [
            self.overlay.len(),
            self.shape.len(),
            self.flags.len(),
            self.heights.len(),
        ]
        .iter()
        .any(|n| *n != tiles)
        {
            return Err("the lesson's ground does not match its size".into());
        }
        if self
            .objects
            .iter()
            .any(|o| o[3] >= u32::from(self.width) || o[4] >= u32::from(self.height))
        {
            return Err("an object lies outside the lesson".into());
        }
        Ok(())
    }

    pub fn from_json(text: &str) -> Result<Self, String> {
        let lesson: Self = serde_json::from_str(text).map_err(|e| e.to_string())?;
        lesson.validate()?;
        Ok(lesson)
    }

    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string(self).map_err(|e| e.to_string())
    }

    pub fn tiles(&self) -> usize {
        usize::from(self.width) * usize::from(self.height)
    }

    pub fn index(&self, x: usize, y: usize) -> usize {
        y * usize::from(self.width) + x
    }

    /// Take the ground plane of a snapshot (tiles, corner heights and objects of a `width` by
    /// `height` area) as a lesson.
    pub fn from_snapshot(
        name: &str,
        origin: [i32; 2],
        width: u16,
        height: u16,
        snapshot: &Snapshot,
        names: BTreeMap<u32, ObjectEntry>,
    ) -> Result<Self, String> {
        let tiles = usize::from(width) * usize::from(height);
        let mut lesson = Self {
            format: LESSON_FORMAT.to_owned(),
            version: LESSON_VERSION,
            name: name.to_owned(),
            origin,
            group: name.to_owned(),
            width,
            height,
            overlay: vec![0; tiles],
            shape: vec![0; tiles],
            rotation: vec![0; tiles],
            underlay: vec![0; tiles],
            flags: vec![0; tiles],
            heights: vec![0; tiles],
            objects: Vec::new(),
            names,
            path_overlays: Vec::new(),
        };
        for tile in snapshot.tiles.iter().filter(|t| t.plane == 0) {
            if tile.x < width && tile.y < height {
                let at = lesson.index(usize::from(tile.x), usize::from(tile.y));
                lesson.overlay[at] = tile.overlay.as_ref().map_or(0, |o| o.id as u16);
                lesson.shape[at] = tile.overlay.as_ref().map_or(0, |o| o.shape);
                lesson.rotation[at] = tile.overlay.as_ref().map_or(0, |o| o.rotation);
                lesson.underlay[at] = tile.underlay.map_or(0, |u| u as u16);
                lesson.flags[at] = tile.flags;
            }
        }
        for corner in snapshot.heights.iter().filter(|h| h.plane == 0) {
            if corner.x < width && corner.y < height {
                let at = lesson.index(usize::from(corner.x), usize::from(corner.y));
                lesson.heights[at] = corner.value;
            }
        }
        for object in snapshot.objects.iter().filter(|o| o.plane == 0) {
            lesson.objects.push([
                object.id,
                u32::from(object.kind),
                u32::from(object.orientation),
                u32::from(object.x),
                u32::from(object.y),
            ]);
        }
        lesson.validate()?;
        Ok(lesson)
    }

    /// The overlays that are paths here: the chosen ones, or, when none were chosen, the ones
    /// that look like paths ([`Self::guess_paths`]) with at least `min_tiles` tiles.
    pub fn path_overlay_set(&self, inside: &BTreeSet<usize>) -> BTreeSet<u16> {
        if !self.path_overlays.is_empty() {
            return self.path_overlays.iter().copied().collect();
        }
        self.guess_paths(inside)
            .into_iter()
            .filter(|g| g.tiles >= 12)
            .map(|g| g.overlay)
            .collect()
    }

    /// The path tiles (`true`): ground with a path overlay that is not inside a building.
    /// `inside` is the set of tile indices inside buildings.
    pub fn path_mask(&self, inside: &BTreeSet<usize>) -> Vec<bool> {
        let set = self.path_overlay_set(inside);
        (0..self.tiles())
            .map(|at| {
                self.overlay[at] != 0 && set.contains(&self.overlay[at]) && !inside.contains(&at)
            })
            .collect()
    }

    /// Which overlays look like paths: long and thin (not a blob or a lake), mostly outside
    /// buildings and not blocked terrain. Each overlay is taken whole: most of its tiles must lie
    /// in thin pieces.
    pub fn guess_paths(&self, inside: &BTreeSet<usize>) -> Vec<PathGuess> {
        let (w, h) = (usize::from(self.width), usize::from(self.height));
        let mut seen = vec![false; w * h];
        // overlay -> (tiles, thin tiles, pieces, width sum)
        let mut stats: BTreeMap<u16, (u32, u32, u32, f32)> = BTreeMap::new();
        for start in 0..w * h {
            let id = self.overlay[start];
            if id == 0 || seen[start] || inside.contains(&start) {
                continue;
            }
            let mut piece = vec![start];
            let mut queue = VecDeque::from([start]);
            seen[start] = true;
            while let Some(at) = queue.pop_front() {
                let (x, y) = (at % w, at / w);
                for (nx, ny) in [
                    (x.wrapping_sub(1), y),
                    (x + 1, y),
                    (x, y.wrapping_sub(1)),
                    (x, y + 1),
                ] {
                    if nx < w && ny < h {
                        let next = ny * w + nx;
                        if !seen[next] && self.overlay[next] == id && !inside.contains(&next) {
                            seen[next] = true;
                            piece.push(next);
                            queue.push_back(next);
                        }
                    }
                }
            }
            let entry = stats.entry(id).or_default();
            entry.0 += piece.len() as u32;
            if let Some(width) = self.thin_width(&piece) {
                entry.1 += piece.len() as u32;
                entry.2 += 1;
                entry.3 += width * piece.len() as f32;
            }
        }
        stats
            .into_iter()
            .filter(|(_, (tiles, thin, ..))| *thin * 10 >= *tiles * 6 && *tiles >= 6)
            .map(|(overlay, (tiles, thin, pieces, width_sum))| PathGuess {
                overlay,
                tiles,
                pieces,
                width: width_sum / thin.max(1) as f32,
            })
            .collect()
    }

    /// The width of a piece when it is path-like: at least 8 tiles, mostly walkable, thin (width
    /// 10 or less) and long (area at least 3 times width squared).
    fn thin_width(&self, piece: &[usize]) -> Option<f32> {
        let (w, h) = (usize::from(self.width), usize::from(self.height));
        if piece.len() < 8 {
            return None;
        }
        if piece.iter().filter(|at| self.flags[**at] & 1 != 0).count() * 10 > piece.len() * 3 {
            return None;
        }
        let members: BTreeSet<usize> = piece.iter().copied().collect();
        // Distance (in steps) to the nearest tile that is not part of the piece.
        let mut depth: BTreeMap<usize, u32> = BTreeMap::new();
        let mut queue = VecDeque::new();
        for &at in piece {
            let (x, y) = (at % w, at / w);
            let edge = [
                (x.wrapping_sub(1), y),
                (x + 1, y),
                (x, y.wrapping_sub(1)),
                (x, y + 1),
            ]
            .iter()
            .any(|&(nx, ny)| nx >= w || ny >= h || !members.contains(&(ny * w + nx)));
            if edge {
                depth.insert(at, 1);
                queue.push_back(at);
            }
        }
        while let Some(at) = queue.pop_front() {
            let d = depth[&at];
            let (x, y) = (at % w, at / w);
            for (nx, ny) in [
                (x.wrapping_sub(1), y),
                (x + 1, y),
                (x, y.wrapping_sub(1)),
                (x, y + 1),
            ] {
                if nx < w && ny < h {
                    let next = ny * w + nx;
                    if members.contains(&next) && !depth.contains_key(&next) {
                        depth.insert(next, d + 1);
                        queue.push_back(next);
                    }
                }
            }
        }
        let max = depth.values().copied().max().unwrap_or(1);
        let width = (2 * max - 1) as f32;
        (width <= 10.0 && piece.len() as f32 >= 3.0 * width * width).then_some(width)
    }
}
