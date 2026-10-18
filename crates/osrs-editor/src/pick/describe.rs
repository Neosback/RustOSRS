//! Copyable text diagnostics for picked tiles and objects.

use super::*;
use std::fmt::Write as _;

pub(super) fn tile_name(info: &TileInfo) -> &'static str {
    match info.surface {
        SurfaceKind::None => "none",
        SurfaceKind::Flat => "flat paint",
        SurfaceKind::Shaped => "shaped model",
    }
}

pub(super) fn write_tile(out: &mut String, tile: (i32, i32), plane: usize, info: &TileInfo) {
    let _ = writeln!(
        out,
        "  tile ({}, {}) plane {plane}: surface={} faces={} skipped_faces={} underlay={} overlay_raw={} shape={} rotation={} settings={:#04x} (bridge={} vis_below={}) shadow={} min_plane={} linked_below={} heights[sw,se,ne,nw]={:?}",
        tile.0,
        tile.1,
        tile_name(info),
        info.faces,
        info.skipped_faces,
        info.underlay,
        info.overlay_raw,
        info.shape,
        info.rotation,
        info.settings,
        info.settings & 2 != 0,
        info.settings & 8 != 0,
        info.shadow,
        info.min_plane,
        info.has_linked_below,
        info.heights,
    );
}

pub(super) fn write_loc(out: &mut String, loc: &LocInfo) {
    let def = &loc.definition;
    let _ = writeln!(
        out,
        "  OBJECT {} name={:?} type={} orientation={} anchor=({}, {}) source_plane={} draw_level={} min_plane={}",
        def.identity.id.get(),
        def.name,
        loc.loc_type,
        loc.orientation,
        loc.tile.0,
        loc.tile.1,
        loc.plane,
        loc.level,
        loc.min_plane,
    );
    let _ = writeln!(out, "    plan: {:?}", loc.plan);
    for (index, slot) in loc.slots.iter().enumerate() {
        let _ = writeln!(
            out,
            "    slot {index}: {} origin={:?} rotation={} bounds={:?}..{:?} vertices={} faces={} biased={} (max {}) alpha={} textured={} hidden={}",
            slot.kind,
            slot.origin,
            slot.rotation,
            slot.min,
            slot.max,
            slot.vertices,
            slot.faces,
            slot.biased_faces,
            slot.max_bias,
            slot.alpha_faces,
            slot.textured_faces,
            slot.hidden_faces,
        );
    }
    let _ = writeln!(
        out,
        "    def: size={}x{} decoration_displacement={} is_rotated={} non_flat_shading={} contour_clip={:?} ambient={} contrast={} animation={:?} morphs={}",
        def.size_x,
        def.size_y,
        def.decoration_displacement,
        def.is_rotated,
        def.non_flat_shading,
        def.contour_clip,
        def.ambient,
        def.contrast,
        def.animation,
        def.morphs.is_some(),
    );
    let _ = writeln!(
        out,
        "    def: category={:?} map_scene={:?} map_icon={:?} blocking_mask={:#04x} support_items={:?} actions={:?} params={}",
        def.category.map(|c| c.get()),
        def.map_scene.map(|s| s.get()),
        def.map_icon.map(|i| i.get()),
        def.blocking_mask,
        def.support_items,
        def.actions,
        def.parameters.len(),
    );
    let _ = writeln!(
        out,
        "    def: scale={:?} translation={:?} recolors={:?} retextures={:?} ground_raise={} full_recolor={:?} placement={:?}",
        def.scale,
        def.translation,
        def.recolors,
        def.retextures,
        def.ground_raise,
        def.full_recolor,
        def.placement,
    );
    let _ = writeln!(out, "    def: models={:?}", def.models);
}

/// Non-empty definition metadata rows for the object inspector grid.
pub fn object_extra_fields(
    def: &osrs_core::definitions::ObjectDefinition,
) -> Vec<(&'static str, String)> {
    let mut rows = Vec::new();
    let actions: Vec<&str> = def.actions.iter().filter_map(Option::as_deref).collect();
    if !actions.is_empty() {
        rows.push(("Actions", actions.join(", ")));
    }
    if let Some(cat) = def.category {
        rows.push(("Category", cat.get().to_string()));
    }
    if let Some(scene) = def.map_scene {
        rows.push(("Map scene", scene.get().to_string()));
    }
    if let Some(icon) = def.map_icon {
        rows.push(("Map icon", icon.get().to_string()));
    }
    if def.blocking_mask != 0 {
        let mut blocked = Vec::new();
        for (bit, dir) in [(1, "N"), (2, "E"), (4, "S"), (8, "W")] {
            if def.blocking_mask & bit != 0 {
                blocked.push(dir);
            }
        }
        rows.push((
            "Approach mask",
            format!("{:#04x} (blocks {})", def.blocking_mask, blocked.join("/")),
        ));
    }
    if let Some(support) = def.support_items.filter(|&s| s != 0) {
        rows.push(("Supports items", support.to_string()));
    }
    if let Some(morphs) = &def.morphs {
        let count = morphs.transforms.iter().flatten().count();
        rows.push(("Morphs", format!("{count} target(s)")));
    }
    if !def.parameters.is_empty() {
        rows.push(("Parameters", format!("{} entry(s)", def.parameters.len())));
    }
    rows
}

pub(super) fn info_for(infos: &Infos, tile: (i32, i32)) -> Option<&Arc<RegionInfo>> {
    infos.get(&(tile.0.div_euclid(64), tile.1.div_euclid(64)))
}

/// Text for one object (`Copy object`).
pub fn describe_object(infos: &Infos, pick: &ObjectPick) -> String {
    let mut out = String::new();
    if let Some(loc) = infos
        .get(&pick.region)
        .and_then(|info| info.locs.get(pick.index))
    {
        let _ = writeln!(out, "=== object ===");
        write_loc(&mut out, loc);
        let _ = writeln!(out, "=== tile under the anchor ===");
        for plane in 0..4 {
            if let Some(info) =
                info_for(infos, loc.tile).and_then(|region| region.tile(plane, loc.tile))
            {
                write_tile(&mut out, loc.tile, plane, info);
            }
        }
    }
    out
}

/// Text for one tile: terrain on every plane and every object anchored on it.
pub fn describe_tile(infos: &Infos, tile: (i32, i32)) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "=== tile ({}, {}) ===", tile.0, tile.1);
    if let Some(region) = info_for(infos, tile) {
        for plane in 0..4 {
            if let Some(info) = region.tile(plane, tile) {
                write_tile(&mut out, tile, plane, info);
            }
        }
        for loc in region.locs_at(tile) {
            write_loc(&mut out, loc);
        }
    }
    out
}

/// Text for the tile and everything within `radius` tiles (objects whose footprint reaches the
/// area are included): the data needed to reason about normal merging and wall joins.
pub fn describe_neighbourhood(
    infos: &Infos,
    center: (i32, i32),
    plane: usize,
    radius: i32,
) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "=== neighbourhood of ({}, {}) radius {radius}, terrain plane {plane} ===",
        center.0, center.1
    );
    for dy in (-radius..=radius).rev() {
        for dx in -radius..=radius {
            let tile = (center.0 + dx, center.1 + dy);
            if let Some(info) = info_for(infos, tile).and_then(|region| region.tile(plane, tile)) {
                write_tile(&mut out, tile, plane, info);
            }
        }
    }
    let _ = writeln!(
        out,
        "--- objects (anchored within radius + 3, footprint overlapping) ---"
    );
    let reach = radius + 3;
    for dy in -reach..=reach {
        for dx in -reach..=reach {
            let anchor = (center.0 + dx, center.1 + dy);
            let Some(region) = info_for(infos, anchor) else {
                continue;
            };
            for loc in region.locs_at(anchor) {
                let footprint = loc.plan.rotated_definition_footprint;
                let (width, depth) = (i32::from(footprint.width), i32::from(footprint.depth));
                let overlaps = loc.tile.0 + width > center.0 - radius
                    && loc.tile.0 <= center.0 + radius
                    && loc.tile.1 + depth > center.1 - radius
                    && loc.tile.1 <= center.1 + radius;
                if overlaps {
                    write_loc(&mut out, loc);
                }
            }
        }
    }
    out
}

/// One-line summary for the hover readout.
pub fn summary(infos: &Infos, pick: &Pick) -> String {
    let mut parts = Vec::new();
    if let Some(object) = &pick.object
        && let Some(loc) = infos
            .get(&object.region)
            .and_then(|info| info.locs.get(object.index))
    {
        parts.push(format!(
            "object {} type {} rot {} at ({}, {}) plane {}",
            loc.definition.identity.id.get(),
            loc.loc_type,
            loc.orientation,
            loc.tile.0,
            loc.tile.1,
            loc.plane,
        ));
    }
    if let Some(tile) = &pick.tile {
        parts.push(format!("tile ({}, {})", tile.tile.0, tile.tile.1));
    }
    parts.join("  |  ")
}

/// The brush-relevant values of one tile on `plane` (`None` outside the resident regions).
pub fn tile_values(infos: &Infos, tile: (i32, i32), plane: u8) -> Option<crate::paint::TileValues> {
    let info = info_for(infos, tile)?.tile(usize::from(plane.min(3)), tile)?;
    Some(crate::paint::TileValues {
        underlay: info.underlay,
        overlay: info.overlay_raw,
        shape: info.shape,
        rotation: info.rotation,
        height: info.heights[0],
        flags: info.settings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use osrs_core::{
        definitions::{
            DefinitionIdentity, ModelScale, ModelTranslation, ObjectDefinition, ObjectMorphs,
            ObjectParameter, ObjectPlacementFlags,
        },
        ids::{CategoryId, MapIconId, MapSceneId, ObjectId, VarbitId},
        provenance::{CacheFingerprint, ProfileDigest, TargetProvenance},
    };
    use std::collections::BTreeMap;

    #[test]
    fn object_extra_fields_formats_approach_mask_morphs_and_params() {
        let mut params = BTreeMap::new();
        params.insert(10, ObjectParameter::Integer(42));
        let provenance = TargetProvenance::new(
            "osrs-live-241-2026-09-30-openrs2-2727",
            ProfileDigest::from_lower_hex(
                "cfdefa9ef99eff799fcef4fdf0ec78d9fdcd72d8e5be78e1c154d018ab4575b7",
            )
            .unwrap_or_else(|e| panic!("{e:?}")),
            CacheFingerprint::from_lower_hex(
                "ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38",
            )
            .unwrap_or_else(|e| panic!("{e:?}")),
            1,
        )
        .unwrap_or_else(|e| panic!("{e:?}"));
        let def = ObjectDefinition {
            identity: DefinitionIdentity::new(ObjectId::new(100), provenance),
            name: Some("Chest".to_owned()),
            models: None,
            size_x: 1,
            size_y: 1,
            placement: ObjectPlacementFlags {
                interact_type: 2,
                blocks_projectiles: true,
                obstructs_ground: false,
                clipped: true,
                solid: true,
                model_clipped: false,
            },
            decoration_displacement: 16,
            support_items: Some(1),
            is_rotated: false,
            non_flat_shading: false,
            contour_clip: None,
            full_recolor: None,
            ground_raise: 0,
            random_animation_start: false,
            animation: None,
            ambient: 0,
            contrast: 0,
            scale: ModelScale {
                x: 128,
                y: 128,
                z: 128,
            },
            translation: ModelTranslation { x: 0, y: 0, z: 0 },
            recolors: Vec::new(),
            retextures: Vec::new(),
            morphs: Some(ObjectMorphs {
                transform_varbit: Some(VarbitId::new(1)),
                transform_varp: None,
                transforms: vec![Some(ObjectId::new(101)), None],
                fallback: Some(ObjectId::new(102)),
            }),
            map_scene: Some(MapSceneId::new(12)),
            map_icon: Some(MapIconId::new(5)),
            category: Some(CategoryId::new(207)),
            actions: [
                Some("Open".to_owned()),
                Some("Search".to_owned()),
                None,
                None,
                None,
            ],
            interactive_hint: None,
            blocking_mask: 0x05,
            sound: None,
            parameters: params,
        };
        let rows = object_extra_fields(&def);
        assert_eq!(
            rows,
            vec![
                ("Actions", "Open, Search".to_owned()),
                ("Category", "207".to_owned()),
                ("Map scene", "12".to_owned()),
                ("Map icon", "5".to_owned()),
                ("Approach mask", "0x05 (blocks N/S)".to_owned()),
                ("Supports items", "1".to_owned()),
                ("Morphs", "1 target(s)".to_owned()),
                ("Parameters", "1 entry(s)".to_owned()),
            ]
        );
    }
}
