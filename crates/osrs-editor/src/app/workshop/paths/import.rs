//! Taking an area of the map as a path lesson: the ground, the heights and the objects in it, and
//! which floor types are paths. Reads the loaded map; changes nothing.

use crate::app::EditorApp;
use osrs_core::coords::{MapTile, SourcePlane};
use osrs_edit::{HeightVertexCoord, MapPosition};
use osrs_lab::{PathLesson, PathObject};
use std::collections::BTreeMap;

pub(super) const SIZES: [u16; 5] = [32, 48, 64, 96, 128];

/// The area of `size` tiles on a side centred on the camera.
pub(super) fn around_camera(app: &EditorApp, size: u16, name: &str) -> Result<PathLesson, String> {
    let (cx, cy) = app.camera_tile();
    let half = i32::from(size) / 2;
    lesson_of(app, cx - half, cy - half, size, size, name)
}

/// The smallest area that holds every selected tile (up to 128 by 128).
pub(super) fn around_selection(app: &EditorApp, name: &str) -> Result<PathLesson, String> {
    let tiles = app.interaction.selection_tiles();
    let (Some(min_x), Some(max_x), Some(min_y), Some(max_y)) = (
        tiles.iter().map(|t| t.0).min(),
        tiles.iter().map(|t| t.0).max(),
        tiles.iter().map(|t| t.1).min(),
        tiles.iter().map(|t| t.1).max(),
    ) else {
        return Err("Select some tiles first (Select tool, tile mode).".into());
    };
    let (w, h) = (max_x - min_x + 1, max_y - min_y + 1);
    if w > 128 || h > 128 {
        return Err("The selection is bigger than 128 by 128 tiles. Select a smaller area.".into());
    }
    if w < 8 || h < 8 {
        return Err("The selection is smaller than 8 by 8 tiles. Select a bigger area.".into());
    }
    lesson_of(app, min_x, min_y, w as u16, h as u16, name)
}

pub(super) fn lesson_of(
    app: &EditorApp,
    x0: i32,
    y0: i32,
    width: u16,
    height: u16,
    name: &str,
) -> Result<PathLesson, String> {
    let document = app.session.document();
    let plane = SourcePlane::new(app.interaction.view_plane()).ok_or("not a floor of the map")?;
    let tiles = usize::from(width) * usize::from(height);
    let mut lesson = PathLesson {
        format: osrs_lab::LESSON_FORMAT.to_owned(),
        version: 1,
        name: name.to_owned(),
        origin: [x0, y0],
        group: format!("g{}_{}", x0.div_euclid(512), y0.div_euclid(512)),
        width,
        height,
        overlay: vec![0; tiles],
        shape: vec![0; tiles],
        rotation: vec![0; tiles],
        underlay: vec![0; tiles],
        flags: vec![0; tiles],
        heights: vec![0; tiles],
        objects: Vec::new(),
        names: BTreeMap::new(),
        places: Vec::new(),
        path_overlays: Vec::new(),
    };
    for y in 0..height {
        for x in 0..width {
            let tile = MapTile::new(x0 + i32::from(x), y0 + i32::from(y));
            let terrain = document.terrain_at(MapPosition::new(tile, plane));
            let at = usize::from(y) * usize::from(width) + usize::from(x);
            lesson.overlay[at] = terrain.overlay.map_or(0, |o| o.id.get() as u16);
            lesson.shape[at] = terrain.overlay.map_or(0, |o| o.shape.get());
            lesson.rotation[at] = terrain.overlay.map_or(0, |o| o.rotation.get());
            lesson.underlay[at] = terrain.underlay.map_or(0, |u| u.get() as u16);
            lesson.flags[at] = terrain.flags;
            lesson.heights[at] = document
                .height_at(HeightVertexCoord::new(tile, plane))
                .unwrap_or(0);
        }
    }
    let book = app.names.custom().book();
    for placed in document.object_entries() {
        let (px, py) = (placed.position.tile.x - x0, placed.position.tile.y - y0);
        if placed.position.plane != plane
            || px < 0
            || py < 0
            || px >= i32::from(width)
            || py >= i32::from(height)
        {
            continue;
        }
        let id = placed.object_id.get();
        lesson.objects.push([
            id,
            u32::from(placed.kind.get()),
            u32::from(placed.orientation.get()),
            px as u32,
            py as u32,
        ]);
        lesson.names.entry(id).or_insert_with(|| {
            let entry = app.placer.catalog_entry(id);
            let cache_name = entry.map(|e| e.name.clone()).unwrap_or_default();
            let symbol = app
                .names
                .book()
                .and_then(|b| b.object(id))
                .unwrap_or_default()
                .to_owned();
            let (name, symbol) = osrs_edit::census_name(book, id, &cache_name, &symbol);
            PathObject {
                name,
                symbol,
                actions: Vec::new(),
                size: entry.map_or([1, 1], |e| {
                    [e.size_x.min(255) as u8, e.size_y.min(255) as u8]
                }),
                blocks: None,
            }
        });
    }
    lesson.validate()?;
    Ok(lesson)
}

/// A line saying why a lesson may not teach much, or nothing when it looks fine.
pub(super) fn caveat(lesson: &PathLesson, path_tiles: usize) -> Option<String> {
    if path_tiles < 30 {
        return Some("It has almost no path in it. Pick an area with roads or trails.".into());
    }
    if lesson.objects.is_empty() {
        return Some("It has no objects (buildings, trees). The AI learns most from places with buildings and doors.".into());
    }
    None
}
