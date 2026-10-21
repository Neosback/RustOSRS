//! Versioned content identity for canonical semantic values; not a session freshness policy.
//!
//! Owns the map/patch SHA-256 preimages specified in `docs/specs/semantic-digests.md`.
//! No history, source, rendering, serialization format or mutable cache is retained here.
use crate::{MapDocument, MapOp, MapPatch, ObjectPlacement, TerrainTile};
use osrs_core::{
    coords::{MapTile, SourcePlane},
    provenance::Digest256,
};
use sha2::{Digest, Sha256};

/// Algorithm/domain identifier for [`MapDocument::content_digest_v1`].
pub const MAP_DOCUMENT_DIGEST_ALGORITHM_V1: &str = "rustosrs-map-document-v1";
/// Algorithm/domain identifier for [`MapPatch::content_digest_v1`].
pub const MAP_PATCH_DIGEST_ALGORITHM_V1: &str = "rustosrs-map-patch-v1";

impl MapDocument {
    /// SHA-256 of the versioned canonical map content, independent of insertion order.
    ///
    /// Sparse default terrain is normalized by this document's existing constructors. Missing
    /// heights remain distinct from explicit zero. Signed coordinates and all u32 IDs are lossless.
    /// Source/profile/build, available regions, revisions, appearance and scenarios are excluded;
    /// a reproduction manifest must identify them separately. This does not authorize a stale job.
    /// Explicit diagnostic operation: enumerates the map and sorts its exact object occurrences.
    pub fn content_digest_v1(&self) -> Digest256 {
        let mut hash = begin(MAP_DOCUMENT_DIGEST_ALGORITHM_V1);
        count(&mut hash, self.terrain_entry_count());
        for (coord, tile) in self.terrain_entries() {
            position(&mut hash, coord.tile, coord.plane);
            terrain(&mut hash, tile);
        }
        count(&mut hash, self.height_vertex_count());
        for (coord, value) in self.height_entries() {
            position(&mut hash, coord.corner, coord.plane);
            hash.update(value.to_le_bytes());
        }
        count(&mut hash, self.object_count());
        for placement in self.object_entries() {
            object(&mut hash, placement);
        }
        Digest256::from_bytes(hash.finalize().into())
    }
}

impl MapPatch {
    /// SHA-256 of this exact ordered transaction, including before-state and no-op operations.
    ///
    /// Uses a domain distinct from the map digest. Does not apply, normalize, reorder or validate
    /// operations; equal resulting maps need not imply equal transaction hashes. Impact is already
    /// derived from operations and is not encoded. Feature/intent/source identity belongs in the
    /// reproduction manifest, never in the semantic patch or this digest.
    pub fn content_digest_v1(&self) -> Digest256 {
        let mut hash = begin(MAP_PATCH_DIGEST_ALGORITHM_V1);
        count(&mut hash, self.len());
        for op in self.ops() {
            match *op {
                MapOp::SetTerrain {
                    coord,
                    before,
                    after,
                } => {
                    hash.update([0]);
                    position(&mut hash, coord.tile, coord.plane);
                    terrain(&mut hash, before);
                    terrain(&mut hash, after);
                }
                MapOp::SetHeight {
                    coord,
                    before,
                    after,
                } => {
                    hash.update([1]);
                    position(&mut hash, coord.corner, coord.plane);
                    hash.update(before.to_le_bytes());
                    hash.update(after.to_le_bytes());
                }
                MapOp::AddObject { placement } => {
                    hash.update([2]);
                    object(&mut hash, placement);
                }
                MapOp::RemoveObject { placement } => {
                    hash.update([3]);
                    object(&mut hash, placement);
                }
            }
        }
        Digest256::from_bytes(hash.finalize().into())
    }
}

fn begin(domain: &str) -> Sha256 {
    let mut hash = Sha256::new();
    hash.update(domain.as_bytes());
    hash.update([0]);
    hash
}
fn count(hash: &mut Sha256, value: usize) {
    hash.update((value as u64).to_le_bytes());
}
fn position(hash: &mut Sha256, tile: MapTile, plane: SourcePlane) {
    hash.update(tile.x.to_le_bytes());
    hash.update(tile.y.to_le_bytes());
    hash.update([plane.index().get()]);
}
fn terrain(hash: &mut Sha256, tile: TerrainTile) {
    match tile.underlay {
        None => hash.update([0]),
        Some(id) => {
            hash.update([1]);
            hash.update(id.get().to_le_bytes());
        }
    }
    match tile.overlay {
        None => hash.update([0]),
        Some(overlay) => {
            hash.update([1]);
            hash.update(overlay.id.get().to_le_bytes());
            hash.update([overlay.shape.get(), overlay.rotation.get()]);
        }
    }
    hash.update([tile.flags]);
}
fn object(hash: &mut Sha256, placement: ObjectPlacement) {
    position(hash, placement.position.tile, placement.position.plane);
    hash.update([placement.kind.get()]);
    hash.update(placement.object_id.get().to_le_bytes());
    hash.update([placement.orientation.get()]);
}

#[cfg(test)]
mod tests;
