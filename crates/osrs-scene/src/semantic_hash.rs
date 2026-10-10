//! Versioned canonical hashing for authoritative semantic scene state.
//!
//! The hash is intentionally renderer-independent. It walks the bounded scene in
//! plane/y/x storage order and encodes every currently owned semantic field with
//! explicit enum/option tags and fixed-width big-endian integers.

use crate::{
    placement::{ModelRequest, PlacementKind, PlacementPlan},
    scene::{SceneGameObject, SceneGrid, ScenePlacedLoc, SemanticTile},
    terrain::{TerrainColorSource, TerrainSurface},
};
use osrs_core::coords::{LocalPoint, SceneTile, StoragePlane};
use sha2::{Digest, Sha256};

/// Domain separator for the first canonical semantic-scene hash format.
pub const SEMANTIC_SCENE_HASH_V1: &str = "rustosrs-semantic-scene-v1";

/// Hash the complete currently owned semantic scene state.
///
/// This is a canonical verification identity, not a persistence format. Any
/// future semantic field that should participate in authoritative document
/// identity requires a deliberate hash-version change or an additive v1 update
/// before that field is considered covered by M9+ determinism guarantees.
pub fn semantic_scene_hash_v1(scene: &SceneGrid) -> [u8; 32] {
    let mut digest = Sha256::new();
    hash_bytes(&mut digest, SEMANTIC_SCENE_HASH_V1.as_bytes());
    hash_u32(&mut digest, scene.width());
    hash_u32(&mut digest, scene.height());
    hash_u8(&mut digest, scene.plane_count());

    for plane_index in 0..scene.plane_count() {
        hash_u8(&mut digest, plane_index);
        let Some(plane) = StoragePlane::new(plane_index) else {
            continue;
        };
        for y in 0..scene.height() {
            for x in 0..scene.width() {
                hash_u32(&mut digest, x);
                hash_u32(&mut digest, y);
                hash_optional_tile(&mut digest, scene.tile(plane, SceneTile::new(x, y)));
            }
        }
    }

    digest.finalize().into()
}

/// Lowercase hexadecimal form of [`semantic_scene_hash_v1`].
pub fn semantic_scene_hash_hex_v1(scene: &SceneGrid) -> String {
    bytes_to_hex(semantic_scene_hash_v1(scene))
}

fn hash_optional_tile(digest: &mut Sha256, tile: Option<&SemanticTile>) {
    match tile {
        Some(tile) => {
            hash_u8(digest, 1);
            hash_tile(digest, tile);
        }
        None => hash_u8(digest, 0),
    }
}

fn hash_tile(digest: &mut Sha256, tile: &SemanticTile) {
    match tile.source_plane() {
        Some(plane) => {
            hash_u8(digest, 1);
            hash_u8(digest, plane.index().get());
        }
        None => hash_u8(digest, 0),
    }
    hash_u8(digest, tile.storage_plane().index().get());

    match tile.terrain.as_ref() {
        Some(terrain) => {
            hash_u8(digest, 1);
            hash_terrain(digest, terrain);
        }
        None => hash_u8(digest, 0),
    }

    hash_optional_loc(digest, tile.floor_decoration());
    hash_optional_loc(digest, tile.boundary());
    hash_optional_loc(digest, tile.wall_decoration());

    hash_len(digest, tile.game_objects().len());
    for object in tile.game_objects() {
        hash_game_object(digest, object);
    }

    hash_optional_tile(digest, tile.linked_below());
}

fn hash_terrain(digest: &mut Sha256, terrain: &TerrainSurface) {
    match terrain {
        TerrainSurface::Flat(flat) => {
            hash_u8(digest, 0);
            hash_i32(digest, flat.heights.southwest);
            hash_i32(digest, flat.heights.southeast);
            hash_i32(digest, flat.heights.northeast);
            hash_i32(digest, flat.heights.northwest);
            hash_i32(digest, flat.colors.southwest);
            hash_i32(digest, flat.colors.southeast);
            hash_i32(digest, flat.colors.northeast);
            hash_i32(digest, flat.colors.northwest);
            hash_optional_i32(digest, flat.texture_id);
            hash_bool(digest, flat.is_flat);
        }
        TerrainSurface::Shaped(shaped) => {
            hash_u8(digest, 1);
            hash_u8(digest, shaped.shape);
            hash_u8(digest, shaped.rotation);
            hash_len(digest, shaped.vertices.len());
            for vertex in &shaped.vertices {
                hash_local_point(digest, vertex.position);
                hash_i32(digest, vertex.underlay_color);
                hash_i32(digest, vertex.overlay_color);
            }
            hash_len(digest, shaped.faces.len());
            for face in &shaped.faces {
                for index in face.indices {
                    hash_u64(digest, u64::try_from(index).unwrap_or(u64::MAX));
                }
                for color in face.colors {
                    hash_i32(digest, color);
                }
                hash_u8(
                    digest,
                    match face.color_source {
                        TerrainColorSource::Underlay => 0,
                        TerrainColorSource::Overlay => 1,
                    },
                );
                hash_optional_i32(digest, face.texture_id);
            }
            hash_bool(digest, shaped.is_flat);
            hash_i32(digest, shaped.underlay_rgb);
            hash_i32(digest, shaped.overlay_rgb);
        }
    }
}

fn hash_optional_loc(digest: &mut Sha256, loc: Option<&ScenePlacedLoc>) {
    match loc {
        Some(loc) => {
            hash_u8(digest, 1);
            hash_u32(digest, loc.object_id().get());
            hash_placement(digest, loc.placement());
        }
        None => hash_u8(digest, 0),
    }
}

fn hash_game_object(digest: &mut Sha256, object: &SceneGameObject) {
    match object.instance_id() {
        Some(instance_id) => {
            hash_u8(digest, 1);
            hash_u64(digest, instance_id);
        }
        None => hash_u8(digest, 0),
    }
    hash_u32(digest, object.object_id().get());
    hash_u8(digest, object.tag_type());
    hash_scene_tile(digest, object.start());
    hash_scene_tile(digest, object.end());
    hash_u8(digest, object.plane().index().get());
    hash_u8(digest, object.edge_mask());
    match object.placement() {
        Some(placement) => {
            hash_u8(digest, 1);
            hash_placement(digest, placement);
        }
        None => hash_u8(digest, 0),
    }
}

fn hash_placement(digest: &mut Sha256, placement: PlacementPlan) {
    hash_u8(digest, placement.source_loc_type.get());
    hash_u8(digest, placement.source_orientation);
    hash_u16(digest, placement.rotated_definition_footprint.width);
    hash_u16(digest, placement.rotated_definition_footprint.depth);
    hash_local_point(digest, placement.model_center);
    hash_local_point(digest, placement.storage_center);

    match placement.kind {
        PlacementKind::FloorDecoration(plan) => {
            hash_u8(digest, 0);
            hash_model_request(digest, plan.model);
        }
        PlacementKind::Boundary(plan) => {
            hash_u8(digest, 1);
            hash_model_request(digest, plan.primary);
            hash_optional_model_request(digest, plan.secondary);
            hash_u16(digest, plan.primary_flag);
            hash_u16(digest, plan.secondary_flag);
        }
        PlacementKind::WallDecoration(plan) => {
            hash_u8(digest, 2);
            hash_model_request(digest, plan.primary);
            hash_optional_model_request(digest, plan.secondary);
            hash_u16(digest, plan.orientation_flag);
            hash_u8(digest, plan.orientation_parameter);
            hash_i32(digest, plan.offset_x);
            hash_i32(digest, plan.offset_z);
        }
        PlacementKind::GameObject(plan) => {
            hash_u8(digest, 3);
            hash_model_request(digest, plan.model);
            hash_u16(digest, plan.storage_footprint.width);
            hash_u16(digest, plan.storage_footprint.depth);
            hash_u16(digest, plan.insertion_flag);
        }
    }
}

fn hash_optional_model_request(digest: &mut Sha256, request: Option<ModelRequest>) {
    match request {
        Some(request) => {
            hash_u8(digest, 1);
            hash_model_request(digest, request);
        }
        None => hash_u8(digest, 0),
    }
}

fn hash_model_request(digest: &mut Sha256, request: ModelRequest) {
    hash_u8(digest, request.loc_type.get());
    hash_u8(digest, request.orientation);
}

fn hash_local_point(digest: &mut Sha256, point: LocalPoint) {
    hash_i32(digest, point.x.units());
    hash_i32(digest, point.y.units());
    hash_i32(digest, point.z.units());
}

fn hash_scene_tile(digest: &mut Sha256, tile: SceneTile) {
    hash_u32(digest, tile.x);
    hash_u32(digest, tile.y);
}

fn hash_optional_i32(digest: &mut Sha256, value: Option<i32>) {
    match value {
        Some(value) => {
            hash_u8(digest, 1);
            hash_i32(digest, value);
        }
        None => hash_u8(digest, 0),
    }
}

fn hash_bool(digest: &mut Sha256, value: bool) {
    hash_u8(digest, u8::from(value));
}

fn hash_len(digest: &mut Sha256, value: usize) {
    hash_u64(digest, u64::try_from(value).unwrap_or(u64::MAX));
}

fn hash_bytes(digest: &mut Sha256, value: &[u8]) {
    hash_len(digest, value.len());
    digest.update(value);
}

fn hash_u8(digest: &mut Sha256, value: u8) {
    digest.update([value]);
}

fn hash_u16(digest: &mut Sha256, value: u16) {
    digest.update(value.to_be_bytes());
}

fn hash_u32(digest: &mut Sha256, value: u32) {
    digest.update(value.to_be_bytes());
}

fn hash_u64(digest: &mut Sha256, value: u64) {
    digest.update(value.to_be_bytes());
}

fn hash_i32(digest: &mut Sha256, value: i32) {
    digest.update(value.to_be_bytes());
}

fn bytes_to_hex(bytes: [u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in bytes {
        output.push(HEX[usize::from(byte >> 4)] as char);
        output.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        placement::{PlacementInput, plan_placement},
        terrain::{FlatTerrainSurface, TerrainCorners, TerrainSurface},
    };
    use osrs_core::{definitions::LocType, ids::ObjectId};

    fn plane_zero() -> Result<StoragePlane, Box<dyn std::error::Error>> {
        StoragePlane::new(0)
            .ok_or_else(|| std::io::Error::other("plane zero must be valid").into())
    }

    #[test]
    fn equal_semantic_scenes_hash_identically_and_mutation_changes_identity()
    -> Result<(), Box<dyn std::error::Error>> {
        let plane = plane_zero()?;
        let mut first = SceneGrid::new(2, 2, 1)?;
        first.set_terrain(
            plane,
            SceneTile::new(0, 0),
            TerrainSurface::Flat(FlatTerrainSurface::new(
                TerrainCorners::new(1, 2, 3, 4),
                TerrainCorners::new(10, 20, 30, 40),
                Some(7),
            )),
        )?;
        let second = first.clone();
        assert_eq!(semantic_scene_hash_v1(&first), semantic_scene_hash_v1(&second));

        let placement = plan_placement(PlacementInput {
            loc_type: LocType::new(22),
            orientation: 0,
            tile: SceneTile::new(1, 1),
            size_x: 1,
            size_y: 1,
            sampled_height: 25,
            existing_wall_displacement: None,
        })?;
        assert!(first.insert_placement(plane, ObjectId::new(9001), placement)?);
        assert_ne!(semantic_scene_hash_v1(&first), semantic_scene_hash_v1(&second));
        Ok(())
    }
}
