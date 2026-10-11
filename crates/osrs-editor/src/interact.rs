//! What the game would offer for the thing under the cursor: the in-game menu entries
//! (`Open Bank booth`, `Examine Bank booth`, `Walk here`, `Cancel`), the one-line tooltip RuneLite's
//! mouse tooltips show, and the hovered tile's outline.
//!
//! This is the editor's stand-in for the client's menu. The entry list is built from the same
//! data the client uses (object name and the five menu actions), so a future client mode can
//! dispatch the same [`Effect`]s to a server.

use crate::pick::{Infos, ObjectPick, Pick};
use eframe::egui::{self, Color32};
use osrs_core::coords::RegionCoord;

/// The colour the game gives an object's name in menus.
pub const OBJECT_TARGET: Color32 = Color32::from_rgb(0, 255, 255);

/// What choosing an entry does.
#[derive(Debug, Clone)]
pub enum Effect {
    /// One of the object's five actions (0-based slot).
    ObjectAction {
        object: ObjectPick,
        slot: usize,
    },
    Examine(ObjectPick),
    WalkHere((i32, i32)),
    Cancel,
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub action: String,
    /// The coloured part (`Bank booth`).
    pub target: Option<String>,
    pub effect: Effect,
}

/// The entries the client would list for `pick`, top (default action) first. `Cancel` is not
/// included; the menu adds it.
pub fn entries(infos: &Infos, pick: &Pick) -> Vec<Entry> {
    let mut entries = Vec::new();
    if let Some(object) = &pick.object
        && let Some(loc) = infos
            .get(&object.region)
            .and_then(|info| info.locs.get(object.index))
        && let Some(name) = loc
            .definition
            .name
            .as_deref()
            .filter(|name| !name.is_empty() && *name != "null")
    {
        for (slot, action) in loc.definition.actions.iter().enumerate() {
            if let Some(action) = action.as_deref().filter(|action| !action.is_empty()) {
                entries.push(Entry {
                    action: action.to_owned(),
                    target: Some(name.to_owned()),
                    effect: Effect::ObjectAction {
                        object: object.clone(),
                        slot,
                    },
                });
            }
        }
        entries.push(Entry {
            action: "Examine".to_owned(),
            target: Some(name.to_owned()),
            effect: Effect::Examine(object.clone()),
        });
    }
    if let Some(tile) = &pick.tile {
        entries.push(Entry {
            action: "Walk here".to_owned(),
            target: None,
            effect: Effect::WalkHere(tile.tile),
        });
    }
    entries
}

/// A menu line as a coloured layout: action in white, target in the object colour.
pub fn entry_job(entry: &Entry, size: f32, extra: Option<&str>) -> egui::text::LayoutJob {
    let font = egui::FontId::proportional(size);
    let mut job = egui::text::LayoutJob::default();
    let mut add = |text: &str, color: Color32| {
        job.append(
            text,
            0.0,
            egui::TextFormat {
                font_id: font.clone(),
                color,
                ..Default::default()
            },
        );
    };
    add(&entry.action, Color32::WHITE);
    if let Some(target) = &entry.target {
        add(" ", Color32::WHITE);
        add(target, OBJECT_TARGET);
    }
    if let Some(extra) = extra {
        add(extra, Color32::WHITE);
    }
    job
}

/// `"/ 3 more options"` as the client appends it to the top entry.
pub fn more_options(count: usize) -> Option<String> {
    match count {
        0 | 1 => None,
        2 => Some(" / 1 more option".to_owned()),
        n => Some(format!(" / {} more options", n - 1)),
    }
}

/// The hovered tile's corner positions `[sw, se, ne, nw]` as world points `[x, height, z]`.
pub fn tile_corners(infos: &Infos, tile: (i32, i32), plane: u8) -> Option<[[f64; 3]; 4]> {
    let region = RegionCoord::new(tile.0.div_euclid(64), tile.1.div_euclid(64));
    let info = infos.get(&(region.x, region.y))?;
    let heights = info.tile(usize::from(plane), tile)?.heights;
    let (x0, z0) = (f64::from(tile.0) * 128.0, f64::from(tile.1) * 128.0);
    let (x1, z1) = (x0 + 128.0, z0 + 128.0);
    // Lift slightly so the outline does not z-fight with the ground.
    let lift = -3.0;
    Some([
        [x0, f64::from(heights[0]) + lift, z0],
        [x1, f64::from(heights[1]) + lift, z0],
        [x1, f64::from(heights[2]) + lift, z1],
        [x0, f64::from(heights[3]) + lift, z1],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn more_options_follow_the_client_wording() {
        assert_eq!(more_options(1), None);
        assert_eq!(more_options(2).as_deref(), Some(" / 1 more option"));
        assert_eq!(more_options(4).as_deref(), Some(" / 3 more options"));
    }
}
