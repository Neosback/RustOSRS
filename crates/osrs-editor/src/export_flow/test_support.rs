//! Deterministic map/cache fixtures; no downloaded cache or GPU is required.
use super::*;
use osrs_cache::decode::{DecodedLocations, DecodedTerrain, EncodedTerrainTile, EncodedTileHeight};
use osrs_cache::encode::{
    CacheStore, ContainerCompression, GroupEntry, ReferenceTable, crc32, encode_container,
    encode_locations, encode_terrain, pack_group,
};
use osrs_cache::profile::TargetProfile;
use osrs_cache::transport::CacheProbe;
use osrs_core::coords::{MapTile, RegionTile, SourcePlane};
use osrs_edit::{MapOp, MapPatch};
use osrs_world::SourceSquare;
use std::sync::atomic::{AtomicU64, Ordering};

pub fn temp(name: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "rustosrs-workflow-{}-{}-{name}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&path).unwrap_or_else(|e| unreachable!("{e}"));
    path
}

pub struct Fixture {
    pub dir: PathBuf,
    pub profile: TargetProfile,
    pub regions: Vec<RegionCoord>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

pub fn plane(p: u8) -> SourcePlane {
    SourcePlane::new(p).unwrap_or_else(|| unreachable!())
}
pub fn position(x: i32, y: i32, p: u8) -> MapPosition {
    MapPosition::new(MapTile::new(x, y), plane(p))
}

pub fn square(region: RegionCoord) -> SourceSquare {
    let tiles = (0..4 * 64 * 64)
        .map(|i| EncodedTerrainTile {
            tile: RegionTile::new(((i / 64) % 64) as u8, (i % 64) as u8)
                .unwrap_or_else(|| unreachable!()),
            source_plane: plane((i / 4096) as u8),
            height: EncodedTileHeight::Explicit(10),
            overlay: None,
            settings: 0,
            underlay_id: 0,
        })
        .collect();
    let terrain = DecodedTerrain::from_parts(region, tiles, 0);
    let locations = DecodedLocations::with_locations(region, Vec::new());
    let files = vec![
        (
            0,
            encode_terrain(&terrain).unwrap_or_else(|e| unreachable!("{e}")),
        ),
        (
            1,
            encode_locations(&locations).unwrap_or_else(|e| unreachable!("{e}")),
        ),
        (2, vec![7, 8, 9]),
        (3, vec![12; 11]),
        (4, vec![]),
    ];
    SourceSquare {
        region,
        group_id: (region.x as u32) << 8 | region.y as u32,
        terrain,
        locations,
        files,
    }
}

pub fn fixture() -> Fixture {
    let dir = temp("source");
    let store = CacheStore::create(&dir).unwrap_or_else(|e| unreachable!("{e}"));
    let regions = vec![
        RegionCoord::new(50, 50),
        RegionCoord::new(51, 50),
        RegionCoord::new(50, 51),
        RegionCoord::new(51, 51),
    ];
    let mut groups = Vec::new();
    for region in &regions {
        let square = square(*region);
        let parts: Vec<&[u8]> = square.files.iter().map(|(_, b)| b.as_slice()).collect();
        let packed = pack_group(&parts).unwrap_or_else(|e| unreachable!("{e}"));
        groups.push(write_group(
            &store,
            5,
            square.group_id,
            packed,
            vec![0, 1, 2, 3, 4],
        ));
    }
    groups.sort_by_key(|g| g.id);
    write_table(&store, 5, groups);
    let row = write_group(&store, 2, 0, vec![11, 22, 33], vec![0]);
    write_table(&store, 2, vec![row]);
    let mut profile = osrs_cache::registry::bundled_profiles()
        .unwrap_or_else(|e| unreachable!("{e}"))
        .remove(0);
    profile.cache_fingerprint.value = Some(
        CacheProbe::of(&dir)
            .unwrap_or_else(|e| unreachable!("{e}"))
            .fingerprint,
    );
    Fixture {
        dir,
        profile,
        regions,
    }
}

fn write_group(
    store: &CacheStore,
    index: u8,
    id: u32,
    bytes: Vec<u8>,
    file_ids: Vec<u32>,
) -> GroupEntry {
    let mut raw = encode_container(bytes.clone(), ContainerCompression::Gzip, None)
        .unwrap_or_else(|e| unreachable!("{e}"));
    let row = GroupEntry {
        id,
        name_hash: None,
        crc: crc32(&raw),
        hash: None,
        whirlpool: None,
        sizes: Some((raw.len() as u32, bytes.len() as u32)),
        version: 1,
        file_ids,
        file_name_hashes: None,
    };
    raw.extend_from_slice(&1u16.to_be_bytes());
    store
        .write_raw_group(index, id, &raw)
        .unwrap_or_else(|e| unreachable!("{e}"));
    row
}

fn write_table(store: &CacheStore, index: u8, groups: Vec<GroupEntry>) {
    let table = ReferenceTable {
        protocol: 7,
        revision: Some(7),
        flags: 4,
        groups,
    };
    let raw = encode_container(
        table.encode().unwrap_or_else(|e| unreachable!("{e}")),
        ContainerCompression::Gzip,
        None,
    )
    .unwrap_or_else(|e| unreachable!("{e}"));
    store
        .write_raw_group(255, u32::from(index), &raw)
        .unwrap_or_else(|e| unreachable!("{e}"));
}

pub fn baseline(regions: &[RegionCoord]) -> MapDocument {
    let mut document = MapDocument::new();
    for r in regions {
        let s = square(*r);
        // MapDocument owns source-region snapshots; build them with the production adapter.
        let map = osrs_world::RegionMap {
            terrain: osrs_scene::terrain_load::RegionTerrain::new(
                osrs_world::region_tiles_from_decoded(&s.terrain),
            )
            .unwrap_or_else(|e| unreachable!("{e:?}")),
            locations: s.locations,
        };
        document.replace_region(
            &crate::session::adapt_region_map(&map).unwrap_or_else(|e| unreachable!("{e:?}")),
        );
    }
    document
}

pub fn snapshot(f: &Fixture) -> ExportSnapshot {
    ExportSnapshot {
        stamp: ExportStamp {
            epoch: 1,
            job: 1,
            edit_revision: 1,
        },
        source: CacheSource::Dir(f.dir.clone()),
        profile: f.profile.id.clone(),
        document: baseline(&f.regions),
        authored: f.regions.iter().copied().collect(),
        pending: Vec::new(),
    }
}

pub fn paint(document: &mut MapDocument, p: MapPosition, id: u32) {
    let before = document.terrain_at(p);
    let after = osrs_edit::TerrainTile {
        underlay: Some(osrs_core::ids::FloorUnderlayId::new(id)),
        ..before
    };
    document
        .apply_patch(&MapPatch::new(vec![MapOp::SetTerrain {
            coord: p,
            before,
            after,
        }]))
        .unwrap_or_else(|e| unreachable!("{e:?}"));
}

pub fn prepared(f: &Fixture) -> PreparedExport {
    let mut snapshot = snapshot(f);
    paint(&mut snapshot.document, position(3263, 3202, 0), 5);
    paint(&mut snapshot.document, position(3264, 3202, 1), 7);
    super::prepare::prepare_with(
        snapshot,
        f.profile
            .cache_fingerprint
            .value
            .clone()
            .unwrap_or_default(),
        |r| Ok(Some(square(r))),
    )
}
