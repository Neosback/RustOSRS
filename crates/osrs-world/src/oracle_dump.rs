//! Text dump of a terrain scene in the exact line format of the pinned-client terrain oracle
//! (`tools/deob-harness/src/TerrainOracle.java`), used by differential tests and debugging.

use osrs_core::coords::{SceneTile, StoragePlane};
use osrs_scene::{SceneGrid, SemanticTile, terrain::TerrainSurface, terrain_load::TerrainLoadGrid};
use std::fmt::{Display, Write as _};

fn join<T: Display>(values: impl IntoIterator<Item = T>) -> String {
    let mut out = String::new();
    for (index, value) in values.into_iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        let _ = write!(out, "{value}");
    }
    out
}

/// One `tile`/`below` oracle line.
pub fn format_tile(tag: &str, plane: u8, x: u32, y: u32, tile: &SemanticTile) -> String {
    let mut line = format!(
        "{tag} {plane} {x} {y} plane={} orig={} min={} linked={}",
        tile.storage_plane().index().get(),
        tile.original_plane().index().get(),
        tile.min_plane(),
        u8::from(tile.linked_below().is_some())
    );
    match &tile.terrain {
        Some(TerrainSurface::Flat(flat)) => {
            let _ = write!(
                line,
                " paint={},{},{},{} tex={} rgb={} flat={}",
                flat.colors.southwest,
                flat.colors.southeast,
                flat.colors.northeast,
                flat.colors.northwest,
                flat.texture_id.unwrap_or(-1),
                flat.rgb,
                u8::from(flat.is_flat)
            );
        }
        Some(TerrainSurface::Shaped(model)) => {
            let textured = model.faces.iter().any(|face| face.texture_id.is_some());
            let _ = write!(
                line,
                " model shape={} rot={} flat={} under={} over={} vx={} vy={} vz={} ca={} cb={} cc={} fx={} fy={} fz={} ft={}",
                model.shape,
                model.rotation,
                u8::from(model.is_flat),
                model.underlay_rgb,
                model.overlay_rgb,
                join(model.vertices.iter().map(|v| v.position.x.units())),
                join(model.vertices.iter().map(|v| v.position.y.units())),
                join(model.vertices.iter().map(|v| v.position.z.units())),
                join(model.faces.iter().map(|f| f.colors[0])),
                join(model.faces.iter().map(|f| f.colors[1])),
                join(model.faces.iter().map(|f| f.colors[2])),
                join(model.faces.iter().map(|f| f.indices[0])),
                join(model.faces.iter().map(|f| f.indices[1])),
                join(model.faces.iter().map(|f| f.indices[2])),
                if textured {
                    join(model.faces.iter().map(|f| f.texture_id.unwrap_or(-1)))
                } else {
                    "-".to_owned()
                },
            );
        }
        None => {}
    }
    line
}

/// Everything after the oracle's two header lines: height rows, setting rows, then tiles in
/// plane/x/y order each followed by its linked-below tile.
pub fn terrain_body_lines(load: &TerrainLoadGrid, scene: &SceneGrid) -> Vec<String> {
    let (size_x, size_y) = (load.size_x(), load.size_y());
    let mut lines = Vec::new();
    for plane in 0..4 {
        for x in 0..=size_x {
            lines.push(format!(
                "h {plane} {x} {}",
                join((0..=size_y).map(|y| load.height(plane, x, y)))
            ));
        }
    }
    for plane in 0..4 {
        for x in 0..size_x {
            lines.push(format!(
                "s {plane} {x} {}",
                join((0..size_y).map(|y| i32::from(load.settings(plane, x, y) as i8)))
            ));
        }
    }
    for plane_index in 0..4_u8 {
        let Some(plane) = StoragePlane::new(plane_index) else {
            continue;
        };
        for x in 0..size_x as u32 {
            for y in 0..size_y as u32 {
                let Some(tile) = scene.tile(plane, SceneTile::new(x, y)) else {
                    continue;
                };
                lines.push(format_tile("tile", plane_index, x, y, tile));
                if let Some(below) = tile.linked_below() {
                    lines.push(format_tile("below", plane_index, x, y, below));
                }
            }
        }
    }
    lines
}
